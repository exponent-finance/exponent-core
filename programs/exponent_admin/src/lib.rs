use anchor_lang::prelude::*;

declare_id!("3D6ojc8vBfDteLBDTTRznZbZRh7bkEGQaYqNkudoTCBQ");

pub const ADMIN_STORE_SEED: &[u8] = b"admin";

#[program]
pub mod exponent_admin {
    use super::*;

    pub fn initialize_admin(ctx: Context<InitializeAdmin>) -> Result<()> {
        let admin_account = &mut ctx.accounts.admin_account;
        admin_account.uber_admin = ID;
        Ok(())
    }

    pub fn invite_admin(ctx: Context<InviteAdmin>) -> Result<()> {
        let admin_account = &mut ctx.accounts.admin_account;
        admin_account.is_uber_admin(&ctx.accounts.uber_admin.key())?;
        admin_account.proposed_uber_admin = Some(ctx.accounts.proposed_admin.key());
        Ok(())
    }

    pub fn accept_invitation(ctx: Context<AcceptInvitation>) -> Result<()> {
        let admin_account = &mut ctx.accounts.admin_account;
        if admin_account.proposed_uber_admin.is_none() {
            return err!(ErrorCode::NoProposedAdmin);
        }
        admin_account.uber_admin = *admin_account.proposed_uber_admin.as_ref().unwrap();
        admin_account.proposed_uber_admin = None;
        Ok(())
    }

    pub fn add_principle_admin(
        ctx: Context<AddPrincipleAdmin>,
        principle: Principle,
    ) -> Result<()> {
        let admin_account = &mut ctx.accounts.admin_account;
        admin_account.is_uber_admin(&ctx.accounts.uber_admin.key())?;
        match principle {
            Principle::MarginfiStandard => admin_account
                .principles
                .marginfi_standard
                .administrators
                .push(ctx.accounts.new_admin.key()),
            Principle::CollectTreasury => admin_account
                .principles
                .collect_treasury
                .administrators
                .push(ctx.accounts.new_admin.key()),
            Principle::KaminoLendStandard => admin_account
                .principles
                .kamino_lend_standard
                .administrators
                .push(ctx.accounts.new_admin.key()),
            Principle::ExponentCore => admin_account
                .principles
                .exponent_core
                .administrators
                .push(ctx.accounts.new_admin.key()),
            Principle::ChangeStatusFlags => admin_account
                .principles
                .change_status_flags
                .administrators
                .push(ctx.accounts.new_admin.key()),
            Principle::JitoRestaking => admin_account
                .principles
                .jito_restaking
                .administrators
                .push(ctx.accounts.new_admin.key()),
        }
        Ok(())
    }

    pub fn remove_principle_admin(
        ctx: Context<RemovePrincipleAdmin>,
        principle: Principle,
    ) -> Result<()> {
        let admin_account = &mut ctx.accounts.admin_account;
        admin_account.is_uber_admin(&ctx.accounts.uber_admin.key())?;
        match principle {
            Principle::MarginfiStandard => {
                if let Some(index) = admin_account
                    .principles
                    .marginfi_standard
                    .administrators
                    .iter()
                    .position(|a| a == &ctx.accounts.admin_to_remove.key())
                {
                    admin_account
                        .principles
                        .marginfi_standard
                        .administrators
                        .remove(index);
                }
            }
            Principle::CollectTreasury => {
                if let Some(index) = admin_account
                    .principles
                    .collect_treasury
                    .administrators
                    .iter()
                    .position(|a| a == &ctx.accounts.admin_to_remove.key())
                {
                    admin_account
                        .principles
                        .collect_treasury
                        .administrators
                        .remove(index);
                }
            }
            Principle::KaminoLendStandard => {
                if let Some(index) = admin_account
                    .principles
                    .kamino_lend_standard
                    .administrators
                    .iter()
                    .position(|a| a == &ctx.accounts.admin_to_remove.key())
                {
                    admin_account
                        .principles
                        .kamino_lend_standard
                        .administrators
                        .remove(index);
                }
            }
            Principle::ExponentCore => {
                if let Some(index) = admin_account
                    .principles
                    .exponent_core
                    .administrators
                    .iter()
                    .position(|a| a == &ctx.accounts.admin_to_remove.key())
                {
                    admin_account
                        .principles
                        .exponent_core
                        .administrators
                        .remove(index);
                }
            }
            Principle::ChangeStatusFlags => {
                if let Some(index) = admin_account
                    .principles
                    .change_status_flags
                    .administrators
                    .iter()
                    .position(|a| a == &ctx.accounts.admin_to_remove.key())
                {
                    admin_account
                        .principles
                        .change_status_flags
                        .administrators
                        .remove(index);
                }
            }
            Principle::JitoRestaking => {
                if let Some(index) = admin_account
                    .principles
                    .jito_restaking
                    .administrators
                    .iter()
                    .position(|a| a == &ctx.accounts.admin_to_remove.key())
                {
                    admin_account
                        .principles
                        .jito_restaking
                        .administrators
                        .remove(index);
                }
            }
        }
        Ok(())
    }

    pub fn realloc_admin(ctx: Context<ReallocAdmin>, new_len: u16) -> Result<()> {
        let admin_account = &mut ctx.accounts.admin_account;
        admin_account.realloc(new_len as usize, false)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeAdmin<'info> {
    #[account(
        init,
        payer = fee_payer,
        space = Admin::size_of_static(),
        seeds = [&ADMIN_STORE_SEED],
        bump
    )]
    pub admin_account: Account<'info, Admin>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InviteAdmin<'info> {
    #[account(mut)]
    pub admin_account: Account<'info, Admin>,
    pub uber_admin: Signer<'info>,
    /// CHECK:
    pub proposed_admin: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AcceptInvitation<'info> {
    #[account(
        mut,
        constraint = admin_account.proposed_uber_admin == Some(new_uber_admin.key())
    )]
    pub admin_account: Account<'info, Admin>,
    pub new_uber_admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct AddPrincipleAdmin<'info> {
    #[account(
        mut,
        realloc = admin_account.to_account_info().data_len() + 32,
        realloc::payer = fee_payer,
        realloc::zero = false
    )]
    pub admin_account: Account<'info, Admin>,
    /// CHECK:
    pub new_admin: UncheckedAccount<'info>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub uber_admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemovePrincipleAdmin<'info> {
    #[account(
        mut,
        realloc = admin_account.to_account_info().data_len() - 32,
        realloc::payer = uber_admin,
        realloc::zero = false
    )]
    pub admin_account: Account<'info, Admin>,
    /// CHECK:
    pub admin_to_remove: UncheckedAccount<'info>,
    #[account(mut)]
    pub uber_admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(new_len: u16)]
pub struct ReallocAdmin<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// CHECK:
    #[account(mut)]
    pub admin_account: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct Admin {
    pub uber_admin: Pubkey,
    pub proposed_uber_admin: Option<Pubkey>,
    pub principles: Principles,
}

impl Admin {
    pub fn is_uber_admin(&self, pubkey: &Pubkey) -> Result<()> {
        if self.uber_admin != *pubkey {
            return err!(ErrorCode::Unauthorized);
        }
        Ok(())
    }

    pub fn size_of_static() -> usize {
        8 + // discriminator
        32 + // uber_admin
        1 + 32 + // optional + proposed_uber_admin
        4 + // principles vec 1
        4 + // principles vec 2
        4 + // principles vec 3
        4 + // principles vec 4
        4 + // principles vec 5
        4 // principles vec 6
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Principles {
    pub marginfi_standard: PrincipleDetails,
    pub collect_treasury: PrincipleDetails,
    pub kamino_lend_standard: PrincipleDetails,
    pub exponent_core: PrincipleDetails,
    pub change_status_flags: PrincipleDetails,
    pub jito_restaking: PrincipleDetails,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PrincipleDetails {
    pub administrators: Vec<Pubkey>,
}

impl PrincipleDetails {
    pub fn is_admin(&self, pubkey: &Pubkey) -> Result<()> {
        if !self.administrators.contains(pubkey) {
            return err!(ErrorCode::Unauthorized);
        }
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum Principle {
    MarginfiStandard,
    CollectTreasury,
    KaminoLendStandard,
    ExponentCore,
    ChangeStatusFlags,
    JitoRestaking,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("There is no proposed admin")]
    NoProposedAdmin,
}