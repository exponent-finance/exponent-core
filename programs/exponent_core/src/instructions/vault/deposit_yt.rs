use crate::{
    error::ExponentCoreError,
    state::*,
    util::{now, token_transfer},
    utils::do_get_sy_state,
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};
use precise_number::Number;
use sy_common::SyState;

use super::common::{update_vault_yield, yield_position_earn};

/// Deposit YT into escrow, increasing the balance stored in owner's YieldTokenPosition
///
/// YT does not actively earn interest (SY) and emissions unless it is deposited into the program
/// Upon depositing, the owner's earned interest (SY) and emissions will be staged

#[event_cpi]
#[derive(Accounts)]
pub struct DepositYt<'info> {
    /// Permissionless depositor - not necessarily the owner of the YieldTokenPosition
    #[account(mut)]
    pub depositor: Signer<'info>,

    /// Vault that manages YT
    #[account(
        mut,
        has_one = sy_program,
        has_one = escrow_yt,
        has_one = address_lookup_table,
        has_one = yield_position
    )]
    pub vault: Account<'info, Vault>,

    /// Position data for YT deposits, linked to vault
    #[account(
        mut,
        has_one = vault,
        realloc = YieldTokenPosition::size_of(vault.emissions.len()),
        realloc::payer = depositor,
        realloc::zero = false,
        // no duplicate YT positions
        constraint = user_yield_position.key() != yield_position.key()
    )]
    pub user_yield_position: Box<Account<'info, YieldTokenPosition>>,

    /// Depositor's YT token account
    #[account(mut)]
    pub yt_src: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault-owned escrow account for YT
    #[account(mut)]
    pub escrow_yt: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    /// CHECK: constrained by the vault
    /// The SY interface implementation for the vault's token
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: constrained by the vault
    pub address_lookup_table: UncheckedAccount<'info>,

    /// Vault-owned yield token position
    #[account(mut)]
    pub yield_position: Box<Account<'info, YieldTokenPosition>>,

    pub system_program: Program<'info, System>,
}

impl<'i> DepositYt<'i> {
    fn transfer_context(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.yt_src.to_account_info(),
                to: self.escrow_yt.to_account_info(),
                authority: self.depositor.to_account_info(),
            },
        )
    }

    fn validate(&self) -> Result<()> {
        let current_unix_timestamp = now();

        require!(
            self.vault.check_status_flags(STATUS_CAN_DEPOSIT_YT),
            ExponentCoreError::DepositingYtDisabled
        );

        require!(
            self.vault.is_active(current_unix_timestamp),
            ExponentCoreError::VaultExpired
        );

        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(ctx: Context<DepositYt>, amount: u64) -> Result<DepositYtEventV2> {
    let current_unix_timestamp = now();

    let sy_state = do_get_sy_state(
        &ctx.accounts.address_lookup_table.to_account_info(),
        &ctx.accounts.vault.cpi_accounts,
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
    )?;

    handle_deposit_yt(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.yield_position,
        &mut ctx.accounts.user_yield_position,
        &sy_state,
        current_unix_timestamp,
        amount,
    )?;

    token_transfer(ctx.accounts.transfer_context(), amount)?;

    let event = DepositYtEventV2 {
        depositor: *ctx.accounts.depositor.key,
        vault: ctx.accounts.vault.key(),
        user_yield_position: ctx.accounts.user_yield_position.key(),
        vault_yield_position: ctx.accounts.yield_position.key(),
        yt_src: ctx.accounts.yt_src.key(),
        escrow_yt: ctx.accounts.escrow_yt.key(),
        amount,
        sy_exchange_rate: sy_state.exchange_rate,
        user_yt_balance_after: ctx.accounts.user_yield_position.yt_balance,
        vault_yt_balance_after: ctx.accounts.yield_position.yt_balance,
        user_staged_yield: ctx.accounts.user_yield_position.interest.staged,
        unix_timestamp: current_unix_timestamp as i64,
        user_interest: ctx.accounts.user_yield_position.interest,
        user_emissions: ctx.accounts.user_yield_position.emissions.clone(),
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct DepositYtEvent {
    pub depositor: Pubkey,
    pub vault: Pubkey,
    pub user_yield_position: Pubkey,
    pub vault_yield_position: Pubkey,
    pub yt_src: Pubkey,
    pub escrow_yt: Pubkey,
    pub amount: u64,
    pub sy_exchange_rate: Number,
    pub user_yt_balance_after: u64,
    pub vault_yt_balance_after: u64,
    pub user_staged_yield: u64,
    pub unix_timestamp: i64,
}

#[event]
pub struct DepositYtEventV2 {
    pub depositor: Pubkey,
    pub vault: Pubkey,
    pub user_yield_position: Pubkey,
    pub vault_yield_position: Pubkey,
    pub yt_src: Pubkey,
    pub escrow_yt: Pubkey,
    pub amount: u64,
    pub sy_exchange_rate: Number,
    pub user_yt_balance_after: u64,
    pub vault_yt_balance_after: u64,
    pub user_staged_yield: u64,
    pub unix_timestamp: i64,
    pub user_interest: YieldTokenTracker,
    pub user_emissions: Vec<YieldTokenTracker>,
}

pub fn handle_deposit_yt(
    vault: &mut Vault,
    vault_yield_position: &mut YieldTokenPosition,
    user_yield_position: &mut YieldTokenPosition,
    sy_state: &SyState,
    now: u32,
    amount: u64,
) -> Result<()> {
    // First, update the vault indexes and sync its own yield position
    update_vault_yield(vault, vault_yield_position, now, sy_state);

    // Do not allow deposits if the current sy exchange rate is lower than the all-time high
    // This prevents users from increasing their position amount with a lower last_seen_index, which would break the economics.
    require!(
        !vault.is_in_emergency_mode(),
        ExponentCoreError::VaultInEmergencyMode
    );

    // Then, stage an earnings with the user's YT position
    yield_position_earn(vault, user_yield_position);

    // increase the user's YT balance
    user_yield_position.inc_yt_balance(amount);

    // decrease the vault's YT balance
    vault_yield_position.dec_yt_balance(amount);

    // After increasing uncollected_sy, set_sy_for_pt
    vault.set_sy_for_pt();

    Ok(())
}