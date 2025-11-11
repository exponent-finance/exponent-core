use crate::{
    state::*,
    util::{deserialize_lookup_table, token_transfer},
    utils::cpi_claim_emission,
    PersonalYieldTrackers,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface, Transfer};
use cpi_common::to_account_metas;

#[event_cpi]
#[derive(Accounts)]
#[instruction(emission_index: u16)]
pub struct MarketCollectEmission<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = sy_program,
        has_one = address_lookup_table,
    )]
    pub market: Box<Account<'info, MarketTwo>>,

    #[account(
        mut,
        has_one = owner,
        has_one = market,
    )]
    pub lp_position: Box<Account<'info, LpPosition>>,

    #[account(
        mut,
        address = market.emissions.trackers[emission_index as usize].token_escrow
    )]
    pub token_emission_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: constrained by token program
    #[account(mut)]
    pub token_emission_dst: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,

    /// CHECK: constrained by market
    pub address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: constrained by market
    pub sy_program: UncheckedAccount<'info>,
}

impl<'i> MarketCollectEmission<'i> {
    fn transfer_emission_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.token_emission_escrow.to_account_info(),
                to: self.token_emission_dst.to_account_info(),
                authority: self.market.to_account_info(),
            },
        )
    }
}

pub fn handler(
    ctx: Context<MarketCollectEmission>,
    emission_index: u16,
) -> Result<MarketCollectEmissionEventV2> {
    let emission_index = emission_index as usize;

    let amount = ctx.accounts.lp_position.emissions.trackers[emission_index].staged;
    // zero it out
    ctx.accounts.lp_position.emissions.trackers[emission_index].staged = 0;

    cpi_claim_emission(
        ctx.accounts.sy_program.key(),
        amount,
        ctx.remaining_accounts,
        to_account_metas(
            &ctx.accounts.market.cpi_accounts.claim_emission[emission_index],
            &deserialize_lookup_table(&ctx.accounts.address_lookup_table),
        ),
        &[&ctx.accounts.market.signer_seeds()],
    )?;

    token_transfer(
        ctx.accounts
            .transfer_emission_ctx()
            .with_signer(&[&ctx.accounts.market.signer_seeds()]),
        amount,
    )?;

    ctx.accounts.market.emissions.trackers[emission_index].last_seen_staged =
        ctx.accounts.market.emissions.trackers[emission_index]
            .last_seen_staged
            .checked_sub(amount)
            .unwrap();

    let event = MarketCollectEmissionEventV2 {
        owner: ctx.accounts.owner.key(),
        market: ctx.accounts.market.key(),
        lp_position: ctx.accounts.lp_position.key(),
        token_emission_escrow: ctx.accounts.token_emission_escrow.key(),
        token_emission_dst: ctx.accounts.token_emission_dst.key(),
        emission_index: emission_index as u16,
        amount_collected: amount,
        timestamp: Clock::get()?.unix_timestamp,
        emissions: ctx.accounts.lp_position.emissions.clone(),
        farms: ctx.accounts.lp_position.farms.clone(),
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct MarketCollectEmissionEvent {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub lp_position: Pubkey,
    pub token_emission_escrow: Pubkey,
    pub token_emission_dst: Pubkey,
    pub emission_index: u16,
    pub amount_collected: u64,
    pub timestamp: i64,
}

#[event]
pub struct MarketCollectEmissionEventV2 {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub lp_position: Pubkey,
    pub token_emission_escrow: Pubkey,
    pub token_emission_dst: Pubkey,
    pub emission_index: u16,
    pub amount_collected: u64,
    pub timestamp: i64,
    pub emissions: PersonalYieldTrackers,
    pub farms: PersonalYieldTrackers,
}