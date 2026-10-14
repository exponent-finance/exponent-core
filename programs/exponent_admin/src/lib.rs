use anchor_lang::prelude::*;

declare_id!("3D6ojc8vBfDteLBDTTRznZbZRh7bkEGQaYqNkudoTCBQ");

pub const ADMIN_STORE_SEED: &[u8] = b"admin";

#[program]
pub mod exponent_admin {
    use super::*;

    pub fn initialize_admin(
        ctx: Context<InitializeAdmin>,
        initial_uber_admin: Pubkey,
    ) -> Result<()> {
        // The uber admin must be a key that can actually sign transactions.
        // Initializing it to the program's own ID (as before) bricked the whole
        // admin hierarchy: no key can ever satisfy `is_uber_admin`, so invites,
        // principle admins, and every downstream admin-gated action were dead.
        require!(initial_uber_admin != ID, ErrorCode::InvalidUberAdmin);
        require!(
            initial_uber_admin != Pubkey::default(),
            ErrorCode::InvalidUberAdmin
        );

        let admin_account = &mut ctx.accounts.admin_account;
        admin_account.uber_admin = initial_uber_admin;
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

        let new_admin_key = ctx.accounts.new_admin.key();

        // Reject duplicates and the zero key: duplicates grow the account by 32
        // bytes per push while removal only removes the first occurrence.
        require!(new_admin_key != Pubkey::default(), ErrorCode::InvalidAdmin);
        let already_exists = match principle {
            Principle::HotAdmin => admin_account
                .principles
                .hot_admin
                .administrators
                .contains(&new_admin_key),
            Principle::ColdAdmin => admin_account
                .principles
                .cold_admin
                .administrators
                .contains(&new_admin_key),
            Principle::PauseAdmin => admin_account
                .principles
                .pause_admin
                .administrators
                .contains(&new_admin_key),
            Principle::Reserved1 => admin_account
                .principles
                .reserved1
                .administrators
                .contains(&new_admin_key),
            Principle::Reserved2 => admin_account
                .principles
                .reserved2
                .administrators
                .contains(&new_admin_key),
            Principle::Reserved3 => admin_account
                .principles
                .reserved3
                .administrators
                .contains(&new_admin_key),
        };
        require!(!already_exists, ErrorCode::AdminAlreadyExists);

        match principle {
            Principle::HotAdmin => admin_account
                .principles
                .hot_admin
                .administrators
                .push(new_admin_key),
            Principle::ColdAdmin => admin_account
                .principles
                .cold_admin
                .administrators
                .push(new_admin_key),
            Principle::PauseAdmin => admin_account
                .principles
                .pause_admin
                .administrators
                .push(new_admin_key),
            Principle::Reserved1 => admin_account
                .principles
                .reserved1
                .administrators
                .push(new_admin_key),
            Principle::Reserved2 => admin_account
                .principles
                .reserved2
                .administrators
                .push(new_admin_key),
            Principle::Reserved3 => admin_account
                .principles
                .reserved3
                .administrators
                .push(new_admin_key),
        }
        Ok(())
    }

    pub fn remove_principle_admin(
        ctx: Context<RemovePrincipleAdmin>,
        principle: Principle,
    ) -> Result<()> {
        let admin_account = &mut ctx.accounts.admin_account;
        admin_account.is_uber_admin(&ctx.accounts.uber_admin.key())?;

        let to_remove = ctx.accounts.admin_to_remove.key();

        // Only shrink the account after a confirmed removal. The previous
        // implementation shrank in the account constraint, before knowing
        // whether anything was removed: removing a non-existent admin left the
        // account 32 bytes too small for its own serialized Vec contents.
        let mut removed = false;
        match principle {
            Principle::HotAdmin => {
                if let Some(index) = admin_account
                    .principles
                    .hot_admin
                    .administrators
                    .iter()
                    .position(|a| a == &to_remove)
                {
                    admin_account
                        .principles
                        .hot_admin
                        .administrators
                        .remove(index);
                    removed = true;
                }
            }
            Principle::ColdAdmin => {
                if let Some(index) = admin_account
                    .principles
                    .cold_admin
                    .administrators
                    .iter()
                    .position(|a| a == &to_remove)
                {
                    admin_account
                        .principles
                        .cold_admin
                        .administrators
                        .remove(index);
                    removed = true;
                }
            }
            Principle::PauseAdmin => {
                if let Some(index) = admin_account
                    .principles
                    .pause_admin
                    .administrators
                    .iter()
                    .position(|a| a == &to_remove)
                {
                    admin_account
                        .principles
                        .pause_admin
                        .administrators
                        .remove(index);
                    removed = true;
                }
            }
            Principle::Reserved1 => {
                if let Some(index) = admin_account
                    .principles
                    .reserved1
                    .administrators
                    .iter()
                    .position(|a| a == &to_remove)
                {
                    admin_account
                        .principles
                        .reserved1
                        .administrators
                        .remove(index);
                    removed = true;
                }
            }
            Principle::Reserved2 => {
                if let Some(index) = admin_account
                    .principles
                    .reserved2
                    .administrators
                    .iter()
                    .position(|a| a == &to_remove)
                {
                    admin_account
                        .principles
                        .reserved2
                        .administrators
                        .remove(index);
                    removed = true;
                }
            }
            Principle::Reserved3 => {
                if let Some(index) = admin_account
                    .principles
                    .reserved3
                    .administrators
                    .iter()
                    .position(|a| a == &to_remove)
                {
                    admin_account
                        .principles
                        .reserved3
                        .administrators
                        .remove(index);
                    removed = true;
                }
            }
        }

        if removed {
            // One Pubkey (32 bytes) was removed, so shrink by exactly that much,
            // but never below the static size of the account.
            let admin_account_info = admin_account.to_account_info();
            let current_size = admin_account_info.data_len();
            let new_size = current_size.saturating_sub(32);
            require!(
                new_size >= Admin::size_of_static(),
                ErrorCode::AccountTooSmall
            );
            admin_account_info.realloc(new_size, false)?;
        }

        Ok(())
    }

    pub fn realloc_admin(ctx: Context<ReallocAdmin>, additional_bytes: u16) -> Result<()> {
        let admin_account = &mut ctx.accounts.admin_account;
        let current_size = admin_account.to_account_info().data_len();
        let new_size = current_size + additional_bytes as usize;

        // Bound the growth so a single call cannot arbitrarily inflate the
        // account (and so repeated calls cannot walk the rent requirements up).
        require!(new_size <= current_size + 1024, ErrorCode::ReallocTooLarge);

        admin_account.realloc(new_size, false)?;

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
    #[account(
        mut,
        seeds = [&ADMIN_STORE_SEED],
        bump
    )]
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
        seeds = [&ADMIN_STORE_SEED],
        bump,
        constraint = admin_account.proposed_uber_admin == Some(new_uber_admin.key())
    )]
    pub admin_account: Account<'info, Admin>,
    pub new_uber_admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct AddPrincipleAdmin<'info> {
    #[account(
        mut,
        seeds = [&ADMIN_STORE_SEED],
        bump,
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
    // The account is shrunk inside the handler, only after a confirmed removal.
    #[account(
        mut,
        seeds = [&ADMIN_STORE_SEED],
        bump
    )]
    pub admin_account: Account<'info, Admin>,
    /// CHECK:
    pub admin_to_remove: UncheckedAccount<'info>,
    #[account(mut)]
    pub uber_admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(additional_bytes: u16)]
pub struct ReallocAdmin<'info> {
    // Only the uber admin may grow the admin account, and the target must be the
    // canonical admin PDA. Previously any signer could realloc any writable
    // account in the transaction.
    #[account(
        mut,
        seeds = [&ADMIN_STORE_SEED],
        bump,
        constraint = admin_account.uber_admin == signer.key()
    )]
    pub admin_account: Account<'info, Admin>,
    pub signer: Signer<'info>,
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
    pub hot_admin: PrincipleDetails,
    pub cold_admin: PrincipleDetails,
    pub pause_admin: PrincipleDetails,
    pub reserved1: PrincipleDetails,
    pub reserved2: PrincipleDetails,
    pub reserved3: PrincipleDetails,
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
    HotAdmin,
    ColdAdmin,
    PauseAdmin,
    Reserved1,
    Reserved2,
    Reserved3,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("There is no proposed admin")]
    NoProposedAdmin,
    #[msg("Invalid admin key: must not be the zero address or the program itself")]
    InvalidUberAdmin,
    #[msg("Invalid admin key")]
    InvalidAdmin,
    #[msg("Admin already exists")]
    AdminAlreadyExists,
    #[msg("Account too small to shrink any further")]
    AccountTooSmall,
    #[msg("Reallocation too large")]
    ReallocTooLarge,
}
