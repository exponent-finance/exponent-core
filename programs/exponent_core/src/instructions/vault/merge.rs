use super::common::update_vault_yield;
use crate::{
    error::ExponentCoreError,
    state::*,
    util::{now, token_transfer},
    utils::{do_get_sy_state, do_withdraw_sy},
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_2022::Burn, token_interface::*};
use precise_number::Number;
use sy_common::SyState;

/// Burn PT & YT in order to receive SY from the vault's escrow account
/// TODO: apply naming conventions (token_yt_src, etc)
#[event_cpi]
#[derive(Accounts)]
pub struct Merge<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: cosntrained by vault - though it does not need to be constrained
    /// This authority owns the escrow_sy account & the robot account with the SY program
    /// Needs to be mutable to be used in deposit_sy CPI
    #[account(mut)]
    pub authority: UncheckedAccount<'info>,

    #[account(
        mut,
        has_one = sy_program,
        has_one = escrow_sy,
        has_one = address_lookup_table,
        has_one = mint_yt,
        has_one = mint_pt,
        has_one = authority,
        has_one = yield_position,
    )]
    pub vault: Box<Account<'info, Vault>>,

    /// Destination account for SY withdrawn from vault
    #[account(mut)]
    pub sy_dst: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault-owned account for SY tokens
    #[account(mut)]
    pub escrow_sy: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The owner's YT token account
    #[account(mut)]
    pub yt_src: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The owner's PT token account
    #[account(mut)]
    pub pt_src: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Mint for YT -- needed for burning
    #[account(mut)]
    pub mint_yt: Box<InterfaceAccount<'info, Mint>>,

    /// Mint for PT -- needed for burning
    #[account(mut)]
    pub mint_pt: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Program<'info, Token>,

    /// CHECK: constrained by SyMetadata
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: constrained by vault
    pub address_lookup_table: UncheckedAccount<'info>,

    /// Yield position for the vault robot account
    #[account(mut)]
    pub yield_position: Box<Account<'info, YieldTokenPosition>>,
}

impl<'a> Merge<'a> {
    fn burn_pt_context(&self) -> CpiContext<'_, '_, '_, 'a, Burn<'a>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Burn {
                from: self.pt_src.to_account_info(),
                mint: self.mint_pt.to_account_info(),
                authority: self.owner.to_account_info(),
            },
        )
    }

    fn burn_yt_context(&self) -> CpiContext<'_, '_, '_, 'a, Burn<'a>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Burn {
                from: self.yt_src.to_account_info(),
                mint: self.mint_yt.to_account_info(),
                authority: self.owner.to_account_info(),
            },
        )
    }

    fn transfer_sy_context(&self) -> CpiContext<'_, '_, '_, 'a, Transfer<'a>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.escrow_sy.to_account_info(),
                to: self.sy_dst.to_account_info(),
                authority: self.authority.to_account_info(),
            },
        )
    }

    /// Transfer SY to the user
    fn transfer_sy(&self, amount: u64) -> Result<()> {
        token_transfer(
            self.transfer_sy_context()
                .with_signer(&[&self.vault.signer_seeds()]),
            amount,
        )
    }

    /// Burn PT & YT in order to receive SY from the vault's escrow account
    fn burn_py(&mut self, amount: u64, current_timestamp: u32) -> Result<()> {
        anchor_spl::token_2022::burn(self.burn_pt_context(), amount)?;

        // If the maturity has passed, do not burn the YT
        // If the vault is still active, then we must burn YT
        if self.vault.is_active(current_timestamp) {
            anchor_spl::token_2022::burn(self.burn_yt_context(), amount)?;
        }

        Ok(())
    }

    fn validate(&self, amount_py: u64) -> Result<()> {
        require!(
            self.vault.check_status_flags(STATUS_CAN_MERGE),
            ExponentCoreError::MergingDisabled
        );

        require!(
            self.vault.is_min_op_size_merge(amount_py),
            ExponentCoreError::OperationAmountTooSmall
        );

        Ok(())
    }
}

// `amount` is amount of PT & YT to be merged together for redemption
#[access_control(ctx.accounts.validate(amount_py))]
pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, Merge<'info>>,
    amount_py: u64,
) -> Result<MergeEvent> {
    let current_unix_timestamp = now();

    // get the latest exchange rate for SY using the CPI interface & return data
    let sy_state = do_get_sy_state(
        &ctx.accounts.address_lookup_table.to_account_info(),
        &ctx.accounts.vault.cpi_accounts,
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
    )?;

    let amount_sy = handle_merge(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.yield_position,
        current_unix_timestamp,
        &sy_state,
        amount_py,
    )?;

    do_withdraw_sy(
        amount_sy,
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.vault.cpi_accounts,
        &ctx.accounts.to_account_infos(),
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.vault.signer_seeds()],
    )
    .expect("failed to withdraw sy from CPI");

    // Transfer SY to the owner
    ctx.accounts.transfer_sy(amount_sy)?;

    // Burn the PT (& YT if the vault is active)
    ctx.accounts.burn_py(amount_py, current_unix_timestamp)?;

    let event = MergeEvent {
        owner: ctx.accounts.owner.key(),
        vault: ctx.accounts.vault.key(),
        sy_dst: ctx.accounts.sy_dst.key(),
        escrow_sy: ctx.accounts.escrow_sy.key(),
        yt_src: ctx.accounts.yt_src.key(),
        pt_src: ctx.accounts.pt_src.key(),
        mint_yt: ctx.accounts.mint_yt.key(),
        mint_pt: ctx.accounts.mint_pt.key(),
        yield_position: ctx.accounts.yield_position.key(),
        amount_py_in: amount_py,
        amount_sy_out: amount_sy,
        sy_exchange_rate: sy_state.exchange_rate,
        pt_redemption_rate: ctx.accounts.vault.pt_redemption_rate(),
        total_sy_in_escrow: ctx.accounts.vault.total_sy_in_escrow,
        pt_supply: ctx.accounts.vault.pt_supply,
        yt_balance: ctx.accounts.yield_position.yt_balance,
        sy_for_pt: ctx.accounts.vault.sy_for_pt,
        is_vault_active: ctx.accounts.vault.is_active(current_unix_timestamp),
        unix_timestamp: Clock::get()?.unix_timestamp,
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct MergeEvent {
    pub owner: Pubkey,
    pub vault: Pubkey,
    pub sy_dst: Pubkey,
    pub escrow_sy: Pubkey,
    pub yt_src: Pubkey,
    pub pt_src: Pubkey,
    pub mint_yt: Pubkey,
    pub mint_pt: Pubkey,
    pub yield_position: Pubkey,
    pub amount_py_in: u64,
    pub amount_sy_out: u64,
    pub sy_exchange_rate: Number,
    pub pt_redemption_rate: Number,
    pub total_sy_in_escrow: u64,
    pub pt_supply: u64,
    pub yt_balance: u64,
    pub sy_for_pt: u64,
    pub is_vault_active: bool,
    pub unix_timestamp: i64,
}

/// Given an amount of PT, calculate the amount of SY to be redeemed
fn calc_amount_sy(vault: &Vault, amount_py: u64) -> u64 {
    let pt_redemption_rate = vault.pt_redemption_rate();

    // Flooring the result to return less SY than the actual amount
    (Number::from_natural_u64(amount_py) * pt_redemption_rate).floor_u64()
}

fn adjust_vault_balances(vault: &mut Vault, amount_sy: u64, amount_pt: u64) {
    vault.dec_total_sy_in_escrow(amount_sy);
    vault.dec_pt_supply(amount_pt);
}

pub fn handle_merge(
    vault: &mut Vault,
    vault_yield_position: &mut YieldTokenPosition,
    now: u32,
    sy_state: &SyState,
    amount_py: u64,
) -> Result<u64> {
    update_vault_yield(vault, vault_yield_position, now, sy_state);

    require!(
        !vault.is_in_emergency_mode(),
        ExponentCoreError::VaultInEmergencyMode
    );

    // If the vault is active, then the YT must be burned
    if vault.is_active(now) {
        vault_yield_position.dec_yt_balance(amount_py);
    }

    let amount_sy = calc_amount_sy(vault, amount_py);

    adjust_vault_balances(vault, amount_sy, amount_py);

    vault.set_sy_for_pt();

    Ok(amount_sy)
}
