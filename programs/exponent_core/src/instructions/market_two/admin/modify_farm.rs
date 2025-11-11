use anchor_lang::prelude::*;
#[allow(deprecated)]
use anchor_spl::token_interface::{transfer, Mint, TokenAccount, TokenInterface, Transfer};
use exponent_admin::Admin;

use crate::MarketTwo;

#[derive(Accounts)]
pub struct ModifyFarm<'info> {
    #[account(mut)]
    pub market: Account<'info, MarketTwo>,

    pub signer: Signer<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    pub admin_state: Account<'info, Admin>,

    #[account(mut)]
    pub token_source: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = market,
    )]
    pub token_farm: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'i> ModifyFarm<'i> {
    pub fn validate(&self) -> Result<()> {
        self.admin_state
            .principles
            .collect_treasury
            .is_admin(&self.signer.key())?;

        Ok(())
    }

    fn to_transfer_sy_in_accounts(&self) -> Transfer<'i> {
        Transfer {
            authority: self.signer.to_account_info(),
            from: self.token_source.to_account_info(),
            to: self.token_farm.to_account_info(),
        }
    }

    pub fn to_transfer_from_market_accounts(&self) -> Transfer<'i> {
        Transfer {
            authority: self.market.to_account_info(),
            from: self.token_farm.to_account_info(),
            to: self.token_source.to_account_info(),
        }
    }

    pub fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_transfer_sy_in_accounts(),
        )
    }

    pub fn transfer_from_market_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_transfer_from_market_accounts(),
        )
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(
    ctx: Context<ModifyFarm>,
    new_expiration_timestamp: u32,
    new_rate: u64,
) -> Result<()> {
    let current_timestamp = Clock::get().unwrap().unix_timestamp as u32;
    let current_lp_escrow_amount = ctx.accounts.market.lp_escrow_amount;
    ctx.accounts
        .market
        .lp_farm
        .increase_share_indexes(current_timestamp, current_lp_escrow_amount);

    let farm_index = ctx
        .accounts
        .market
        .lp_farm
        .find_farm_emission_position(ctx.accounts.mint.key())
        .unwrap();

    let current_farm = &mut ctx.accounts.market.lp_farm.farm_emissions[farm_index];

    current_farm.expiry_timestamp = new_expiration_timestamp;
    current_farm.token_rate = new_rate;

    let new_tokens_needed =
        (new_expiration_timestamp as i64 - current_timestamp as i64) * new_rate as i64;

    // If the token_farm has less than the required amount, transfer the difference
    // Otherwise, transfer surplus to the token_source
    if new_tokens_needed > ctx.accounts.token_farm.amount as i64 {
        let required_amount = new_tokens_needed - ctx.accounts.token_farm.amount as i64;
        #[allow(deprecated)]
        transfer(ctx.accounts.transfer_ctx(), required_amount as u64)?;
    } else {
        let surplus_amount = ctx.accounts.token_farm.amount as i64 - new_tokens_needed;
        #[allow(deprecated)]
        transfer(
            ctx.accounts
                .transfer_from_market_ctx()
                .with_signer(&[&ctx.accounts.market.signer_seeds()]),
            surplus_amount as u64,
        )?;
    }

    Ok(())
}