use crate::{
    instructions::self_cpi,
    state::*,
    utils::{py_to_sy_ceil, sy_cpi},
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, WrapperSellPt<'info>>,
    amount_pt: u64,
    min_base_amount: u64,
    redeem_sy_rem_accounts_until: u8,
) -> Result<()> {
    // get sy_state
    let sy_state = sy_cpi::do_get_sy_state(
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        &ctx.remaining_accounts[redeem_sy_rem_accounts_until as usize..],
        ctx.accounts.sy_program.key(),
    )?;
    // CPI to trade_pt (sell PT for SY)
    let net_trader_pt = -(amount_pt as i64); // Negative because we're selling PT

    let sy_constraint = py_to_sy_ceil(sy_state.exchange_rate, min_base_amount) as i64;

    let trade_pt_return_data = self_cpi::do_cpi_trade_pt(
        self_cpi::TradePtAccounts {
            trader: ctx.accounts.seller.to_account_info(),
            market: ctx.accounts.market.to_account_info(),
            token_sy_trader: ctx.accounts.token_sy_trader.to_account_info(),
            token_pt_trader: ctx.accounts.token_pt_trader.to_account_info(),
            token_sy_escrow: ctx.accounts.token_sy_escrow.to_account_info(),
            token_pt_escrow: ctx.accounts.token_pt_escrow.to_account_info(),
            address_lookup_table: ctx.accounts.address_lookup_table.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            sy_program: ctx.accounts.sy_program.to_account_info(),
            token_fee_treasury_sy: ctx.accounts.token_fee_treasury_sy.to_account_info(),
            event_authority: ctx.accounts.event_authority.to_account_info(),
            program: ctx.accounts.program.to_account_info(),
        },
        &ctx.remaining_accounts[redeem_sy_rem_accounts_until as usize..],
        net_trader_pt,
        sy_constraint,
    )?;

    ctx.accounts.market.reload()?;

    let redeem_base_accounts = &ctx.remaining_accounts[..redeem_sy_rem_accounts_until as usize];
    let redeem_sy_return_data = sy_cpi::cpi_redeem_sy(
        ctx.accounts.sy_program.key(),
        trade_pt_return_data.net_trader_sy as u64,
        redeem_base_accounts,
        redeem_base_accounts.to_vec().to_account_metas(None),
    )?;

    emit_cpi!(SellPtEvent {
        market: ctx.accounts.market.key(),
        seller: ctx.accounts.seller.key(),
        pt_amount_in: amount_pt,
        base_amount_out: redeem_sy_return_data.base_out_amount,
        unix_timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperSellPt<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        mut,
        has_one = sy_program
    )]
    pub market: Box<Account<'info, MarketTwo>>,

    #[account(mut)]
    pub token_sy_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Checked by trade_pt
    #[account(mut)]
    pub token_pt_trader: UncheckedAccount<'info>,

    /// CHECK: Checked by trade_pt
    #[account(mut)]
    pub token_sy_escrow: UncheckedAccount<'info>,

    /// CHECK: Checked by trade_pt
    #[account(mut)]
    pub token_pt_escrow: UncheckedAccount<'info>,

    /// CHECK: Checked by trade_pt
    pub address_lookup_table: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: Checked by market
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: Checked by trade_pt
    #[account(mut)]
    pub token_fee_treasury_sy: UncheckedAccount<'info>,
}

#[event]
pub struct SellPtEvent {
    pub market: Pubkey,
    pub seller: Pubkey,
    pub pt_amount_in: u64,
    pub base_amount_out: u64,
    pub unix_timestamp: i64,
}
