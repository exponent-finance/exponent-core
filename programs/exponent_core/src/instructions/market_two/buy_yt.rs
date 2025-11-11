use crate::{
    error::ExponentCoreError,
    instructions::self_cpi::{do_cpi_strip, do_cpi_trade_pt, StripAccounts, TradePtAccounts},
    state::*,
    util::token_transfer,
    utils::{do_deposit_sy, do_get_sy_state, do_withdraw_sy, py_to_sy_ceil},
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};
use precise_number::Number;

#[event_cpi]
#[derive(Accounts)]
pub struct BuyYt<'info> {
    #[account(mut)]
    pub trader: Signer<'info>,

    #[account(
        mut,
        has_one = vault,
        has_one = address_lookup_table,
        has_one = sy_program,
        has_one = token_sy_escrow,
        has_one = token_pt_escrow,
        has_one = token_fee_treasury_sy
    )]
    pub market: Box<Account<'info, MarketTwo>>,

    /// Source account for the trader's SY tokens
    #[account(mut)]
    pub token_sy_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Destination for receiving YT tokens
    #[account(mut)]
    pub token_yt_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_pt_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Temporary SY account owned by market
    #[account(mut)]
    pub token_sy_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    /// PT liquidity owned by market
    #[account(mut)]
    pub token_pt_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Checked by trade_pt
    #[account(mut)]
    pub token_fee_treasury_sy: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: constrained by market
    pub address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: constrained by market
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: constrained by strip CPI
    #[account(mut)]
    pub vault_authority: UncheckedAccount<'info>,

    /// CHECK: constrained by strip CPI
    #[account(
        mut,
        address = market.vault
    )]
    pub vault: UncheckedAccount<'info>,

    /// CHECK: constrained by strip CPI
    #[account(mut)]
    pub token_sy_escrow_vault: UncheckedAccount<'info>,

    /// CHECK: constrained by strip CPI
    #[account(mut)]
    pub mint_yt: UncheckedAccount<'info>,

    /// CHECK: constrained by strip CPI
    #[account(mut)]
    pub mint_pt: UncheckedAccount<'info>,

    /// CHECK: constrained by strip CPI
    pub address_lookup_table_vault: UncheckedAccount<'info>,

    /// CHECK: constrained by strip CPI
    #[account(mut)]
    pub yield_position: UncheckedAccount<'info>,
}

impl<'i> BuyYt<'i> {
    fn borrow_sy_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.token_sy_escrow.to_account_info(),
            to: self.token_sy_trader.to_account_info(),
            authority: self.market.to_account_info(),
        }
    }

    fn do_borrow_sy(&self, amount: u64) -> Result<()> {
        token_transfer(
            CpiContext::new(
                self.token_program.to_account_info(),
                self.borrow_sy_accounts(),
            )
            .with_signer(&[&self.market.signer_seeds()]),
            amount,
        )
    }

    fn repay_sy_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.token_sy_trader.to_account_info(),
            to: self.token_sy_escrow.to_account_info(),
            authority: self.trader.to_account_info(),
        }
    }

    fn strip_sy_accounts(&self) -> StripAccounts<'i> {
        StripAccounts {
            depositor: self.trader.to_account_info(),
            authority: self.vault_authority.to_account_info(),
            vault: self.vault.to_account_info(),
            sy_src: self.token_sy_trader.to_account_info(),
            escrow_sy: self.token_sy_escrow_vault.to_account_info(),
            yt_dst: self.token_yt_trader.to_account_info(),
            pt_dst: self.token_pt_trader.to_account_info(),
            mint_yt: self.mint_yt.to_account_info(),
            mint_pt: self.mint_pt.to_account_info(),
            address_lookup_table: self.address_lookup_table_vault.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            yield_position: self.yield_position.to_account_info(),
            token_program: self.token_program.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
        }
    }

    fn trade_pt_accounts(&self) -> TradePtAccounts<'i> {
        TradePtAccounts {
            trader: self.trader.to_account_info(),
            market: self.market.to_account_info(),
            token_sy_trader: self.token_sy_trader.to_account_info(),
            token_pt_trader: self.token_pt_trader.to_account_info(),
            token_sy_escrow: self.token_sy_escrow.to_account_info(),
            token_pt_escrow: self.token_pt_escrow.to_account_info(),
            address_lookup_table: self.address_lookup_table.to_account_info(),
            token_program: self.token_program.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            token_fee_treasury_sy: self.token_fee_treasury_sy.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
        }
    }

    fn do_repay_sy(&self, amount: u64) -> Result<()> {
        token_transfer(
            CpiContext::new(
                self.token_program.to_account_info(),
                self.repay_sy_accounts(),
            ),
            amount,
        )
    }

    pub fn validate(&self) -> Result<()> {
        require!(
            self.market.check_status_flags(STATUS_CAN_BUY_YT),
            ExponentCoreError::BuyingYtDisabled
        );

        Ok(())
    }
}

/// To buy YT with SY, the following steps are taken:
/// - Set the max amount of SY you will spend
/// - Borrow an amount of SY from the pool
/// - Strip the SY into PT & YT (exactly getting the yt_out you expect)
/// - Sell the PT into the AMM for SY
/// - Repay the borrowed SY, keeping behind enough SY that does not exceed the max_sy_in
#[access_control(ctx.accounts.validate())]
pub fn handler<'i>(
    ctx: Context<'_, '_, '_, 'i, BuyYt<'i>>,
    sy_in: u64,
    yt_out: u64,
) -> Result<BuyYtEvent> {
    // get exchange rate for SY
    let sy_exchange_rate = do_get_sy_state(
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
    )
    .map(|x| x.exchange_rate)
    .expect("failed to get sy state");

    // calculate how much SY must be stripped to get the target YT
    // Use ceiling division
    let sy_to_strip = py_to_sy_ceil(sy_exchange_rate, yt_out);

    assert!(
        sy_to_strip > sy_in,
        "sy_to_strip must be greater than max_sy_in"
    );

    let sy_to_borrow = sy_to_strip - sy_in;

    // Check borrow amount is less than target strip amount
    assert!(
        sy_to_borrow < sy_to_strip,
        "sy_to_borrow must be less than sy_to_strip"
    );

    // The total amount of SY spent by the trader is the amount of SY stripped minus the amount of SY borrowed
    let net_sy_spend = sy_to_strip - sy_to_borrow;

    // Check slippage
    assert!(
        net_sy_spend <= sy_in,
        "net_sy_spend must be less than or equal to max_sy_in"
    );

    // =========== Perform the borrow ===========

    // First, withdraw the intended borrowed SY from the market's SY balance in the SY program
    // this does not mutate the market account
    do_withdraw_sy(
        sy_to_borrow,
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        &ctx.accounts.to_account_infos(),
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.market.signer_seeds()],
    )
    .expect("failed to withdraw sy from CPI");

    // borrow the SY to the trader's token account
    ctx.accounts
        .do_borrow_sy(sy_to_borrow)
        .expect("borrow SY failed");

    // ========== Done with the borrow ===========
    // strip sy_to_strip
    let pt_out = do_cpi_strip(
        ctx.accounts.strip_sy_accounts(),
        ctx.remaining_accounts,
        sy_to_strip,
    )
    .map(|x| x.amount_py_out)
    .expect("Strip failed");

    ctx.accounts.market.is_current_flash_swap = true;
    ctx.accounts.market.exit(&crate::ID)?;

    // sell all PT from the strip operation
    // constrain it so that the trader receives at least the borrowed amount of SY
    // although, the trader should receive the exact amount of SY they expect, since we ran a simulated sale of PT above
    let market_pre_sy_balance = ctx.accounts.market.financials.sy_balance;
    let pt_sell: i64 = pt_out.try_into().unwrap();
    let res = do_cpi_trade_pt(
        ctx.accounts.trade_pt_accounts(),
        ctx.remaining_accounts,
        pt_sell * -1i64,
        sy_to_borrow.try_into().unwrap(),
    )
    .expect("sell PT failed");
    let received_sy = res.net_trader_sy as u64;

    assert!(
        received_sy >= sy_to_borrow,
        "received SY must be greater than or equal to borrowed SY"
    );
    let sy_leftover = received_sy.checked_sub(sy_to_borrow).unwrap();

    // Reload the market state before mutating
    ctx.accounts.market.reload()?;

    ctx.accounts.market.is_current_flash_swap = false;

    let market_post_sy_balance = ctx.accounts.market.financials.sy_balance;

    assert!(
        market_post_sy_balance <= market_pre_sy_balance - received_sy,
        "market SY post balance must be less than or equal to pre-trade SY balance, because of the treasury fee"
    );

    // Gift the leftover Dust to the Market
    let sy_to_repay = sy_to_borrow + sy_leftover;

    // repay the borrowed SY to the Market's escrow account
    ctx.accounts
        .do_repay_sy(sy_to_repay)
        .expect("repay SY failed");

    // Deposit the SY into the SY program from the Market's escrow account
    do_deposit_sy(
        sy_to_repay,
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        &ctx.accounts.to_account_infos(),
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.market.signer_seeds()],
    )
    .expect("CPI deposit SY failed");

    let event = BuyYtEvent {
        trader: ctx.accounts.trader.key(),
        market: ctx.accounts.market.key(),
        token_sy_trader: ctx.accounts.token_sy_trader.key(),
        token_yt_trader: ctx.accounts.token_yt_trader.key(),
        token_pt_trader: ctx.accounts.token_pt_trader.key(),
        token_sy_escrow: ctx.accounts.token_sy_escrow.key(),
        token_pt_escrow: ctx.accounts.token_pt_escrow.key(),
        max_sy_in: sy_in,
        yt_out,
        sy_exchange_rate,
        sy_to_strip,
        sy_borrowed: sy_to_borrow,
        pt_out,
        sy_repaid: sy_to_repay,
        timestamp: Clock::get()?.unix_timestamp,
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct BuyYtEvent {
    pub trader: Pubkey,
    pub market: Pubkey,
    pub token_sy_trader: Pubkey,
    pub token_yt_trader: Pubkey,
    pub token_pt_trader: Pubkey,
    pub token_sy_escrow: Pubkey,
    pub token_pt_escrow: Pubkey,
    pub max_sy_in: u64,
    pub yt_out: u64,
    pub sy_exchange_rate: Number,
    pub sy_to_strip: u64,
    pub sy_borrowed: u64,
    pub pt_out: u64,
    pub sy_repaid: u64,
    pub timestamp: i64,
}