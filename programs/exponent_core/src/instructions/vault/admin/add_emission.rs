use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenAccount;
use exponent_admin::Admin;

use crate::{cpi_common::CpiAccounts, utils::do_deposit_sy, Vault, YieldTokenPosition};

#[derive(Accounts)]
#[instruction(cpi_accounts: CpiAccounts, treasury_fee_bps: u16)]
pub struct AddEmission<'info> {
    pub authority: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(
        mut,
        has_one = sy_program,
        has_one = address_lookup_table,
        has_one = yield_position,
        realloc = Vault::size_of_static(vault.emissions.len() + 1) + cpi_accounts.size_of(),
        realloc::payer = fee_payer,
        realloc::zero = false,
    )]
    pub vault: Box<Account<'info, Vault>>,

    pub admin: Box<Account<'info, Admin>>,

    /// CHECK: constrained by vault
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: constrained by vault
    pub address_lookup_table: UncheckedAccount<'info>,

    /// Assert that the new token account is owned by the vault
    #[account(
        token::authority = vault.authority,
    )]
    pub robot_token_account: InterfaceAccount<'info, TokenAccount>,

    pub treasury_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Increase the robot's position size
    #[account(
        mut,
        realloc = YieldTokenPosition::size_of(vault.emissions.len() + 1),
        realloc::payer = fee_payer,
        realloc::zero = false,
    )]
    pub yield_position: Box<Account<'info, YieldTokenPosition>>,

    pub system_program: Program<'info, System>,
}

impl AddEmission<'_> {
    fn validate(&self) -> Result<()> {
        self.admin
            .principles
            .exponent_core
            .is_admin(self.authority.key)?;

        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, AddEmission<'info>>,
    cpi_accounts: CpiAccounts,
    treasury_fee_bps: u16,
) -> Result<()> {
    ctx.accounts.vault.cpi_accounts = cpi_accounts;

    let sy_state = do_deposit_sy(
        0,
        &ctx.accounts.address_lookup_table.to_account_info(),
        &ctx.accounts.vault.cpi_accounts,
        &ctx.accounts.to_account_infos(),
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.vault.signer_seeds()],
    )?;

    ctx.accounts.vault.add_emission(
        ctx.accounts.robot_token_account.key(),
        &sy_state,
        ctx.accounts.treasury_token_account.key(),
        treasury_fee_bps,
    );

    Ok(())
}
