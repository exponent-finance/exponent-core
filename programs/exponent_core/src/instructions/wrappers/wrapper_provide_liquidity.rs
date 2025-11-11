use crate::{
    cpi_common::CpiInterfaceContext,
    instructions::self_cpi,
    util::deserialize_lookup_table,
    utils::{do_get_sy_state, sy_cpi},
    MarketTwo, Vault,
};
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::*};
use precise_number::Number;

#[event_cpi]
#[derive(Accounts)]
pub struct WrapperProvideLiquidity<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    /// CHECK: constrained by vault
    /// The authority owned by the vault for minting PT/YT
    pub authority: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub vault: Box<Account<'info, Vault>>,

    #[account(
        mut,
        has_one = vault,
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
    /// CHECK: Checked by strip
    #[account(mut)]
    pub token_sy_depositor: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub escrow_sy: UncheckedAccount<'info>,

    /// Token account for YT owned by the depositor
    /// CHECK: Checked by strip
    #[account(mut)]
    pub token_yt_depositor: UncheckedAccount<'info>,

    /// Token account for PT owned by the depositor
    /// CHECK: Checked by strip
    #[account(mut)]
    pub token_pt_depositor: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub mint_yt: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    #[account(mut)]
    pub mint_pt: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    pub token_program: Program<'info, Token>,

    /// CHECK: Checked by strip
    pub vault_address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: constrained by market
    #[account(
        constraint = market_address_lookup_table.key() == market.address_lookup_table
    )]
    pub market_address_lookup_table: UncheckedAccount<'info>,

    /// CHECK: Checked by strip
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_yt
    #[account(mut)]
    pub user_yield_position: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_yt
    #[account(mut)]
    pub escrow_yt: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_lp
    #[account(mut)]
    pub token_lp_escrow: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_lp
    #[account(mut)]
    pub lp_position: UncheckedAccount<'info>,

    /// CHECK: Checked by deposit_yt
    #[account(mut)]
    pub vault_robot_yield_position: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'i> WrapperProvideLiquidity<'i> {
    fn to_strip_accounts(&self) -> self_cpi::StripAccounts<'i> {
        self_cpi::StripAccounts {
            depositor: self.depositor.to_account_info(),
            authority: self.authority.to_account_info(),
            vault: self.vault.to_account_info(),
            sy_src: self.token_sy_depositor.to_account_info(),
            escrow_sy: self.escrow_sy.to_account_info(),
            yt_dst: self.token_yt_depositor.to_account_info(),
            pt_dst: self.token_pt_depositor.to_account_info(),
            mint_yt: self.mint_yt.to_account_info(),
            mint_pt: self.mint_pt.to_account_info(),
            token_program: self.token_program.to_account_info(),
            address_lookup_table: self.vault_address_lookup_table.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            yield_position: self.vault_robot_yield_position.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
            program: self.program.to_account_info(),
        }
    }

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

    fn to_deposit_yt_accounts(&self) -> self_cpi::DepositYtAccounts<'i> {
        self_cpi::DepositYtAccounts {
            depositor: self.depositor.to_account_info(),
            vault: self.vault.to_account_info(),
            escrow_yt: self.escrow_yt.to_account_info(),
            token_program: self.token_program.to_account_info(),
            sy_program: self.sy_program.to_account_info(),
            address_lookup_table: self.vault_address_lookup_table.to_account_info(),
            yield_position: self.vault_robot_yield_position.to_account_info(),
            system_program: self.system_program.to_account_info(),
            yt_src: self.token_yt_depositor.to_account_info(),
            user_yield_position: self.user_yield_position.to_account_info(),
            program: self.program.to_account_info(),
            event_authority: self.event_authority.to_account_info(),
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
}

fn calc_strip_amount(
    total_amount_sy: u64,
    cur_sy_rate: Number,
    market_pt_liq: u64,
    market_sy_liq: u64,
) -> u64 {
    let amt_sy = Number::from_natural_u64(total_amount_sy);

    // we want to split amt_sy into two lots: A & B
    // B will be stripped into PT -- at the amount B * cur_sy_rate
    // A remains
    // The proportion A/(B * cur_sy_rate) should be the same as the proportion of SY/PT in the market
    // Eq 1: A + B = amt_sy
    // Eq 2: A / (B * cur_sy_rate) = market_sy_liq / market_pt_liq
    // => A = B * cur_sy_rate * market_sy_liq / market_pt_liq
    // => B = A * market_pt_liq / (cur_sy_rate * market_sy_liq)
    // => B = (amt_sy - B) * market_pt_liq / (cur_sy_rate * market_sy_liq)
    // => B = (amt_sy * market_pt_liq) / (market_pt_liq + market_sy_liq * cur_sy_rate)

    let to_strip =
        amt_sy * market_pt_liq.into() / (cur_sy_rate * market_sy_liq.into() + market_pt_liq.into());

    to_strip.ceil_u64()
}

pub fn handler<'i>(
    ctx: Context<'_, '_, '_, 'i, WrapperProvideLiquidity<'i>>,
    amount_base: u64,
    min_lp_out: u64,
    mint_sy_rem_accounts_until: u8,
) -> Result<()> {
    let mint_sy_rem_accounts = &ctx.remaining_accounts[..mint_sy_rem_accounts_until as usize];
    let interface_cpi_rem_accounts = &ctx.remaining_accounts[mint_sy_rem_accounts_until as usize..];

    // mint SY from base
    let mint_sy_return_data = sy_cpi::cpi_mint_sy(
        ctx.accounts.vault.sy_program,
        amount_base,
        mint_sy_rem_accounts,
        mint_sy_rem_accounts.to_vec().to_account_metas(None),
    )?;

    assert!(
        mint_sy_return_data.sy_out_amount > 0,
        "sy_amount cannot be 0"
    );

    let sy_state = do_get_sy_state(
        &ctx.accounts.vault_address_lookup_table.to_account_info(),
        &ctx.accounts.vault.cpi_accounts,
        interface_cpi_rem_accounts,
        ctx.accounts.sy_program.key(),
    )?;

    let market_pt_liq = ctx.accounts.market.financials.pt_balance;
    let market_sy_liq = ctx.accounts.market.financials.sy_balance;

    let to_strip = calc_strip_amount(
        mint_sy_return_data.sy_out_amount,
        sy_state.exchange_rate,
        market_pt_liq,
        market_sy_liq,
    );
    let sy_remainder = mint_sy_return_data
        .sy_out_amount
        .checked_sub(to_strip)
        .unwrap();

    let vault_alt = deserialize_lookup_table(&ctx.accounts.vault_address_lookup_table);

    // strip SY into PT & YT
    let e = self_cpi::do_cpi_strip(
        ctx.accounts.to_strip_accounts(),
        filter_pubkeys(
            &vault_alt,
            interface_cpi_rem_accounts,
            &ctx.accounts.vault.cpi_accounts.deposit_sy,
        )
        .as_slice(),
        to_strip,
    )?;

    // Reload the vault, because stripping SY changes vault state (uncollected sy, etc)
    ctx.accounts.vault.reload()?;

    // get the amount of PT minted
    let py_amount = e.amount_py_out;

    let lp_amount_before = ctx.accounts.token_lp_dst.amount;
    // CPI to deposit liquidity
    self_cpi::do_cpi_deposit_liquidity(
        ctx.accounts.to_deposit_liquidity_accounts(),
        interface_cpi_rem_accounts,
        py_amount,
        sy_remainder,
        min_lp_out,
    )?;

    // Reload the market, because depositing liquidity changes market state
    ctx.accounts.market.reload()?;
    ctx.accounts.mint_lp.reload()?;
    ctx.accounts.token_lp_dst.reload()?;

    let lp_amount_after = ctx.accounts.token_lp_dst.amount;

    // CPI to deposit YT
    self_cpi::do_cpi_deposit_yt(
        ctx.accounts.to_deposit_yt_accounts(),
        filter_pubkeys(
            &vault_alt,
            interface_cpi_rem_accounts,
            &ctx.accounts.vault.cpi_accounts.get_sy_state,
        )
        .as_slice(),
        py_amount,
    )?;

    // Reload the vault, because depositing YT changes vault state (uncollected sy, etc)
    ctx.accounts.vault.reload()?;

    let lp_balance_diff = lp_amount_after - lp_amount_before;

    // CPI to deposit LP
    self_cpi::do_cpi_deposit_lp(
        ctx.accounts.to_deposit_lp_accounts(),
        interface_cpi_rem_accounts,
        lp_balance_diff,
    )?;

    // Reload the market, because depositing LP changes market state
    ctx.accounts.market.reload()?;

    let event = WrapperProvideLiquidityEvent {
        user_address: *ctx.accounts.depositor.key,
        amount_base_in: amount_base,
        amount_lp_out: lp_balance_diff,
        market_address: ctx.accounts.market.key(),
        amount_yt_out: e.amount_py_out,
        lp_price: ctx.accounts.market.financials.lp_price_in_asset(
            Clock::get().unwrap().unix_timestamp as u64,
            sy_state.exchange_rate,
            ctx.accounts.mint_lp.supply,
        ),
    };

    emit_cpi!(event);

    Ok(())
}

pub fn filter_pubkeys<'i>(
    alt: &Vec<Pubkey>,
    rem_accounts: &[AccountInfo<'i>],
    cpi_accounts: &Vec<CpiInterfaceContext>,
) -> Vec<AccountInfo<'i>> {
    let get_key_from_alt = |alt_index: u8| -> &Pubkey { alt.get(alt_index as usize).unwrap() };

    let is_in_cpi_accounts = |ac: &AccountInfo| -> bool {
        for cpi_ac in cpi_accounts {
            let k = get_key_from_alt(cpi_ac.alt_index);
            if ac.key.eq(k) {
                return true;
            }
        }
        false
    };

    rem_accounts
        .iter()
        .filter(|x| is_in_cpi_accounts(x))
        .cloned()
        .collect::<Vec<_>>()
}

#[event]
pub struct WrapperProvideLiquidityEvent {
    pub user_address: Pubkey,
    pub market_address: Pubkey,
    pub amount_base_in: u64,
    pub amount_lp_out: u64,
    pub amount_yt_out: u64,
    pub lp_price: f64,
}