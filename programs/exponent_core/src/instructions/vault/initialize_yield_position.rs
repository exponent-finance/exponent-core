use anchor_lang::prelude::*;

use crate::{seeds::YIELD_POSITION_SEED, state::*};

/// Initialize a new YieldTokenPosition for a user
///
/// The YieldTokenPosition tracks earned interest (SY) and emissions for a user's deposited YT
///
#[event_cpi]
#[derive(Accounts)]
pub struct InitializeYieldPosition<'info> {
    /// Owner of the new YieldTokenPosition
    /// ie - the "owner" is not a Signer
    #[account(mut)]
    pub owner: Signer<'info>,

    pub vault: Box<Account<'info, Vault>>,

    #[account(
        init,
        payer = owner,
        space = YieldTokenPosition::size_of(vault.emissions.len()),
        seeds = [
            YIELD_POSITION_SEED,
            vault.key().as_ref(),
            owner.key().as_ref(),
        ],
        bump
    )]
    pub yield_position: Account<'info, YieldTokenPosition>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitializeYieldPosition>) -> Result<InitializeYieldPositionEvent> {
    let yield_position = &mut ctx.accounts.yield_position;
    yield_position.owner = ctx.accounts.owner.key();
    yield_position.vault = ctx.accounts.vault.key();

    let event = InitializeYieldPositionEvent {
        owner: *ctx.accounts.owner.key,
        vault: ctx.accounts.vault.key(),
        yield_position: ctx.accounts.yield_position.key(),
        unix_timestamp: Clock::get()?.unix_timestamp,
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct InitializeYieldPositionEvent {
    pub owner: Pubkey,
    pub vault: Pubkey,
    pub yield_position: Pubkey,
    pub unix_timestamp: i64,
}
