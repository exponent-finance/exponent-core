use crate::{
    cpi_common::CpiAccounts,
    seeds::MARKET_SEED,
    utils::{cpi_init_sy_personal_account, do_deposit_sy},
    MarketTwo, Vault, ID,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::Token,
    token_2022::{self, MintTo, Transfer},
    token_interface::{Mint, TokenAccount},
};
use exponent_admin::Admin;
use precise_number::Number;
use token_util::{create_associated_token_account_2022, create_mint_2022, create_token_account};

#[derive(Accounts)]
#[instruction(
    ln_fee_rate_root: f64,
    rate_scalar_root: f64,
    init_rate_anchor: f64,
    sy_exchange_rate: Number,
    pt_init: u64,
    sy_init: u64,
    fee_treasury_sy_bps: u16,
    cpi_accounts: CpiAccounts,
    seed_id: u8,
)]
pub struct MarketTwoInit<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub admin_signer: Signer<'info>,

    /// There is 1 market per vault
    #[account(
        init,
        payer = payer,
        seeds = [
            MARKET_SEED,
            vault.key().as_ref(),
            &[seed_id],
        ],
        bump,
        space = MarketTwo::size_of(&cpi_accounts, 0, 0)
    )]
    pub market: Account<'info, MarketTwo>,

    /// Links the mint_sy & mint_pt & sy_program together
    #[account(
        has_one = mint_sy,
        has_one = mint_pt,
        has_one = sy_program
    )]
    pub vault: Box<Account<'info, Vault>>,

    pub mint_sy: Box<InterfaceAccount<'info, Mint>>,
    pub mint_pt: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: created & validated in handler
    #[account(mut)]
    pub mint_lp: UncheckedAccount<'info>,

    /// CHECK: created & validated in handler
    #[account(mut)]
    pub escrow_pt: UncheckedAccount<'info>,

    /// This account for SY is only a temporary pass-through account
    /// It is used to transfer SY tokens from the signer to the market
    /// And then from the market to the SY program's escrow
    /// CHECK: created and validated in handler
    #[account(mut)]
    pub escrow_sy: UncheckedAccount<'info>,

    /// Holds activated LP tokens for farming & SY emissions
    /// CHECK: created and validated in handler
    #[account(mut)]
    pub escrow_lp: UncheckedAccount<'info>,

    /// Signer's PT token account
    #[account(mut)]
    pub pt_src: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Signer's SY token account
    #[account(mut)]
    pub sy_src: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Receiving account for LP tokens
    /// CHECK: created and validated in handler
    #[account(mut)]
    pub lp_dst: UncheckedAccount<'info>,

    /// Use the old Token program as the implementation for PT & SY & LP tokens
    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,

    /// CHECK: constrained by vault
    pub sy_program: UncheckedAccount<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    /// CHECK: high trust instruction
    pub address_lookup_table: UncheckedAccount<'info>,

    pub admin: Box<Account<'info, Admin>>,

    #[account(
        token::mint = mint_sy,
    )]
    pub token_treasury_fee_sy: InterfaceAccount<'info, TokenAccount>,
}

impl<'i> MarketTwoInit<'i> {
    fn transfer_pt_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.pt_src.to_account_info(),
            to: self.escrow_pt.to_account_info(),
            authority: self.payer.to_account_info(),
        }
    }

    fn transfer_sy_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.sy_src.to_account_info(),
            to: self.escrow_sy.to_account_info(),
            authority: self.payer.to_account_info(),
        }
    }

    fn mint_lp_accounts(&self) -> MintTo<'i> {
        MintTo {
            mint: self.mint_lp.to_account_info(),
            to: self.lp_dst.to_account_info(),
            authority: self.market.to_account_info(),
        }
    }

    fn mint_lp_context(&self) -> CpiContext<'_, '_, '_, 'i, MintTo<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.mint_lp_accounts(),
        )
    }

    fn transfer_pt_context(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.transfer_pt_accounts(),
        )
    }

    fn transfer_sy_context(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.transfer_sy_accounts(),
        )
    }

    fn do_transfer_pt(&self, amount: u64) -> Result<()> {
        #[allow(deprecated)]
        token_2022::transfer(self.transfer_pt_context(), amount)
    }

    fn do_transfer_sy(&self, amount: u64) -> Result<()> {
        #[allow(deprecated)]
        token_2022::transfer(self.transfer_sy_context(), amount)
    }

    fn do_mint_lp(&self, amount: u64) -> Result<()> {
        token_2022::mint_to(
            self.mint_lp_context()
                .with_signer(&[&self.market.signer_seeds()]),
            amount,
        )
    }

    fn create_lp_mint(&self) -> Result<()> {
        let decimals = self.mint_sy.decimals;
        let (addr, bump) =
            Pubkey::find_program_address(&[b"mint_lp", self.market.key().as_ref()], &ID);

        assert_eq!(addr, self.mint_lp.key());

        create_mint_2022(
            &self.market.to_account_info(),
            &self.payer.to_account_info(),
            &self.mint_lp.to_account_info(),
            &self.token_program.to_account_info(),
            &self.system_program.to_account_info(),
            decimals,
            &[&[b"mint_lp", self.market.key().as_ref(), &[bump]]],
        )
    }

    /// Generic function to create token accounts for the market
    fn create_market_token_account(
        &self,
        mint: &AccountInfo<'i>,
        token_account: &AccountInfo<'i>,
        seed: &[u8],
    ) -> Result<Pubkey> {
        let (addr, bump) = Pubkey::find_program_address(&[seed, self.market.key().as_ref()], &ID);
        assert_eq!(addr, token_account.key());

        create_token_account(
            &self.market.to_account_info(),
            &self.payer.to_account_info(),
            token_account,
            mint,
            &self.system_program.to_account_info(),
            &self.token_program.to_account_info(),
            &[&[seed, self.market.key().as_ref(), &[bump]]],
        )?;

        Ok(addr)
    }

    fn create_payer_lp_account(&self) -> Result<()> {
        create_associated_token_account_2022(
            &self.payer.to_account_info(),
            &self.payer.to_account_info(),
            &self.mint_lp.to_account_info(),
            &self.lp_dst.to_account_info(),
            &self.token_program.to_account_info(),
            &self.associated_token_program.to_account_info(),
            &self.system_program.to_account_info(),
        )
    }

    fn create_escrow_pt(&self) -> Result<Pubkey> {
        self.create_market_token_account(
            &self.mint_pt.to_account_info(),
            &self.escrow_pt.to_account_info(),
            b"escrow_pt",
        )
    }

    fn create_escrow_sy(&self) -> Result<Pubkey> {
        self.create_market_token_account(
            &self.mint_sy.to_account_info(),
            &self.escrow_sy.to_account_info(),
            b"escrow_sy",
        )
    }

    fn create_escrow_lp(&self) -> Result<Pubkey> {
        self.create_market_token_account(
            &self.mint_lp.to_account_info(),
            &self.escrow_lp.to_account_info(),
            b"escrow_lp",
        )
    }

    fn validate(&self) -> Result<()> {
        self.validate_admin()?;
        Ok(())
    }

    fn validate_admin(&self) -> Result<()> {
        self.admin
            .principles
            .exponent_core
            .is_admin(self.admin_signer.key)
    }
}

/// Create a market struct from the raw arguments
pub fn make_market(
    ctx: &Context<MarketTwoInit>,
    ln_fee_rate_root: f64,
    rate_scalar_root: f64,
    init_rate_anchor: f64,
    sy_exchange_rate: Number,
    pt_init: u64,
    sy_init: u64,
    token_treasury_fee_sy: Pubkey,
    cpi_accounts: &CpiAccounts,
    fee_treasury_sy_bps: u16,
    seed_id: u8,
) -> MarketTwo {
    let expiration_ts = (ctx.accounts.vault.start_ts + ctx.accounts.vault.duration) as u64;
    let mint_pt = ctx.accounts.mint_pt.key();
    let mint_sy = ctx.accounts.mint_sy.key();
    let vault = ctx.accounts.vault.key();
    let mint_lp = ctx.accounts.mint_lp.key();
    let token_escrow_sy = ctx.accounts.escrow_sy.key();
    let token_escrow_pt = ctx.accounts.escrow_pt.key();
    let sy_program = ctx.accounts.sy_program.key();
    let escrow_lp = ctx.accounts.escrow_lp.key();

    MarketTwo::new(
        ctx.accounts.market.key(),
        [ctx.bumps.market],
        expiration_ts,
        ln_fee_rate_root,
        rate_scalar_root,
        init_rate_anchor,
        pt_init,
        sy_init,
        sy_exchange_rate,
        mint_pt,
        mint_sy,
        vault,
        mint_lp,
        token_escrow_pt,
        token_escrow_sy,
        escrow_lp,
        ctx.accounts.address_lookup_table.key(),
        token_treasury_fee_sy,
        sy_program,
        cpi_accounts.clone(),
        fee_treasury_sy_bps,
        seed_id,
    )
}

#[access_control(ctx.accounts.validate())]
pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, MarketTwoInit<'info>>,
    // log of fee rate root
    ln_fee_rate_root: f64,

    // rate scalar root amount
    rate_scalar_root: f64,

    // initial rate anchor
    init_rate_anchor: f64,

    // exchange rate for SY into base asset
    // TODO - get this through a CPI call instead
    sy_exchange_rate: Number,

    // initial amount of PT liquidity
    pt_init: u64,

    // initial amount of SY liquidity
    sy_init: u64,

    // fee treasury SY BPS
    fee_treasury_sy_bps: u16,

    // indexes for CPI account vectors
    cpi_accounts: CpiAccounts,

    // unique seed id for the market
    seed_id: u8,
) -> Result<()> {
    // make the market account from a factory
    let market = make_market(
        &ctx,
        ln_fee_rate_root,
        rate_scalar_root,
        init_rate_anchor,
        sy_exchange_rate,
        pt_init,
        sy_init,
        ctx.accounts.token_treasury_fee_sy.key(),
        &cpi_accounts,
        fee_treasury_sy_bps,
        seed_id,
    );
    ctx.accounts.market.set_inner(market);

    ctx.accounts.create_lp_mint()?;

    // token account for holding PT liquidity
    ctx.accounts.create_escrow_pt()?;

    // token account for passing SY to the SY program and back
    ctx.accounts.create_escrow_sy()?;

    // token account to hold deposited LP tokens for earning emissions
    ctx.accounts.create_escrow_lp()?;

    // create a token account for the receiver's LP tokens
    // we must do this in the instruction because the Mint is created in this instruction
    ctx.accounts.create_payer_lp_account()?;

    // transfer tokens from user to market
    ctx.accounts.do_transfer_pt(pt_init)?;
    ctx.accounts.do_transfer_sy(sy_init)?;

    // give user LP tokens in exchange
    ctx.accounts
        .do_mint_lp(calc_lp_tokens_out(pt_init, sy_init))?;

    // Create an account for the Market robot with the SY Program
    cpi_init_sy_personal_account(ctx.accounts.sy_program.key(), ctx.remaining_accounts)?;

    do_deposit_sy(
        sy_init,
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        &ctx.accounts.to_account_infos(),
        &ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.market.signer_seeds()],
    )?;

    Ok(())
}

/// compute the geometric mean of pt & sy.
/// this is the amount of LP tokens to mint
fn calc_lp_tokens_out(pt_in: u64, sy_in: u64) -> u64 {
    let product = pt_in
        .checked_mul(sy_in)
        .expect("Overflow occurred during multiplication");

    // Compute the square root safely
    (product as f64).sqrt() as u64
}