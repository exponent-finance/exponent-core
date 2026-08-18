use crate::{instructions::self_cpi, utils::sy_cpi, Vault};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperMerge<'info> {
    #[account(mut)]
    pub merger: Signer<'info>,

    /// Token account for SY owned by the merger
    #[account(mut)]
    pub token_sy_merger: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub vault: Account<'info, Vault>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub escrow_sy: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_yt_merger: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub token_pt_merger: InterfaceAccount<'info, TokenAccount>,

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

    /// CHECK: Checked by strip
    #[account(mut)]
    pub vault_robot_yield_position: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    pub sy_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'i> WrapperMerge<'i> {
    fn to_strip_accounts(&self) -> self_cpi::MergeAccounts<'i> {
        self_cpi::MergeAccounts {
            authority: self.authority.to_account_info(),
            vault: self.vault.to_account_info(),
            escrow_sy: self.escrow_sy.to_account_info(),
            mint_yt: self.mint_yt.to_account_info(),
            mint_pt: self.mint_pt.to_account_info(),
            token_program: self.token_program.to_account_info(),
            address_lookup_table: self.vault_address_lookup_table.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            yield_position: self.vault_robot_yield_position.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
            owner: self.merger.to_account_info(),
            pt_src: self.token_pt_merger.to_account_info(),
            yt_src: self.token_yt_merger.to_account_info(),
            sy_dst: self.token_sy_merger.to_account_info(),
        }
    }
}

pub fn handler<'i>(
    ctx: Context<'_, '_, '_, 'i, WrapperMerge<'i>>,
    amount_py: u64,
    redeem_sy_accounts_until: u8,
) -> Result<()> {
    let redeem_sy_rem_accounts = &ctx.remaining_accounts[..redeem_sy_accounts_until as usize];
    let interface_cpi_rem_accounts = &ctx.remaining_accounts[redeem_sy_accounts_until as usize..];

    let merge_return_data = self_cpi::do_cpi_merge(
        ctx.accounts.to_strip_accounts(),
        interface_cpi_rem_accounts,
        amount_py,
    )?;

    ctx.accounts.vault.reload()?;

    let redeem_sy_return_data = sy_cpi::cpi_redeem_sy(
        ctx.accounts.vault.sy_program,
        merge_return_data.amount_sy_out,
        redeem_sy_rem_accounts,
        redeem_sy_rem_accounts.to_vec().to_account_metas(None),
    )?;

    let event = WrapperMergeEvent {
        user_address: *ctx.accounts.merger.key,
        vault_address: ctx.accounts.vault.key(),
        amount_py_in: amount_py,
        amount_sy_redeemed: merge_return_data.amount_sy_out,
        amount_base_out: redeem_sy_return_data.base_out_amount,
    };

    emit_cpi!(event);

    Ok(())
}

#[event]
pub struct WrapperMergeEvent {
    pub user_address: Pubkey,
    pub vault_address: Pubkey,
    pub amount_py_in: u64,
    pub amount_sy_redeemed: u64,
    pub amount_base_out: u64,
}
