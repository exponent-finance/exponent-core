use anchor_lang::prelude::*;
#[allow(deprecated)]
use anchor_spl::token_interface::{transfer, Mint, TokenAccount, TokenInterface, Transfer};
use exponent_admin::Admin;

use crate::{error::ExponentCoreError, MarketTwo};

#[derive(Accounts)]
pub struct AddFarm<'info> {
    #[account(
        mut,
        realloc = MarketTwo::size_of(&market.cpi_accounts, market.emissions.trackers.len(), market.lp_farm.farm_emissions.len() + 1),
        realloc::payer = fee_payer,
        realloc::zero = false,
    )]
    pub market: Account<'info, MarketTwo>,

    pub signer: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub mint_new: InterfaceAccount<'info, Mint>,

    pub admin_state: Account<'info, Admin>,

    #[account(mut)]
    pub token_source: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint_new,
        associated_token::authority = market,
        associated_token::token_program = token_program
    )]
    pub token_farm: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

impl<'i> AddFarm<'i> {
    pub fn add_farm(&mut self, token_rate: u64, expiry_ts: u32, token_mint: &Pubkey) {
        self.market.add_farm(token_rate, expiry_ts, token_mint);
    }

    pub fn validate(&self) -> Result<()> {
        self.admin_state
            .principles
            .exponent_core
            .is_admin(&self.signer.key())?;

        // Check if the new farm's mint already exists
        if self
            .market
            .lp_farm
            .farm_emissions
            .iter()
            .any(|farm| farm.mint == self.mint_new.key())
        {
            return Err(ExponentCoreError::FarmAlreadyExists.into());
        }

        Ok(())
    }

    fn to_transfer_sy_in_accounts(&self) -> Transfer<'i> {
        Transfer {
            authority: self.signer.to_account_info(),
            from: self.token_source.to_account_info(),
            to: self.token_farm.to_account_info(),
        }
    }

    pub fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.to_transfer_sy_in_accounts(),
        )
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(ctx: Context<AddFarm>, token_rate: u64, until_timestamp: u32) -> Result<()> {
    let farm_token_mint = &ctx.accounts.mint_new;
    let current_unix_timestamp = Clock::get()?.unix_timestamp as u32;

    let current_lp_escrow_amount = ctx.accounts.market.lp_escrow_amount;
    ctx.accounts
        .market
        .lp_farm
        .increase_share_indexes(current_unix_timestamp, current_lp_escrow_amount);

    ctx.accounts
        .add_farm(token_rate, until_timestamp, &farm_token_mint.key());

    let duration = until_timestamp
        .checked_sub(current_unix_timestamp)
        .ok_or(ExponentCoreError::DurationNegative)
        .unwrap() as u64;

    let required_amount = duration.checked_mul(token_rate).unwrap();

    #[allow(deprecated)]
    transfer(ctx.accounts.transfer_ctx(), required_amount)?;

    Ok(())
}