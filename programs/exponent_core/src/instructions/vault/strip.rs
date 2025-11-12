use super::common::update_vault_yield;
use crate::{
    error::ExponentCoreError,
    state::*,
    util::{now, token_transfer},
    utils::*,
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};
use precise_number::Number;
use sy_common::SyState;

/// Deposit SY into the vault and receive an equivalent quantity of PT & YT tokens
/// The amount of PT & YT is determined by the exchange rate of the SY token
/// TODO: naming conventions (eg, `token_yt_src` etc)
#[event_cpi]
#[derive(Accounts)]
pub struct Strip<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    /// CHECK: constrained by vault
    /// This account owns the mints for PT & YT
    /// And owns the robot account with the SY program
    /// Needs to be mutable to be used in deposit_sy CPI
    #[account(mut)]
    pub authority: UncheckedAccount<'info>,

    #[account(
        mut,
        has_one = sy_program,
        has_one = mint_yt,
        has_one = mint_pt,
        has_one = escrow_sy,
        has_one = authority,
        has_one = address_lookup_table,
        has_one = yield_position
    )]
    pub vault: Box<Account<'info, Vault>>,

    /// Depositor's SY token account
    #[account(mut)]
    pub sy_src: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Temporary account owned by the vault for receiving SY tokens before depositing into the SY Program's escrow account
    #[account(mut)]
    pub escrow_sy: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Final destination for receiving YT (probably, but not necessarily, owned by the depositor)
    #[account(mut)]
    pub yt_dst: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Final destination for receiving PT (probably, but not necessarily, owned by the depositor)
    #[account(mut)]
    pub pt_dst: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub mint_yt: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub mint_pt: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Program<'info, Token>,

    /// Address lookup table for vault
    /// CHECK: constrained by vault
    pub address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: constrained by vault
    pub sy_program: UncheckedAccount<'info>,

    /// Vault-owned yield position account
    #[account(mut)]
    pub yield_position: Account<'info, YieldTokenPosition>,
}

impl<'a> Strip<'a> {
    fn mint_pt_context(&self) -> CpiContext<'_, '_, '_, 'a, MintTo<'a>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            MintTo {
                mint: self.mint_pt.to_account_info(),
                to: self.pt_dst.to_account_info(),
                authority: self.authority.to_account_info(),
            },
        )
    }

    fn mint_yt_context(&self) -> CpiContext<'_, '_, '_, 'a, MintTo<'a>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            MintTo {
                mint: self.mint_yt.to_account_info(),
                to: self.yt_dst.to_account_info(),
                authority: self.authority.to_account_info(),
            },
        )
    }

    fn transfer_sy_context(&self) -> CpiContext<'_, '_, '_, 'a, Transfer<'a>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.sy_src.to_account_info(),
                to: self.escrow_sy.to_account_info(),
                authority: self.depositor.to_account_info(),
            },
        )
    }

    fn mint_py(&mut self, amount: u64) -> Result<()> {
        anchor_spl::token_2022::mint_to(
            self.mint_pt_context()
                .with_signer(&[&self.vault.signer_seeds()]),
            amount,
        )?;

        anchor_spl::token_2022::mint_to(
            self.mint_yt_context()
                .with_signer(&[&self.vault.signer_seeds()]),
            amount,
        )?;

        Ok(())
    }

    fn validate(&self, amount: u64) -> Result<()> {
        let current_timestamp = now();

        require!(
            self.vault.check_status_flags(STATUS_CAN_STRIP),
            ExponentCoreError::StrippingDisabled
        );

        require!(
            self.vault.is_active(current_timestamp),
            ExponentCoreError::VaultIsNotActive
        );

        require!(
            self.vault.is_min_op_size_strip(amount),
            ExponentCoreError::OperationAmountTooSmall
        );

        Ok(())
    }
}

// `amount` is amount of SY to strip
#[access_control(ctx.accounts.validate(amount))]
pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, Strip<'info>>,
    amount: u64,
) -> Result<StripEvent> {
    // First, transfer SY tokens to account owned by vault
    token_transfer(ctx.accounts.transfer_sy_context(), amount)?;

    // Then transfer SY tokens into sy_program
    let sy_state = do_deposit_sy(
        amount,
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.vault.cpi_accounts,
        &ctx.accounts.to_account_infos(),
        &ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.vault.signer_seeds()],
    )?;

    // Main logic mutating state
    let current_unix_timestamp = now();
    let amount_py = handle_strip(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.yield_position,
        current_unix_timestamp,
        &sy_state,
        amount,
    )?;

    // Mint PT & YT to target accounts
    ctx.accounts.mint_py(amount_py)?;

    let event = StripEvent {
        depositor: ctx.accounts.depositor.key(),
        vault: ctx.accounts.vault.key(),
        authority: ctx.accounts.authority.key(),
        sy_src: ctx.accounts.sy_src.key(),
        escrow_sy: ctx.accounts.escrow_sy.key(),
        yt_dst: ctx.accounts.yt_dst.key(),
        pt_dst: ctx.accounts.pt_dst.key(),
        mint_yt: ctx.accounts.mint_yt.key(),
        mint_pt: ctx.accounts.mint_pt.key(),
        yield_position: ctx.accounts.yield_position.key(),
        amount_sy_in: amount,
        amount_py_out: amount_py,
        sy_exchange_rate: ctx.accounts.vault.last_seen_sy_exchange_rate,
        total_sy_in_escrow: ctx.accounts.vault.total_sy_in_escrow,
        pt_supply: ctx.accounts.vault.pt_supply,
        yt_balance: ctx.accounts.yield_position.yt_balance,
        all_time_high_sy_exchange_rate: ctx.accounts.vault.all_time_high_sy_exchange_rate,
        sy_for_pt: ctx.accounts.vault.sy_for_pt,
        unix_timestamp: Clock::get()?.unix_timestamp,
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct StripEvent {
    pub depositor: Pubkey,
    pub vault: Pubkey,
    pub authority: Pubkey,
    pub sy_src: Pubkey,
    pub escrow_sy: Pubkey,
    pub yt_dst: Pubkey,
    pub pt_dst: Pubkey,
    pub mint_yt: Pubkey,
    pub mint_pt: Pubkey,
    pub yield_position: Pubkey,
    pub amount_sy_in: u64,
    pub amount_py_out: u64,
    pub sy_exchange_rate: Number,
    pub total_sy_in_escrow: u64,
    pub pt_supply: u64,
    pub yt_balance: u64,
    pub all_time_high_sy_exchange_rate: Number,
    pub sy_for_pt: u64,
    pub unix_timestamp: i64,
}

/// Handle the strip logic for the vault
/// Returns the amount of PT & YT to mint
pub fn handle_strip(
    vault: &mut Vault,
    vault_yield_position: &mut YieldTokenPosition,
    current_unix_timestamp: u32,
    sy_state: &SyState,
    amount_sy: u64,
) -> Result<u64> {
    update_vault_yield(
        vault,
        vault_yield_position,
        current_unix_timestamp,
        &sy_state,
    );

    require!(
        !vault.is_in_emergency_mode(),
        ExponentCoreError::VaultInEmergencyMode
    );

    // Get value of PT & YT tokens for SY deposited
    let amount_py = sy_to_py(vault.last_seen_sy_exchange_rate, amount_sy);

    // Increase the vault's balance of SY held in escrow
    vault.inc_total_sy_in_escrow(amount_sy);

    // Increase the total PT supply
    vault.inc_pt_supply(amount_py);

    vault_yield_position.inc_yt_balance(amount_py);

    vault.set_sy_for_pt();

    Ok(amount_py)
}
