use crate::{
    instructions::self_cpi,
    utils::{sy_cpi, sy_to_py_ceil},
    MarketTwo,
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperProvideLiquidityBase<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        has_one = token_pt_escrow,
        has_one = token_sy_escrow,
        has_one = mint_lp,
    )]
    pub market: Box<Account<'info, MarketTwo>>,

    /// PT liquidity account
    #[account(mut)]
    pub token_pt_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    /// SY token account owned by the market
    #[account(mut)]
    pub token_sy_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_lp_dst: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub mint_lp: Box<InterfaceAccount<'info, Mint>>,

    /// Token account for SY owned by the depositor
    /// CHECK: Checked by trade_pt
    #[account(mut)]
    pub token_sy_depositor: InterfaceAccount<'info, TokenAccount>,

    /// Token account for PT owned by the depositor
    /// CHECK: Checked by trade_pt
    #[account(mut)]
    pub token_pt_depositor: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Checked by trade_pt
    pub token_program: Program<'info, Token>,

    /// CHECK: Checked by trade_pt
    pub market_address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: Checked by trade_pt
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: Checked by trade_pt
    #[account(mut)]
    pub token_fee_treasury_sy: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_lp
    #[account(mut)]
    pub token_lp_escrow: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_lp
    #[account(mut)]
    pub lp_position: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'i> WrapperProvideLiquidityBase<'i> {
    fn to_deposit_liquidity_accounts(&self) -> self_cpi::DepositLiquidityAccounts<'i> {
        self_cpi::DepositLiquidityAccounts {
            depositor: self.depositor.to_account_info(),
            market: self.market.to_account_info(),
            token_pt_escrow: self.token_pt_escrow.to_account_info(),
            token_sy_escrow: self.token_sy_escrow.to_account_info(),
            mint_lp: self.mint_lp.to_account_info(),
            token_lp_dst: self.token_lp_dst.to_account_info(),
            address_lookup_table: self.market_address_lookup_table.to_account_info(),
            token_program: self.token_program.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            token_pt_src: self.token_pt_depositor.to_account_info(),
            token_sy_src: self.token_sy_depositor.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
        }
    }

    fn to_deposit_lp_accounts(&self) -> self_cpi::DepositLpAccounts<'i> {
        self_cpi::DepositLpAccounts {
            owner: self.depositor.to_account_info(),
            address_lookup_table: self.market_address_lookup_table.to_account_info(),
            lp_position: self.lp_position.to_account_info(),
            market: self.market.to_account_info(),
            token_program: self.token_program.to_account_info(),
            token_lp_src: self.token_lp_dst.to_account_info(),
            token_lp_escrow: self.token_lp_escrow.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
            mint_lp: self.mint_lp.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
        }
    }

    fn to_trade_pt_accounts(&self) -> self_cpi::TradePtAccounts<'i> {
        self_cpi::TradePtAccounts {
            trader: self.depositor.to_account_info(),
            market: self.market.to_account_info(),
            token_sy_trader: self.token_sy_depositor.to_account_info(),
            token_pt_trader: self.token_pt_depositor.to_account_info(),
            token_sy_escrow: self.token_sy_escrow.to_account_info(),
            token_pt_escrow: self.token_pt_escrow.to_account_info(),
            address_lookup_table: self.market_address_lookup_table.to_account_info(),
            token_program: self.token_program.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
            token_fee_treasury_sy: self.token_fee_treasury_sy.to_account_info(),
        }
    }
}

pub fn handler<'i>(
    ctx: Context<'_, '_, '_, 'i, WrapperProvideLiquidityBase<'i>>,
    _amount_base: u64,
    min_lp_out: u64,
    mint_sy_accounts_until: u8,
    external_pt_to_buy: u64,
    external_sy_constraint: u64,
) -> Result<()> {
    let mint_sy_rem_accounts = &ctx.remaining_accounts[..mint_sy_accounts_until as usize];
    let interface_cpi_rem_accounts = &ctx.remaining_accounts[mint_sy_accounts_until as usize..];

    let sy_state = sy_cpi::do_get_sy_state(
        &ctx.accounts.market_address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        interface_cpi_rem_accounts,
        ctx.accounts.sy_program.key(),
    )?;

    let mut market_financials_clone = ctx.accounts.market.financials.clone();
    let trade_simulation_result = market_financials_clone.trade_pt(
        sy_state.exchange_rate,
        external_pt_to_buy as i64,
        Clock::get()?.unix_timestamp as u64,
        true,
        ctx.accounts.market.fee_treasury_sy_bps,
    );

    let sy_needed_for_pt = trade_simulation_result.net_trader_sy.abs() as u64;

    let lp_supply = ctx.accounts.mint_lp.supply;

    let sim_result = market_financials_clone.add_liquidity(
        u64::MAX, // SY intent of max to see how much we need
        external_pt_to_buy,
        lp_supply,
    );

    let sy_needed_for_liquidity = sim_result.sy_in;

    let total_sy_needed = sy_needed_for_pt
        .checked_add(sy_needed_for_liquidity)
        .unwrap();

    let base_amount_needed = sy_to_py_ceil(sy_state.exchange_rate, total_sy_needed);

    // Mint SY from base tokens
    let mint_sy_return_data = sy_cpi::cpi_mint_sy(
        ctx.accounts.market.sy_program,
        base_amount_needed,
        mint_sy_rem_accounts,
        mint_sy_rem_accounts.to_vec().to_account_metas(None),
    )?;

    ctx.accounts.market.is_current_flash_swap = true;
    ctx.accounts.market.exit(&crate::ID)?;

    let trade_pt_return_data = self_cpi::do_cpi_trade_pt(
        ctx.accounts.to_trade_pt_accounts(),
        interface_cpi_rem_accounts,
        external_pt_to_buy as i64,
        (external_sy_constraint as i64).checked_mul(-1).unwrap(),
    )?;

    ctx.accounts.market.reload()?;

    // CPI to deposit liquidity
    let deposit_liquidity_return_data = self_cpi::do_cpi_deposit_liquidity(
        ctx.accounts.to_deposit_liquidity_accounts(),
        interface_cpi_rem_accounts,
        trade_pt_return_data.net_trader_pt as u64,
        u64::MAX,
        min_lp_out,
    )?;

    // Reload the market, because depositing liquidity changes market state
    ctx.accounts.market.reload()?;
    ctx.accounts.mint_lp.reload()?;

    // CPI to deposit LP
    self_cpi::do_cpi_deposit_lp(
        ctx.accounts.to_deposit_lp_accounts(),
        interface_cpi_rem_accounts,
        deposit_liquidity_return_data.lp_out,
    )?;

    // Reload the market, because depositing LP changes market state
    ctx.accounts.market.reload()?;

    ctx.accounts.market.is_current_flash_swap = false;

    let event = WrapperProvideLiquidityBaseEvent {
        user_address: *ctx.accounts.depositor.key,
        amount_base_in: base_amount_needed,
        amount_lp_out: deposit_liquidity_return_data.lp_out,
        market_address: ctx.accounts.market.key(),
        trade_amount_pt_out: trade_pt_return_data.net_trader_pt as u64,
        trade_amount_sy_in: mint_sy_return_data.sy_out_amount,
        lp_price: ctx.accounts.market.financials.lp_price_in_asset(
            Clock::get().unwrap().unix_timestamp as u64,
            mint_sy_return_data.exchange_rate,
            ctx.accounts.mint_lp.supply,
        ),
    };

    emit_cpi!(event);

    Ok(())
}

#[event]
pub struct WrapperProvideLiquidityBaseEvent {
    pub user_address: Pubkey,
    pub market_address: Pubkey,
    pub amount_base_in: u64,
    pub trade_amount_pt_out: u64,
    pub trade_amount_sy_in: u64,
    pub amount_lp_out: u64,
    pub lp_price: f64,
}