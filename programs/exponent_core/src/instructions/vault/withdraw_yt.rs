use super::common::{update_vault_yield, yield_position_earn};
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

/// Withdraw YT from escrow, decreasing the balance stored in owner's YieldTokenPosition
///
/// Upon withdrawing, the owner's earned interest (SY) and emissions will be staged
/// before the YT balance is reduced and tokens are transferred back to the owner
#[event_cpi]
#[derive(Accounts)]
pub struct WithdrawYt<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Vault that manages YT
    #[account(
        mut,
        has_one = sy_program,
        has_one = escrow_yt,
        has_one = authority,
        has_one = address_lookup_table,
        has_one = yield_position
    )]
    pub vault: Account<'info, Vault>,

    /// Position data for YT deposits, linked to vault
    #[account(
        mut,
        has_one = vault,
        has_one = owner,
        realloc = YieldTokenPosition::size_of(vault.emissions.len()),
        realloc::payer = owner,
        realloc::zero = false,
    )]
    pub user_yield_position: Box<Account<'info, YieldTokenPosition>>,

    /// Withdrawer's YT token account
    #[account(mut)]
    pub yt_dst: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault-owned escrow account for YT
    #[account(mut)]
    pub escrow_yt: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    pub authority: SystemAccount<'info>,

    /// CHECK: constrained by the vault
    /// The SY interface implementation for the vault's token
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: constrained by the vault
    pub address_lookup_table: UncheckedAccount<'info>,

    #[account(mut)]
    pub yield_position: Box<Account<'info, YieldTokenPosition>>,

    pub system_program: Program<'info, System>,
}

impl<'i> WithdrawYt<'i> {
    fn transfer_context(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.escrow_yt.to_account_info(),
                to: self.yt_dst.to_account_info(),
                authority: self.authority.to_account_info(),
            },
        )
    }

    /// Only allow withdrawing YT if the vault is active and has the withdraw YT flag enabled
    fn validate(&self) -> Result<()> {
        let current_unix_timestamp = now();
        require!(
            self.vault.check_status_flags(STATUS_CAN_WITHDRAW_YT),
            ExponentCoreError::WithdrawingYtDisabled
        );

        require!(
            self.vault.is_active(current_unix_timestamp),
            ExponentCoreError::VaultExpired
        );

        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(ctx: Context<WithdrawYt>, amount: u64) -> Result<WithdrawYtEventV2> {
    let cur_ts = now();
    let sy_state = do_get_sy_state(
        &ctx.accounts.address_lookup_table.to_account_info(),
        &ctx.accounts.vault.cpi_accounts,
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
    )?;

    handle_withdraw_yt(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.yield_position,
        &mut ctx.accounts.user_yield_position,
        &sy_state,
        cur_ts,
        amount,
    );

    token_transfer(
        ctx.accounts
            .transfer_context()
            .with_signer(&[&ctx.accounts.vault.signer_seeds()]),
        amount,
    )?;

    let event = WithdrawYtEventV2 {
        owner: ctx.accounts.owner.key(),
        vault: ctx.accounts.vault.key(),
        user_yield_position: ctx.accounts.user_yield_position.key(),
        vault_yield_position: ctx.accounts.yield_position.key(),
        yt_dst: ctx.accounts.yt_dst.key(),
        escrow_yt: ctx.accounts.escrow_yt.key(),
        amount,
        sy_exchange_rate: ctx.accounts.vault.last_seen_sy_exchange_rate,
        user_yt_balance_after: ctx.accounts.user_yield_position.yt_balance,
        vault_yt_balance_after: ctx.accounts.yield_position.yt_balance,
        user_staged_yield: ctx.accounts.user_yield_position.interest.staged,
        unix_timestamp: Clock::get()?.unix_timestamp,
        user_interest: ctx.accounts.user_yield_position.interest,
        user_emissions: ctx.accounts.user_yield_position.emissions.clone(),
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct WithdrawYtEvent {
    pub owner: Pubkey,
    pub vault: Pubkey,
    pub user_yield_position: Pubkey,
    pub vault_yield_position: Pubkey,
    pub yt_dst: Pubkey,
    pub escrow_yt: Pubkey,
    pub amount: u64,
    pub sy_exchange_rate: Number,
    pub user_yt_balance_after: u64,
    pub vault_yt_balance_after: u64,
    pub user_staged_yield: u64,
    pub unix_timestamp: i64,
}

#[event]
pub struct WithdrawYtEventV2 {
    pub owner: Pubkey,
    pub vault: Pubkey,
    pub user_yield_position: Pubkey,
    pub vault_yield_position: Pubkey,
    pub yt_dst: Pubkey,
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

pub fn handle_withdraw_yt(
    vault: &mut Vault,
    vault_yield_position: &mut YieldTokenPosition,
    user_yield_position: &mut YieldTokenPosition,
    sy_state: &SyState,
    now: u32,
    amount: u64,
) {
    update_vault_yield(vault, vault_yield_position, now, sy_state);

    // Note that withdraw YT only can occur if the vault is active
    yield_position_earn(vault, user_yield_position);

    user_yield_position.dec_yt_balance(amount);
    vault_yield_position.inc_yt_balance(amount);

    // After increasing uncollected_sy, set_sy_for_pt
    vault.set_sy_for_pt();
}
