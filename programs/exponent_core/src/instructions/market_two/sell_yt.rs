use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_2022::Transfer, token_interface::TokenAccount};

use crate::{
    error::ExponentCoreError,
    instructions::self_cpi::{do_cpi_merge, do_cpi_trade_pt, MergeAccounts, TradePtAccounts},
    state::MarketTwo,
    util::token_transfer,
    STATUS_CAN_SELL_YT,
};

#[event_cpi]
#[derive(Accounts)]
pub struct SellYt<'info> {
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

    #[account(mut)]
    pub token_yt_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_pt_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_sy_trader: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Account owned by market
    /// This is a temporary account that receives SY tokens from the user, after which it transfers them to the SY program
    #[account(mut)]
    pub token_sy_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    /// PT liquidity account
    #[account(mut)]
    pub token_pt_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: constrained by market
    pub address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: Checked by trade_pt
    #[account(mut)]
    pub token_fee_treasury_sy: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: constrained by self-cpi to merge
    #[account(
        mut,
        address = market.vault
    )]
    pub vault: UncheckedAccount<'info>,

    /// CHECK: constrained by self-cpi to merge
    /// Vault-robot authority
    #[account(mut)]
    pub authority_vault: UncheckedAccount<'info>,

    /// CHECK: constrained by self-cpi to merge
    /// Vault-robot owned escrow account for SY
    #[account(mut)]
    pub token_sy_escrow_vault: UncheckedAccount<'info>,

    /// CHECK: constrained by self-cpi to merge
    #[account(mut)]
    pub mint_yt: UncheckedAccount<'info>,

    /// CHECK: constrained by self-cpi to merge
    #[account(mut)]
    pub mint_pt: UncheckedAccount<'info>,

    /// CHECK: constrained by self-cpi to merge
    /// ALT owned by vault
    pub address_lookup_table_vault: UncheckedAccount<'info>,

    /// CHECK: constrained by self-cpi to merge
    /// Yield position owned by vault robot
    #[account(mut)]
    pub yield_position_vault: UncheckedAccount<'info>,

    /// CHECK: constrain by market
    pub sy_program: UncheckedAccount<'info>,
}

impl<'i> SellYt<'i> {
    fn borrow_pt_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.token_pt_escrow.to_account_info(),
            to: self.token_pt_trader.to_account_info(),
            authority: self.market.to_account_info(),
        }
    }

    /// Construct the accounts for the merge instruction
    fn merge_py_account(&self) -> MergeAccounts<'i> {
        MergeAccounts {
            owner: self.trader.to_account_info(),
            authority: self.authority_vault.to_account_info(),
            vault: self.vault.to_account_info(),
            sy_dst: self.token_sy_trader.to_account_info(),
            escrow_sy: self.token_sy_escrow_vault.to_account_info(),
            yt_src: self.token_yt_trader.to_account_info(),
            pt_src: self.token_pt_trader.to_account_info(),
            mint_yt: self.mint_yt.to_account_info(),
            mint_pt: self.mint_pt.to_account_info(),
            token_program: self.token_program.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            address_lookup_table: self.address_lookup_table_vault.to_account_info(),
            yield_position: self.yield_position_vault.to_account_info(),
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

    fn repay_pt_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.token_pt_trader.to_account_info(),
            to: self.token_pt_escrow.to_account_info(),
            authority: self.trader.to_account_info(),
        }
    }

    fn borrow_pt_context(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.borrow_pt_accounts(),
        )
    }

    fn repay_pt_context(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.repay_pt_accounts(),
        )
    }

    fn borrow_pt(&self, amount: u64) -> Result<()> {
        let cpi_ctx = self.borrow_pt_context();
        token_transfer(cpi_ctx.with_signer(&[&self.market.signer_seeds()]), amount)
    }

    fn repay_pt(&self, amount: u64) -> Result<()> {
        let cpi_ctx = self.repay_pt_context();
        token_transfer(cpi_ctx, amount)
    }

    /// Perform the CPI to buy PT with SY
    /// Returns the amount of SY spent
    fn do_cpi_buy_pt(
        &self,
        remaining_accounts: &[AccountInfo<'i>],
        amount: i64,
        sy_constraint: i64,
    ) -> Result<u64> {
        let evt = do_cpi_trade_pt(
            self.trade_pt_accounts(),
            remaining_accounts,
            amount,
            sy_constraint,
        )?;
        Ok(evt.net_trader_sy.abs() as u64)
    }

    pub fn validate(&self) -> Result<()> {
        require!(
            self.market.check_status_flags(STATUS_CAN_SELL_YT),
            ExponentCoreError::SellingYtDisabled
        );

        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler<'i>(
    ctx: Context<'_, '_, '_, 'i, SellYt<'i>>,
    yt_in: u64,
    min_sy_out: u64,
) -> Result<SellYtEvent> {
    // The market must have at least twice the amount of PT as YT in order to sell YT
    // This because the trader borrows the YT amount from the market
    // And then buys the YT amount back from the market
    // So they take 2x the YT amount of PT from the market
    assert!(
        ctx.accounts.market.financials.pt_balance >= yt_in * 2,
        "insufficient PT liquidity in the market"
    );

    // Flash borrow PT for the user
    // This does not mutate the market financials' PT balance
    ctx.accounts
        .borrow_pt(yt_in)
        .expect("Insufficient PT liquidity in the market");

    // Do the merge of PT & YT into SY
    // Get the amount of SY received from the merge operation
    let sy_recv = do_cpi_merge(
        ctx.accounts.merge_py_account(),
        ctx.remaining_accounts,
        yt_in,
    )
    .map(|res| res.amount_sy_out)
    .expect("Merge failed");

    // the maximum spend of SY is the amount received from the merge operation
    let sy_constraint: i64 = sy_recv
        .try_into()
        .expect("overflow converting sy u64 to i64");

    let sy_constraint = sy_constraint * -1;

    ctx.accounts.market.is_current_flash_swap = true;
    ctx.accounts.market.exit(&crate::ID)?;

    // perform the purchase of PT with the SY
    let sy_spent = ctx
        .accounts
        .do_cpi_buy_pt(
            ctx.remaining_accounts,
            yt_in.try_into().expect("overflow converting yt_in to i64"),
            sy_constraint,
        )
        .expect("Trade PT failed");

    // must reload the market to get the updated financials
    ctx.accounts.market.reload()?;

    ctx.accounts.market.is_current_flash_swap = false;

    // The leftover SY is the difference between the amount received from Merge and spent SY buying back PT
    let sy_leftover = sy_recv
        .checked_sub(sy_spent)
        .expect("spent more SY than received from merge");

    // check that the user kept at least min_sy_out
    assert!(sy_leftover >= min_sy_out, "did not meet min_sy_out");

    ctx.accounts
        .repay_pt(yt_in)
        .expect("Insufficient balance to repay PT");

    let event = SellYtEvent {
        trader: ctx.accounts.trader.key(),
        market: ctx.accounts.market.key(),
        token_yt_trader: ctx.accounts.token_yt_trader.key(),
        token_pt_trader: ctx.accounts.token_pt_trader.key(),
        token_sy_trader: ctx.accounts.token_sy_trader.key(),
        token_sy_escrow: ctx.accounts.token_sy_escrow.key(),
        token_pt_escrow: ctx.accounts.token_pt_escrow.key(),
        amount_yt_in: yt_in,
        amount_sy_received_from_merge: sy_recv,
        amount_sy_spent_buying_pt: sy_spent,
        amount_sy_out: sy_leftover,
        pt_borrowed_and_repaid: yt_in,
        timestamp: Clock::get()?.unix_timestamp,
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct SellYtEvent {
    pub trader: Pubkey,
    pub market: Pubkey,
    pub token_yt_trader: Pubkey,
    pub token_pt_trader: Pubkey,
    pub token_sy_trader: Pubkey,
    pub token_sy_escrow: Pubkey,
    pub token_pt_escrow: Pubkey,
    pub amount_yt_in: u64,
    pub amount_sy_received_from_merge: u64,
    pub amount_sy_spent_buying_pt: u64,
    pub amount_sy_out: u64,
    pub pt_borrowed_and_repaid: u64,
    pub timestamp: i64,
}