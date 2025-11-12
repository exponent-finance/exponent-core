use crate::{
    instructions::self_cpi::{self},
    state::*,
    util::now,
    utils::{do_get_sy_state, py_to_sy_ceil, sy_cpi},
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};
use precise_number::Number;

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, WrapperBuyYt<'info>>,
    // exact amount of YT the trader wants to buy
    yt_out: u64,
    // max base amount the trader is willing to spend
    max_base_amount: u64,
    // The number of accounts to be used for minting SY
    mint_sy_accounts_length: u8,
) -> Result<()> {
    let sy_exchange_rate = do_get_sy_state(
        &ctx.accounts.market_address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
    )
    .map(|x| x.exchange_rate)
    .expect("failed to get sy state");

    // simulate the buy_yt instruction to determine how much asset to spend
    let asset_spend = sim_buy_yt(
        yt_out,
        sy_exchange_rate,
        now() as u64,
        &mut ctx.accounts.market.financials.clone(),
        ctx.accounts.market.fee_treasury_sy_bps,
    );

    if asset_spend > max_base_amount {
        panic!("slippage exceeded:asset_spend is greater than max_base_amount");
    }

    let mint_sy_accounts = &ctx.remaining_accounts[..mint_sy_accounts_length as usize];
    let mint_sy_return_data = sy_cpi::cpi_mint_sy(
        ctx.accounts.sy_program.key(),
        asset_spend,
        mint_sy_accounts,
        mint_sy_accounts.to_vec().to_account_metas(None),
    )?;

    let buy_yt_return_data = self_cpi::do_cpi_buy_yt(
        self_cpi::BuyYtAccounts {
            trader: ctx.accounts.buyer.to_account_info(),
            market: ctx.accounts.market.to_account_info(),
            token_sy_trader: ctx.accounts.token_sy_trader.to_account_info(),
            token_yt_trader: ctx.accounts.token_yt_trader.to_account_info(),
            token_pt_trader: ctx.accounts.token_pt_trader.to_account_info(),
            token_sy_escrow: ctx.accounts.token_sy_escrow.to_account_info(),
            token_pt_escrow: ctx.accounts.token_pt_escrow.to_account_info(),
            address_lookup_table: ctx.accounts.market_address_lookup_table.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            sy_program: ctx.accounts.sy_program.to_account_info(),
            vault_authority: ctx.accounts.vault_authority.to_account_info(),
            vault: ctx.accounts.vault.to_account_info(),
            token_sy_escrow_vault: ctx.accounts.token_sy_escrow_vault.to_account_info(),
            mint_yt: ctx.accounts.mint_yt.to_account_info(),
            mint_pt: ctx.accounts.mint_pt.to_account_info(),
            address_lookup_table_vault: ctx.accounts.vault_address_lookup_table.to_account_info(),
            yield_position: ctx.accounts.yield_position.to_account_info(),
            token_fee_treasury_sy: ctx.accounts.token_fee_treasury_sy.to_account_info(),
            event_authority: ctx.accounts.event_authority.to_account_info(),
            program: ctx.accounts.program.to_account_info(),
        },
        &ctx.remaining_accounts[mint_sy_accounts_length as usize..],
        mint_sy_return_data.sy_out_amount,
        yt_out,
    )?;

    ctx.accounts.market.reload()?;
    ctx.accounts.vault.reload()?;

    self_cpi::do_cpi_deposit_yt(
        self_cpi::DepositYtAccounts {
            depositor: ctx.accounts.buyer.to_account_info(),
            vault: ctx.accounts.vault.to_account_info(),
            address_lookup_table: ctx.accounts.vault_address_lookup_table.to_account_info(),
            escrow_yt: ctx.accounts.escrow_yt.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            sy_program: ctx.accounts.sy_program.to_account_info(),
            user_yield_position: ctx.accounts.user_yield_position.to_account_info(),
            yield_position: ctx.accounts.yield_position.to_account_info(),
            yt_src: ctx.accounts.token_yt_trader.to_account_info(),
            event_authority: ctx.accounts.event_authority.to_account_info(),
            program: ctx.accounts.program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
        ctx.remaining_accounts,
        buy_yt_return_data.pt_out,
    )?;

    ctx.accounts.vault.reload()?;

    emit_cpi!(WrapperBuyYtEvent {
        market: ctx.accounts.market.key(),
        buyer: ctx.accounts.buyer.key(),
        base_in_amount: asset_spend,
        yt_out_amount: yt_out,
        unix_timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Returns the amount of asset that needs to be spent to buy the target YT
fn sim_buy_yt(
    yt_out: u64,
    sy_exchange_rate: Number,
    now: u64,
    mf: &mut MarketFinancials,
    fee_treasury_sy_bps: u16,
) -> u64 {
    // calculate how much SY must be stripped to get the target YT
    let target_sy = py_to_sy_ceil(sy_exchange_rate, yt_out);

    // figure out how much SY to borrow
    // by seeing how much SY will be received from selling the PT
    // the amount of PT is equal to the amount of YT desired
    let pt_sell: i64 = yt_out.try_into().unwrap();

    let sy_to_borrow = mf
        .trade_pt(
            sy_exchange_rate,
            -1i64 * pt_sell,
            now.into(),
            true,
            fee_treasury_sy_bps,
        )
        .net_trader_sy
        .abs() as u64;

    let sy_spend = target_sy - sy_to_borrow;

    // convert sy_spend to asset
    (sy_exchange_rate * sy_spend.into()).ceil_u64()
}

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperBuyYt<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        has_one = sy_program
    )]
    pub market: Box<Account<'info, MarketTwo>>,

    #[account(mut)]
    pub token_sy_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub token_yt_trader: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub token_pt_trader: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub token_sy_escrow: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub token_pt_escrow: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    pub market_address_lookup_table: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: Checked by market
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub vault_authority: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub vault: Box<Account<'info, Vault>>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub token_sy_escrow_vault: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub mint_yt: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub mint_pt: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    pub vault_address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub user_yield_position: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_yt
    #[account(mut)]
    pub yield_position: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_yt
    #[account(mut)]
    pub escrow_yt: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_yt
    pub system_program: UncheckedAccount<'info>,

    /// CHECK: Checked by buy_yt
    #[account(mut)]
    pub token_fee_treasury_sy: UncheckedAccount<'info>,
}

impl<'info> WrapperBuyYt<'info> {}

#[event]
pub struct WrapperBuyYtEvent {
    pub market: Pubkey,
    pub buyer: Pubkey,
    pub yt_out_amount: u64,
    pub base_in_amount: u64,
    pub unix_timestamp: i64,
}
