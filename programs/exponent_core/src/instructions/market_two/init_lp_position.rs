use crate::{seeds::LP_POSITION_SEED, LpPosition, MarketTwo};
use anchor_lang::prelude::*;

#[event_cpi]
#[derive(Accounts)]
pub struct InitLpPosition<'info> {
    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// CHECK: none - this init handler is permissionless
    pub owner: UncheckedAccount<'info>,

    pub market: Box<Account<'info, MarketTwo>>,

    #[account(
        init,
        payer = fee_payer,
        space = LpPosition::static_size_of(market.emissions.trackers.len(), market.lp_farm.farm_emissions.len()),
        seeds = [
            LP_POSITION_SEED,
            market.key().as_ref(),
            owner.key.as_ref()
        ],
        bump
    )]
    pub lp_position: Box<Account<'info, LpPosition>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitLpPosition>) -> Result<InitLpPositionEvent> {
    let lp_position_inner =
        LpPosition::new_from_market(&ctx.accounts.market, ctx.accounts.owner.key());

    ctx.accounts.lp_position.set_inner(lp_position_inner);

    let event = InitLpPositionEvent {
        fee_payer: ctx.accounts.fee_payer.key(),
        owner: ctx.accounts.owner.key(),
        market: ctx.accounts.market.key(),
        lp_position: ctx.accounts.lp_position.key(),
        num_emission_trackers: ctx.accounts.market.emissions.trackers.len() as u8,
        num_farm_emissions: ctx.accounts.market.lp_farm.farm_emissions.len() as u8,
        timestamp: Clock::get()?.unix_timestamp,
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct InitLpPositionEvent {
    pub fee_payer: Pubkey,
    pub owner: Pubkey,
    pub market: Pubkey,
    pub lp_position: Pubkey,
    pub num_emission_trackers: u8,
    pub num_farm_emissions: u8,
    pub timestamp: i64,
}
