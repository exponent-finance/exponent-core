use crate::{error::ExponentCoreError, state::*, utils::do_withdraw_sy};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_2022::{burn, Burn},
};
#[allow(deprecated)]
use anchor_spl::{
    token_2022::{transfer, Transfer},
    token_interface::{Mint, TokenAccount},
};

#[event_cpi]
#[derive(Accounts)]
pub struct WithdrawLiquidity<'info> {
    #[account(mut)]
    pub withdrawer: Signer<'info>,

    #[account(
        mut,
        has_one = token_pt_escrow,
        has_one = token_sy_escrow,
        has_one = mint_lp,
        has_one = sy_program,
        has_one = address_lookup_table
    )]
    pub market: Box<Account<'info, MarketTwo>>,

    #[account(mut)]
    pub token_pt_dst: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_sy_dst: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Market PT liquidity account
    #[account(mut)]
    pub token_pt_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Market-owned interchange account for SY
    #[account(mut)]
    pub token_sy_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_lp_src: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub mint_lp: Box<InterfaceAccount<'info, Mint>>,

    /// Address lookup table for vault
    /// CHECK: constrained by market
    pub address_lookup_table: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: constrained by market
    pub sy_program: UncheckedAccount<'info>,
}

impl<'i> WithdrawLiquidity<'i> {
    fn to_transfer_pt_out_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.token_pt_escrow.to_account_info(),
            to: self.token_pt_dst.to_account_info(),
            authority: self.market.to_account_info(),
        }
    }

    fn to_transfer_sy_out_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.token_sy_escrow.to_account_info(),
            to: self.token_sy_dst.to_account_info(),
            authority: self.market.to_account_info(),
        }
    }

    fn to_burn_lp_accounts(&self) -> Burn<'i> {
        Burn {
            mint: self.mint_lp.to_account_info(),
            from: self.token_lp_src.to_account_info(),
            authority: self.withdrawer.to_account_info(),
        }
    }

    fn to_transfer_pt_out_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_transfer_pt_out_accounts(),
        )
    }

    fn to_transfer_sy_out_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_transfer_sy_out_accounts(),
        )
    }

    fn to_burn_lp_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Burn<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_burn_lp_accounts(),
        )
    }

    fn do_transfer_pt_out(&self, amount: u64) -> Result<()> {
        #[allow(deprecated)]
        transfer(
            self.to_transfer_pt_out_ctx()
                .with_signer(&[&self.market.signer_seeds()]),
            amount,
        )
    }

    fn do_transfer_sy_out(&self, amount: u64) -> Result<()> {
        #[allow(deprecated)]
        transfer(
            self.to_transfer_sy_out_ctx()
                .with_signer(&[&self.market.signer_seeds()]),
            amount,
        )
    }

    fn do_transfers_out(&self, pt_out: u64, sy_out: u64) -> Result<()> {
        self.do_transfer_pt_out(pt_out)?;
        self.do_transfer_sy_out(sy_out)?;
        Ok(())
    }

    fn do_burn_lp(&self, amount: u64) -> Result<()> {
        burn(self.to_burn_lp_ctx(), amount)
    }

    pub fn validate(&mut self, lp_in: u64) -> Result<()> {
        require!(
            self.market
                .check_status_flags(STATUS_CAN_WITHDRAW_LIQUIDITY),
            ExponentCoreError::WithdrawingLiquidityDisabled
        );

        self.market.liquidity_net_balance_limits.verify_limits(
            Clock::get()?.unix_timestamp as u32,
            self.mint_lp.supply,
            -(lp_in as i64),
        )?;

        Ok(())
    }
}

#[access_control(ctx.accounts.validate(lp_in))]
pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, WithdrawLiquidity<'info>>,
    lp_in: u64,
    min_pt_out: u64,
    min_sy_out: u64,
) -> Result<WithdrawLiquidityEvent> {
    let r = ctx
        .accounts
        .market
        .financials
        .rm_liquidity(lp_in, ctx.accounts.mint_lp.supply);

    if r.sy_out < min_sy_out {
        return Err(ExponentCoreError::MinSyOutNotMet.into());
    }

    if r.pt_out < min_pt_out {
        return Err(ExponentCoreError::MinPtOutNotMet.into());
    }

    ctx.accounts.do_burn_lp(lp_in)?;

    // Then transfer SY tokens into sy_program
    do_withdraw_sy(
        r.sy_out,
        &ctx.accounts.address_lookup_table.to_account_info(),
        &ctx.accounts.market.cpi_accounts,
        &ctx.accounts.to_account_infos(),
        &ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.market.signer_seeds()],
    )?;

    ctx.accounts.do_transfers_out(r.pt_out, r.sy_out)?;

    let event = WithdrawLiquidityEvent {
        withdrawer: ctx.accounts.withdrawer.key(),
        market: ctx.accounts.market.key(),
        token_pt_dst: ctx.accounts.token_pt_dst.key(),
        token_sy_dst: ctx.accounts.token_sy_dst.key(),
        token_pt_escrow: ctx.accounts.token_pt_escrow.key(),
        token_sy_escrow: ctx.accounts.token_sy_escrow.key(),
        token_lp_src: ctx.accounts.token_lp_src.key(),
        mint_lp: ctx.accounts.mint_lp.key(),
        lp_in,
        pt_out: r.pt_out,
        sy_out: r.sy_out,
        new_lp_supply: ctx.accounts.mint_lp.supply - lp_in,
        timestamp: Clock::get()?.unix_timestamp,
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct WithdrawLiquidityEvent {
    pub withdrawer: Pubkey,
    pub market: Pubkey,
    pub token_pt_dst: Pubkey,
    pub token_sy_dst: Pubkey,
    pub token_pt_escrow: Pubkey,
    pub token_sy_escrow: Pubkey,
    pub token_lp_src: Pubkey,
    pub mint_lp: Pubkey,
    pub lp_in: u64,
    pub pt_out: u64,
    pub sy_out: u64,
    pub new_lp_supply: u64,
    pub timestamp: i64,
}