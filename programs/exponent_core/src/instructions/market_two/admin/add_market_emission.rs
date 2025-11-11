use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use exponent_admin::Admin;
use precise_number::Number;

use crate::{cpi_common::CpiAccounts, MarketTwo};

#[derive(Accounts)]
#[instruction(cpi_accounts: CpiAccounts)]
pub struct AddMarketEmission<'info> {
    #[account(
        mut,
        realloc = MarketTwo::size_of(&cpi_accounts, market.emissions.trackers.len() + 1, market.lp_farm.farm_emissions.len()),
        realloc::payer = fee_payer,
        realloc::zero = false,
    )]
    pub market: Account<'info, MarketTwo>,

    pub signer: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub mint_new: InterfaceAccount<'info, Mint>,

    pub admin_state: Account<'info, Admin>,

    #[account(
        mut,
        associated_token::mint = mint_new,
        associated_token::authority = market,
        associated_token::token_program = token_program,
    )]
    pub token_emission: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

impl<'i> AddMarketEmission<'i> {
    pub fn validate(&self) -> Result<()> {
        self.admin_state
            .principles
            .exponent_core
            .is_admin(&self.signer.key())?;

        Ok(())
    }

    pub fn update_market(&mut self, cpi_accounts: CpiAccounts) {
        self.market.cpi_accounts = cpi_accounts;
    }
}

// todo: to make sure the robot's positions in the sy program get realloced by making an empty deposit when adding an emission
#[access_control(ctx.accounts.validate())]
pub fn handler(ctx: Context<AddMarketEmission>, cpi_accounts: CpiAccounts) -> Result<()> {
    ctx.accounts.update_market(cpi_accounts);

    ctx.accounts
        .market
        .emissions
        .trackers
        .push(crate::MarketEmission {
            token_escrow: ctx.accounts.token_emission.key(),
            lp_share_index: Number::ZERO,
            last_seen_staged: 0,
        });

    Ok(())
}