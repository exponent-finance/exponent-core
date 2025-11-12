use crate::{
    error::ExponentCoreError, util::token_transfer, LpPosition, MarketTwo, PersonalYieldTrackers,
};
use amount_value::Amount;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface, Transfer};

#[event_cpi]
#[derive(Accounts)]
pub struct ClaimFarmEmissions<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    pub market: Account<'info, MarketTwo>,

    #[account(
        mut,
        has_one = market,
        has_one = owner
    )]
    pub lp_position: Account<'info, LpPosition>,

    #[account(mut)]
    pub token_dst: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = market,
    )]
    pub token_farm: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'i> ClaimFarmEmissions<'i> {
    fn to_transfer_sy_in_accounts(&self) -> Transfer<'i> {
        Transfer {
            authority: self.market.to_account_info(),
            from: self.token_farm.to_account_info(),
            to: self.token_dst.to_account_info(),
        }
    }

    fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_transfer_sy_in_accounts(),
        )
    }
}

pub fn handler(
    ctx: Context<ClaimFarmEmissions>,
    amount: Amount,
) -> Result<ClaimFarmEmissionsEventV2> {
    let index = ctx
        .accounts
        .market
        .lp_farm
        .find_farm_emission_position(ctx.accounts.mint.key())
        .ok_or(ExponentCoreError::FarmDoesNotExist)?;

    let amount_to_send = amount.to_u64(ctx.accounts.lp_position.farms.trackers[index].staged)?;

    token_transfer(
        ctx.accounts
            .transfer_ctx()
            .with_signer(&[&ctx.accounts.market.signer_seeds()]),
        amount_to_send,
    )?;

    ctx.accounts.lp_position.farms.trackers[index].dec_staged(amount_to_send);

    let event = ClaimFarmEmissionsEventV2 {
        owner: ctx.accounts.owner.key(),
        market: ctx.accounts.market.key(),
        lp_position: ctx.accounts.lp_position.key(),
        token_dst: ctx.accounts.token_dst.key(),
        mint: ctx.accounts.mint.key(),
        token_farm: ctx.accounts.token_farm.key(),
        farm_index: index as u8,
        amount_claimed: amount_to_send,
        remaining_staged: ctx.accounts.lp_position.farms.trackers[index].staged,
        emissions: ctx.accounts.lp_position.emissions.clone(),
        farms: ctx.accounts.lp_position.farms.clone(),
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct ClaimFarmEmissionsEvent {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub lp_position: Pubkey,
    pub token_dst: Pubkey,
    pub mint: Pubkey,
    pub token_farm: Pubkey,
    pub farm_index: u8,
    pub amount_claimed: u64,
    pub remaining_staged: u64,
}

#[event]
pub struct ClaimFarmEmissionsEventV2 {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub lp_position: Pubkey,
    pub token_dst: Pubkey,
    pub mint: Pubkey,
    pub token_farm: Pubkey,
    pub farm_index: u8,
    pub amount_claimed: u64,
    pub remaining_staged: u64,
    pub emissions: PersonalYieldTrackers,
    pub farms: PersonalYieldTrackers,
}
