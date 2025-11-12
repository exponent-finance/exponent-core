use crate::{error::ExponentCoreError, state::*, utils::do_deposit_sy};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{mint_to, MintTo},
};
#[allow(deprecated)]
use anchor_spl::{
    token_interface::{transfer, Transfer},
    token_interface::{Mint, TokenAccount},
};

#[event_cpi]
#[derive(Accounts)]
pub struct DepositLiquidity<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

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
    pub token_pt_src: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_sy_src: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_pt_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_sy_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_lp_dst: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub mint_lp: Box<InterfaceAccount<'info, Mint>>,

    /// Address lookup table for vault
    /// CHECK: constrained by market
    pub address_lookup_table: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: constrained by market
    pub sy_program: UncheckedAccount<'info>,
}

impl<'i> DepositLiquidity<'i> {
    fn to_transfer_pt_in_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.token_pt_src.to_account_info(),
            to: self.token_pt_escrow.to_account_info(),
            authority: self.depositor.to_account_info(),
        }
    }

    fn to_transfer_sy_in_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.token_sy_src.to_account_info(),
            to: self.token_sy_escrow.to_account_info(),
            authority: self.depositor.to_account_info(),
        }
    }

    fn to_mint_lp_accounts(&self) -> MintTo<'i> {
        MintTo {
            mint: self.mint_lp.to_account_info(),
            to: self.token_lp_dst.to_account_info(),
            authority: self.market.to_account_info(),
        }
    }

    fn to_transfer_pt_in_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_transfer_pt_in_accounts(),
        )
    }

    fn to_transfer_sy_in_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_transfer_sy_in_accounts(),
        )
    }

    fn to_mint_lp_ctx(&self) -> CpiContext<'_, '_, '_, 'i, MintTo<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_mint_lp_accounts(),
        )
    }

    fn do_transfer_pt_in(&self, amount: u64) -> Result<()> {
        #[allow(deprecated)]
        transfer(self.to_transfer_pt_in_ctx(), amount)
    }

    fn do_transfer_sy_in(&self, amount: u64) -> Result<()> {
        #[allow(deprecated)]
        transfer(self.to_transfer_sy_in_ctx(), amount)
    }

    fn do_transfers_in(&self, pt_in: u64, sy_in: u64) -> Result<()> {
        self.do_transfer_pt_in(pt_in)?;
        self.do_transfer_sy_in(sy_in)?;
        Ok(())
    }

    fn do_mint_lp(&self, amount: u64) -> Result<()> {
        mint_to(
            self.to_mint_lp_ctx()
                .with_signer(&[&self.market.signer_seeds()]),
            amount,
        )
    }

    fn verify_lp_supply(&mut self) -> Result<()> {
        self.mint_lp.reload()?;

        if self.market.check_supply_lp(self.mint_lp.supply) == false {
            return Err(ExponentCoreError::LpSupplyMaximumExceeded.into());
        }

        Ok(())
    }

    fn validate(&self) -> Result<()> {
        let current_timestamp = Clock::get()?.unix_timestamp as u64;

        require!(
            self.market.is_active(current_timestamp),
            ExponentCoreError::VaultIsNotActive
        );

        require!(
            self.market.check_status_flags(STATUS_CAN_DEPOSIT_LIQUIDITY),
            ExponentCoreError::DepositingLiquidityDisabled
        );

        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, DepositLiquidity<'info>>,
    pt_intent: u64,
    sy_intent: u64,
    min_lp_out: u64,
) -> Result<DepositLiquidityEvent> {
    let r = ctx.accounts.market.financials.add_liquidity(
        sy_intent,
        pt_intent,
        ctx.accounts.mint_lp.supply,
    );

    if r.lp_out < min_lp_out {
        return Err(ExponentCoreError::MinLpOutNotMet.into());
    }

    ctx.accounts
        .market
        .liquidity_net_balance_limits
        .verify_limits(
            Clock::get()?.unix_timestamp as u32,
            ctx.accounts.mint_lp.supply,
            r.lp_out as i64,
        )?;

    ctx.accounts.do_transfers_in(r.pt_in, r.sy_in)?;
    ctx.accounts.do_mint_lp(r.lp_out)?;

    do_deposit_sy(
        r.sy_in,
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        &ctx.accounts.to_account_infos(),
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.market.signer_seeds()],
    )?;

    ctx.accounts.verify_lp_supply()?;

    let event = DepositLiquidityEvent {
        depositor: ctx.accounts.depositor.key(),
        market: ctx.accounts.market.key(),
        token_pt_src: ctx.accounts.token_pt_src.key(),
        token_sy_src: ctx.accounts.token_sy_src.key(),
        token_pt_escrow: ctx.accounts.token_pt_escrow.key(),
        token_sy_escrow: ctx.accounts.token_sy_escrow.key(),
        token_lp_dst: ctx.accounts.token_lp_dst.key(),
        mint_lp: ctx.accounts.mint_lp.key(),
        pt_intent,
        sy_intent,
        pt_in: r.pt_in,
        sy_in: r.sy_in,
        lp_out: r.lp_out,
        new_lp_supply: ctx.accounts.mint_lp.supply,
        timestamp: Clock::get()?.unix_timestamp,
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct DepositLiquidityEvent {
    pub depositor: Pubkey,
    pub market: Pubkey,
    pub token_pt_src: Pubkey,
    pub token_sy_src: Pubkey,
    pub token_pt_escrow: Pubkey,
    pub token_sy_escrow: Pubkey,
    pub token_lp_dst: Pubkey,
    pub mint_lp: Pubkey,
    pub pt_intent: u64,
    pub sy_intent: u64,
    pub pt_in: u64,
    pub sy_in: u64,
    pub lp_out: u64,
    pub new_lp_supply: u64,
    pub timestamp: i64,
}
