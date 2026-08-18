use crate::{instructions::self_cpi, utils::sy_cpi, Vault};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperStrip<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    /// Token account for SY owned by the depositor
    #[account(mut)]
    pub token_sy_depositor: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub vault: Account<'info, Vault>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub escrow_sy: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_yt_depositor: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub token_pt_depositor: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub mint_yt: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub mint_pt: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Checked by strip
    pub authority: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    pub vault_address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    pub token_program: Program<'info, Token>,

    /// CHECK: Checked by deposit_yt
    #[account(mut)]
    pub escrow_yt: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_yt
    #[account(mut)]
    pub user_yield_position: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub vault_robot_yield_position: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    pub sy_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'i> WrapperStrip<'i> {
    fn to_strip_accounts(&self) -> self_cpi::StripAccounts<'i> {
        self_cpi::StripAccounts {
            depositor: self.depositor.to_account_info(),
            authority: self.authority.to_account_info(),
            vault: self.vault.to_account_info(),
            sy_src: self.token_sy_depositor.to_account_info(),
            escrow_sy: self.escrow_sy.to_account_info(),
            yt_dst: self.token_yt_depositor.to_account_info(),
            pt_dst: self.token_pt_depositor.to_account_info(),
            mint_yt: self.mint_yt.to_account_info(),
            mint_pt: self.mint_pt.to_account_info(),
            token_program: self.token_program.to_account_info(),
            address_lookup_table: self.vault_address_lookup_table.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            yield_position: self.vault_robot_yield_position.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
        }
    }

    fn to_deposit_yt_accounts(&self) -> self_cpi::DepositYtAccounts<'i> {
        self_cpi::DepositYtAccounts {
            depositor: self.depositor.to_account_info(),
            vault: self.vault.to_account_info(),
            escrow_yt: self.escrow_yt.to_account_info(),
            token_program: self.token_program.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            address_lookup_table: self.vault_address_lookup_table.to_account_info(),
            yield_position: self.vault_robot_yield_position.to_account_info(),
            system_program: self.system_program.to_account_info(),
            yt_src: self.token_yt_depositor.to_account_info(),
            user_yield_position: self.user_yield_position.to_account_info(),
            program: self.program.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
        }
    }
}

pub fn handler<'i>(
    ctx: Context<'_, '_, '_, 'i, WrapperStrip<'i>>,
    amount_base: u64,
    mint_sy_accounts_until: u8,
) -> Result<()> {
    let mint_sy_rem_accounts = &ctx.remaining_accounts[..mint_sy_accounts_until as usize];
    let interface_cpi_rem_accounts = &ctx.remaining_accounts[mint_sy_accounts_until as usize..];

    // mint SY from base
    let mint_sy_return_data = sy_cpi::cpi_mint_sy(
        ctx.accounts.vault.sy_program,
        amount_base,
        mint_sy_rem_accounts,
        mint_sy_rem_accounts.to_vec().to_account_metas(None),
    )?;

    assert!(
        mint_sy_return_data.sy_out_amount > 0,
        "sy_amount cannot be 0"
    );

    let strip_return_data = self_cpi::do_cpi_strip(
        ctx.accounts.to_strip_accounts(),
        interface_cpi_rem_accounts,
        mint_sy_return_data.sy_out_amount,
    )?;

    ctx.accounts.vault.reload()?;

    self_cpi::do_cpi_deposit_yt(
        ctx.accounts.to_deposit_yt_accounts(),
        interface_cpi_rem_accounts,
        strip_return_data.amount_py_out,
    )?;

    ctx.accounts.vault.reload()?;

    let event = WrapperStripEvent {
        user_address: *ctx.accounts.depositor.key,
        vault_address: ctx.accounts.vault.key(),
        amount_base_in: amount_base,
        amount_sy_stripped: mint_sy_return_data.sy_out_amount,
        amount_py_out: strip_return_data.amount_py_out,
    };

    emit_cpi!(event);

    Ok(())
}

#[event]
pub struct WrapperStripEvent {
    pub user_address: Pubkey,
    pub vault_address: Pubkey,
    pub amount_base_in: u64,
    pub amount_sy_stripped: u64,
    pub amount_py_out: u64,
}
