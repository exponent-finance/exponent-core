use crate::{instructions::self_cpi, utils::sy_cpi, MarketTwo};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperWithdrawLiquidityClassic<'info> {
    #[account(mut)]
    pub withdrawer: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, MarketTwo>,

    /// PT liquidity account
    #[account(mut)]
    pub token_pt_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    /// SY token account owned by the market
    #[account(mut)]
    pub token_sy_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_lp_src: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub mint_lp: Box<InterfaceAccount<'info, Mint>>,

    /// Token account for SY owned by the depositor
    /// CHECK: Checked by strip
    #[account(mut)]
    pub token_sy_withdrawer: InterfaceAccount<'info, TokenAccount>,

    /// Token account for PT owned by the depositor
    /// CHECK: Checked by strip
    #[account(mut)]
    pub token_pt_withdrawer: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Checked by strip
    pub token_program: Program<'info, Token>,

    /// CHECK: Checked by Trade Pt, Withdraw Liquidity, Withdraw Lp
    pub market_address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_lp
    #[account(mut)]
    pub token_lp_escrow: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_lp
    #[account(mut)]
    pub lp_position: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'i> WrapperWithdrawLiquidityClassic<'i> {
    fn to_withdraw_liquidity_accounts(&self) -> self_cpi::WithdrawLiquidityAccounts<'i> {
        self_cpi::WithdrawLiquidityAccounts {
            withdrawer: self.withdrawer.to_account_info(),
            market: self.market.to_account_info(),
            token_pt_dst: self.token_pt_withdrawer.to_account_info(),
            token_sy_dst: self.token_sy_withdrawer.to_account_info(),
            token_pt_escrow: self.token_pt_escrow.to_account_info(),
            token_sy_escrow: self.token_sy_escrow.to_account_info(),
            token_lp_src: self.token_lp_src.to_account_info(),
            mint_lp: self.mint_lp.to_account_info(),
            address_lookup_table: self.market_address_lookup_table.to_account_info(),
            token_program: self.token_program.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
        }
    }

    fn to_withdraw_lp_accounts(&self) -> self_cpi::WithdrawLpAccounts<'i> {
        self_cpi::WithdrawLpAccounts {
            owner: self.withdrawer.to_account_info(),
            market: self.market.to_account_info(),
            mint_lp: self.mint_lp.to_account_info(),
            lp_position: self.lp_position.to_account_info(),
            token_lp_dst: self.token_lp_src.to_account_info(),
            token_lp_escrow: self.token_lp_escrow.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            address_lookup_table: self.market_address_lookup_table.to_account_info(),
            token_program: self.token_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
        }
    }
}

pub fn handler<'i>(
    ctx: Context<'_, '_, '_, 'i, WrapperWithdrawLiquidityClassic<'i>>,
    amount_lp: u64,
    redeem_sy_accounts_length: u8,
) -> Result<()> {
    // withdraw lp
    self_cpi::do_cpi_withdraw_lp(
        ctx.accounts.to_withdraw_lp_accounts(),
        &ctx.remaining_accounts[redeem_sy_accounts_length as usize..],
        amount_lp,
    )?;

    ctx.accounts.market.reload()?;

    let withdraw_liquidity_return_data = self_cpi::do_cpi_withdraw_liquidity(
        ctx.accounts.to_withdraw_liquidity_accounts(),
        &ctx.remaining_accounts[redeem_sy_accounts_length as usize..],
        amount_lp,
        0,
        0,
    )?;

    ctx.accounts.market.reload()?;
    ctx.accounts.mint_lp.reload()?;

    let redeem_sy_return_data = sy_cpi::cpi_redeem_sy(
        ctx.accounts.sy_program.key(),
        withdraw_liquidity_return_data.sy_out,
        &ctx.remaining_accounts[..redeem_sy_accounts_length as usize],
        ctx.remaining_accounts[..redeem_sy_accounts_length as usize]
            .to_vec()
            .to_account_metas(None),
    )?;

    emit_cpi!(WrapperWithdrawLiquidityClassicEvent {
        user_address: ctx.accounts.withdrawer.key(),
        market_address: ctx.accounts.market.key(),
        amount_base_out: redeem_sy_return_data.base_out_amount,
        amount_lp_in: amount_lp,
        amount_pt_out: withdraw_liquidity_return_data.pt_out,
        lp_price: ctx.accounts.market.financials.lp_price_in_asset(
            Clock::get().unwrap().unix_timestamp as u64,
            redeem_sy_return_data.exchange_rate,
            ctx.accounts.mint_lp.supply,
        ),
    });

    Ok(())
}

#[event]
pub struct WrapperWithdrawLiquidityClassicEvent {
    /// User who withdrew liquidity
    pub user_address: Pubkey,
    /// Market that the user withdrew liquidity from
    pub market_address: Pubkey,
    /// Amount of base token out
    pub amount_base_out: u64,
    /// Amount of LP token in
    pub amount_lp_in: u64,
    /// Amount of PT token out
    pub amount_pt_out: u64,
    /// LP price in asset
    pub lp_price: f64,
}