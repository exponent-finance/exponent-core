use anchor_lang::prelude::*;
use exponent_admin::Admin;

use crate::{cpi_common::CpiAccounts, Vault};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum AdminAction {
    SetVaultStatus(u8),
    ChangeVaultBpsFee(u16),
    ChangeVaultTreasuryTokenAccount(Pubkey),
    ChangeEmissionTreasuryTokenAccount {
        emission_index: u16,
        new_token_account: Pubkey,
    },
    ChangeMinOperationSize {
        is_strip: bool,
        new_size: u64,
    },
    ChangeEmissionBpsFee {
        emission_index: u16,
        new_fee_bps: u16,
    },
    ChangeCpiAccounts {
        cpi_accounts: CpiAccounts,
    },
    ChangeClaimLimits {
        max_claim_amount_per_window: u64,
        claim_window_duration_seconds: u32,
    },
    ChangeMaxPySupply {
        new_max_py_supply: u64,
    },
    ChangeAddressLookupTable(Pubkey),
    RemoveVaultEmission(u8),
}

#[derive(Accounts)]
pub struct ModifyVaultSetting<'info> {
    #[account(mut)]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub admin_state: Account<'info, Admin>,

    pub system_program: Program<'info, System>,
}

impl ModifyVaultSetting<'_> {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(ctx: Context<ModifyVaultSetting>, action: AdminAction) -> Result<()> {
    let vault = &mut ctx.accounts.vault;

    match action {
        AdminAction::SetVaultStatus(new_status) => {
            ctx.accounts
                .admin_state
                .principles
                .change_status_flags
                .is_admin(ctx.accounts.signer.key)?;

            vault.status = new_status;
        }
        AdminAction::ChangeVaultBpsFee(new_fee) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            assert!(
                new_fee <= 10000,
                "Fee BPS must be less than or equal to 10000"
            );

            vault.interest_bps_fee = new_fee;
        }
        AdminAction::ChangeVaultTreasuryTokenAccount(new_account) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            vault.treasury_sy_token_account = new_account;
        }
        AdminAction::ChangeEmissionTreasuryTokenAccount {
            emission_index,
            new_token_account,
        } => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            vault.emissions[emission_index as usize].treasury_token_account = new_token_account;
        }
        AdminAction::ChangeMinOperationSize { is_strip, new_size } => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            if is_strip {
                vault.min_op_size_strip = new_size;
            } else {
                vault.min_op_size_merge = new_size;
            }
        }
        AdminAction::ChangeEmissionBpsFee {
            emission_index,
            new_fee_bps,
        } => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            assert!(
                new_fee_bps <= 10000,
                "Fee BPS must be less than or equal to 10000"
            );

            vault.emissions[emission_index as usize].fee_bps = new_fee_bps;
        }
        AdminAction::ChangeCpiAccounts { cpi_accounts } => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            let old_size = vault.to_account_info().data_len();
            let new_size = Vault::size_of_static(vault.emissions.len()) + cpi_accounts.size_of();

            if new_size > old_size {
                let additional_rent = Rent::get()?.minimum_balance(new_size - old_size);
                anchor_lang::system_program::transfer(
                    CpiContext::new(
                        ctx.accounts.system_program.to_account_info(),
                        anchor_lang::system_program::Transfer {
                            from: ctx.accounts.signer.to_account_info(),
                            to: vault.to_account_info(),
                        },
                    ),
                    additional_rent,
                )?;
            }

            vault.to_account_info().realloc(new_size, false)?;
            vault.cpi_accounts = cpi_accounts;
        }
        AdminAction::ChangeClaimLimits {
            max_claim_amount_per_window,
            claim_window_duration_seconds,
        } => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            vault.claim_limits.claim_window_start_timestamp = Clock::get()?.unix_timestamp as u32;
            vault.claim_limits.total_claim_amount_in_window = 0;
            vault.claim_limits.max_claim_amount_per_window = max_claim_amount_per_window;
            vault.claim_limits.claim_window_duration_seconds = claim_window_duration_seconds;
        }
        AdminAction::ChangeMaxPySupply { new_max_py_supply } => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            vault.max_py_supply = new_max_py_supply;
        }
        AdminAction::ChangeAddressLookupTable(address_lookup_table) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            vault.address_lookup_table = address_lookup_table;
        }
        AdminAction::RemoveVaultEmission(emission_index) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            vault.emissions.remove(emission_index as usize);
        }
    }

    Ok(())
}
