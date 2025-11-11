use super::common::{update_vault_yield, yield_position_earn};
use crate::{error::ExponentCoreError, state::*, util::now, utils::do_get_sy_state};
use anchor_lang::prelude::*;
use precise_number::Number;
use sy_common::SyState;

#[event_cpi]
#[derive(Accounts)]
pub struct StageYield<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        has_one = sy_program,
        has_one = address_lookup_table,
        has_one = yield_position,
    )]
    pub vault: Box<Account<'info, Vault>>,

    /// Position data for YT deposits, linked to vault
    #[account(
        mut,
        has_one = vault,
        realloc = YieldTokenPosition::size_of(vault.emissions.len()),
        realloc::payer = payer,
        realloc::zero = false,
        // no duplicate YT positions
        constraint = user_yield_position.key() != yield_position.key()
    )]
    pub user_yield_position: Box<Account<'info, YieldTokenPosition>>,

    /// Yield position for the vault robot account
    #[account(mut)]
    pub yield_position: Box<Account<'info, YieldTokenPosition>>,

    /// CHECK: constrained by SyMetadata
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: constrained by vault
    pub address_lookup_table: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<StageYield>) -> Result<StageYieldEventV2> {
    let sy_state = do_get_sy_state(
        &ctx.accounts.address_lookup_table.to_account_info(),
        &ctx.accounts.vault.cpi_accounts,
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
    )?;

    handle_stage_yt_yield(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.yield_position,
        &mut ctx.accounts.user_yield_position,
        &sy_state,
        now(),
    )?;

    let event = StageYieldEventV2 {
        payer: ctx.accounts.payer.key(),
        vault: ctx.accounts.vault.key(),
        user_yield_position: ctx.accounts.user_yield_position.key(),
        vault_yield_position: ctx.accounts.yield_position.key(),
        sy_exchange_rate: sy_state.exchange_rate,
        user_yt_balance: ctx.accounts.user_yield_position.yt_balance,
        user_staged_yield: ctx.accounts.user_yield_position.interest.staged,
        unix_timestamp: Clock::get()?.unix_timestamp,
        user_interest: ctx.accounts.user_yield_position.interest,
        user_emissions: ctx.accounts.user_yield_position.emissions.clone(),
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct StageYieldEvent {
    pub payer: Pubkey,
    pub vault: Pubkey,
    pub user_yield_position: Pubkey,
    pub vault_yield_position: Pubkey,
    pub sy_exchange_rate: Number,
    pub user_yt_balance: u64,
    pub user_staged_yield: u64,
    pub user_staged_emissions: Vec<u64>,
    pub unix_timestamp: i64,
}

#[event]
pub struct StageYieldEventV2 {
    pub payer: Pubkey,
    pub vault: Pubkey,
    pub user_yield_position: Pubkey,
    pub vault_yield_position: Pubkey,
    pub sy_exchange_rate: Number,
    pub user_yt_balance: u64,
    pub user_staged_yield: u64,
    pub unix_timestamp: i64,
    pub user_interest: YieldTokenTracker,
    pub user_emissions: Vec<YieldTokenTracker>,
}

pub fn handle_stage_yt_yield(
    vault: &mut Vault,
    vault_yield_position: &mut YieldTokenPosition,
    user_yield_position: &mut YieldTokenPosition,
    sy_state: &SyState,
    now: u32,
) -> Result<()> {
    // update vault indexees from SY state
    // and stage any yield to the vault's robot account
    update_vault_yield(vault, vault_yield_position, now, sy_state);

    // TODO - consider removing this check, since deeper in the stack we check for this
    require!(
        !vault.is_in_emergency_mode(),
        ExponentCoreError::VaultInEmergencyMode
    );

    yield_position_earn(vault, user_yield_position);

    // Set SY for PT
    vault.set_sy_for_pt();

    Ok(())
}