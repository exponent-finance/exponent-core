use crate::{
    instructions::self_cpi,
    state::*,
    utils::{sy_cpi, sy_to_py_ceil},
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, WrapperBuyPt<'info>>,
    pt_amount: u64,
    max_base_amount: u64,
    mint_sy_rem_accounts_until: u8,
) -> Result<()> {
    let current_unix_timestamp = Clock::get()?.unix_timestamp;

    // Get SY state
    let sy_state = sy_cpi::do_get_sy_state(
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        &ctx.remaining_accounts[mint_sy_rem_accounts_until as usize..],
        ctx.accounts.sy_program.key(),
    )?;

    // Calculate required SY amount based on market price
    let sy_amount = ctx
        .accounts
        .market
        .financials
        .clone()
        .trade_pt(
            sy_state.exchange_rate,
            pt_amount as i64,
            current_unix_timestamp as u64,
            false,
            ctx.accounts.market.fee_treasury_sy_bps,
        )
        .net_trader_sy;

    // Calculate required base token amount from sy exchange rate and required sy amount
    let required_base_amount = sy_to_py_ceil(sy_state.exchange_rate, sy_amount.abs() as u64);

    // Ensure the required base amount is within the user's specified limit
    assert!(
        required_base_amount <= max_base_amount,
        "Exceeded max base amount"
    );

    // Mint SY tokens
    let mint_base_accounts = &ctx.remaining_accounts[..mint_sy_rem_accounts_until as usize];

    sy_cpi::cpi_mint_sy(
        ctx.accounts.sy_program.key(),
        required_base_amount,
        mint_base_accounts,
        mint_base_accounts.to_vec().to_account_metas(None),
    )?;

    // CPI to trade_pt (buy PT with SY)
    let net_trader_pt = pt_amount as i64;
    let sy_constraint = -(sy_amount.abs() as i64); // Negative because we're spending SY

    self_cpi::do_cpi_trade_pt(
        self_cpi::TradePtAccounts {
            trader: ctx.accounts.buyer.to_account_info(),
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
        &ctx.remaining_accounts[mint_sy_rem_accounts_until as usize..],
        net_trader_pt,
        sy_constraint,
    )?;

    ctx.accounts.market.reload()?;

    emit_cpi!(BuyPtEvent {
        market: ctx.accounts.market.key(),
        buyer: ctx.accounts.buyer.key(),
        base_amount_in: required_base_amount,
        pt_amount_out: pt_amount,
        unix_timestamp: current_unix_timestamp,
    });

    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperBuyPt<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

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
pub struct BuyPtEvent {
    pub market: Pubkey,
    pub buyer: Pubkey,
    pub base_amount_in: u64,
    pub pt_amount_out: u64,
    pub unix_timestamp: i64,
}