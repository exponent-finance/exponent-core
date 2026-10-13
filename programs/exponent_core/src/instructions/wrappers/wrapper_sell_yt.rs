use crate::{
    instructions::self_cpi,
    state::*,
    utils::{py_to_sy_ceil, sy_cpi},
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, WrapperSellYt<'info>>,
    yt_amount: u64,
    min_base_amount: u64,
    redeem_sy_accounts_until: u8,
) -> Result<()> {
    // Get SY state
    let sy_state = sy_cpi::do_get_sy_state(
        &ctx.accounts.market_address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        &ctx.remaining_accounts[redeem_sy_accounts_until as usize..],
        ctx.accounts.sy_program.key(),
    )?;

    let sy_constraint = py_to_sy_ceil(sy_state.exchange_rate, min_base_amount) as i64;

    // CPI to sell_yt (sell YT for SY)
    let sell_yt_return_data = self_cpi::do_cpi_sell_yt(
        self_cpi::SellYtAccounts {
            trader: ctx.accounts.seller.to_account_info(),
            market: ctx.accounts.market.to_account_info(),
            token_sy_trader: ctx.accounts.token_sy_trader.to_account_info(),
            token_yt_trader: ctx.accounts.token_yt_trader.to_account_info(),
            token_pt_trader: ctx.accounts.token_pt_trader.to_account_info(),
            token_sy_escrow: ctx.accounts.token_sy_escrow.to_account_info(),
            token_pt_escrow: ctx.accounts.token_pt_escrow.to_account_info(),
            address_lookup_table: ctx.accounts.market_address_lookup_table.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            sy_program: ctx.accounts.sy_program.to_account_info(),
            vault: ctx.accounts.vault.to_account_info(),
            token_sy_escrow_vault: ctx.accounts.token_sy_escrow_vault.to_account_info(),
            mint_yt: ctx.accounts.mint_yt.to_account_info(),
            mint_pt: ctx.accounts.mint_pt.to_account_info(),
            address_lookup_table_vault: ctx.accounts.vault_address_lookup_table.to_account_info(),
            yield_position_vault: ctx.accounts.yield_position.to_account_info(),
            authority_vault: ctx.accounts.vault_authority.to_account_info(),
            token_fee_treasury_sy: ctx.accounts.token_fee_treasury_sy.to_account_info(),
            event_authority: ctx.accounts.event_authority.to_account_info(),
            program: ctx.accounts.program.to_account_info(),
        },
        &ctx.remaining_accounts[redeem_sy_accounts_until as usize..],
        yt_amount,
        sy_constraint.abs() as u64,
    )?;

    // Reload the market after the sell_yt CPI in order to persist the correct data
    ctx.accounts.market.reload()?;

    // Redeem only the SY received from selling YT
    let redeem_sy_accounts = &ctx.remaining_accounts[..redeem_sy_accounts_until as usize];
    let redeem_sy_return_data = sy_cpi::cpi_redeem_sy(
        ctx.accounts.sy_program.key(),
        sell_yt_return_data.amount_sy_out,
        redeem_sy_accounts,
        redeem_sy_accounts.to_vec().to_account_metas(None),
    )?;

    // Emit the WrapperSellYtEvent
    emit_cpi!(WrapperSellYtEvent {
        seller: ctx.accounts.seller.key(),
        market: ctx.accounts.market.key(),
        yt_in_amount: yt_amount,
        base_out_amount: redeem_sy_return_data.base_out_amount,
        unix_timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperSellYt<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        mut,
        has_one = sy_program
    )]
    pub market: Box<Account<'info, MarketTwo>>,

    #[account(mut)]
    pub token_sy_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_yt_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_pt_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Checked by sell_yt
    #[account(mut)]
    pub token_sy_escrow: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    #[account(mut)]
    pub token_pt_escrow: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    pub market_address_lookup_table: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: Checked by market
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    #[account(mut)]
    pub vault_authority: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    #[account(mut)]
    pub vault: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    #[account(mut)]
    pub token_sy_escrow_vault: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    #[account(mut)]
    pub mint_yt: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    #[account(mut)]
    pub mint_pt: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    pub vault_address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    #[account(mut)]
    pub yield_position: UncheckedAccount<'info>,

    /// CHECK: Checked by sell_yt
    #[account(mut)]
    pub token_fee_treasury_sy: UncheckedAccount<'info>,
}

#[event]
pub struct WrapperSellYtEvent {
    pub seller: Pubkey,
    pub market: Pubkey,
    pub yt_in_amount: u64,
    pub base_out_amount: u64,
    pub unix_timestamp: i64,
}
