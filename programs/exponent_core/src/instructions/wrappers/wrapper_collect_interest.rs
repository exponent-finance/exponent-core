use crate::{
    instructions::self_cpi::{self, CollectInterestAccounts},
    utils::cpi_redeem_sy,
    Vault,
};
use amount_value::Amount;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::*;

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperCollectInterest<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    /// CHECK: constrained by vault
    /// The authority owned by the vault for minting PT/YT
    #[account(mut)]
    pub authority: UncheckedAccount<'info>,

    /// CHECK: Checked by collect interest
    #[account(mut)]
    pub vault: Box<Account<'info, Vault>>,

    /// CHECK:
    pub address_lookup_table: UncheckedAccount<'info>,

    /// CHECK:
    #[account(mut)]
    pub escrow_sy: UncheckedAccount<'info>,

    /// CHECK:
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK:
    pub token_program: UncheckedAccount<'info>,

    /// CHECK:
    #[account(mut)]
    pub yield_position: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_sy_dst: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK:
    #[account(mut)]
    pub treasury_sy_token_account: UncheckedAccount<'info>,
}

impl<'i> WrapperCollectInterest<'i> {
    fn to_collect_interest_accounts(&self) -> CollectInterestAccounts<'i> {
        CollectInterestAccounts {
            address_lookup_table: self.address_lookup_table.to_account_info(),
            authority: self.authority.to_account_info(),
            escrow_sy: self.escrow_sy.to_account_info(),
            owner: self.claimer.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            token_program: self.token_program.to_account_info(),
            yield_position: self.yield_position.to_account_info(),
            vault: self.vault.to_account_info(),
            token_sy_dst: self.token_sy_dst.to_account_info(),
            treasury_sy_token_account: self.treasury_sy_token_account.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
        }
    }
}

pub fn handler<'i>(
    ctx: Context<'_, '_, '_, 'i, WrapperCollectInterest<'i>>,
    redeem_sy_accounts_length: u8,
) -> Result<()> {
    let collect_interest_return_data = self_cpi::do_cpi_collect_interest(
        ctx.accounts.to_collect_interest_accounts(),
        &ctx.remaining_accounts[redeem_sy_accounts_length as usize..],
        Amount::All,
    )?;

    ctx.accounts.vault.reload()?;

    let redeem_sy_return_data = cpi_redeem_sy(
        *ctx.accounts.sy_program.key,
        collect_interest_return_data.amount_to_user,
        &ctx.remaining_accounts[..redeem_sy_accounts_length as usize],
        ctx.remaining_accounts[..redeem_sy_accounts_length as usize]
            .to_vec()
            .to_account_metas(None),
    )?;

    let event = WrapperCollectInterestEvent {
        depositor: *ctx.accounts.claimer.key,
        vault: ctx.accounts.vault.key(),
        amount_base_collected: redeem_sy_return_data.base_out_amount,
        unix_timestamp: Clock::get().unwrap().unix_timestamp,
    };

    emit_cpi!(event);

    Ok(())
}

#[event]
pub struct WrapperCollectInterestEvent {
    pub depositor: Pubkey,
    pub vault: Pubkey,
    pub amount_base_collected: u64,
    pub unix_timestamp: i64,
}