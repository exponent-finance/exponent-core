use crate::{utils::do_get_position_state, LpPosition, MarketTwo, PersonalYieldTrackers};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_2022::{self, Transfer},
    token_interface::Mint,
};

#[event_cpi]
#[derive(Accounts)]
pub struct DepositLp<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = token_lp_escrow,
        has_one = sy_program,
        has_one = address_lookup_table,
        has_one = mint_lp
    )]
    pub market: Box<Account<'info, MarketTwo>>,

    // Owner is not checked, as anyone can donate LP tokens to a position
    #[account(
        mut,
        has_one = market,
        realloc = LpPosition::static_size_of(market.emissions.trackers.len(), market.lp_farm.farm_emissions.len()),
        realloc::payer = owner,
        realloc::zero = false,
    )]
    pub lp_position: Box<Account<'info, LpPosition>>,

    /// CHECK: constrained by token program
    #[account(mut)]
    pub token_lp_src: UncheckedAccount<'info>,

    /// CHECK: constrained by market
    #[account(mut)]
    pub token_lp_escrow: UncheckedAccount<'info>,

    pub mint_lp: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: constrained by market
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: constrained by market
    pub address_lookup_table: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,
}

impl<'i> DepositLp<'i> {
    fn transfer_lp_in_accounts(&self) -> Transfer<'i> {
        Transfer {
            from: self.token_lp_src.to_account_info(),
            to: self.token_lp_escrow.to_account_info(),
            authority: self.owner.to_account_info(),
        }
    }

    fn transfer_lp_in_context(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            self.transfer_lp_in_accounts(),
        )
    }

    fn do_transfer_lp_in(&mut self, amount: u64) -> Result<()> {
        #[allow(deprecated)]
        token_2022::transfer(self.transfer_lp_in_context(), amount)?;

        self.market.lp_escrow_amount = self.market.lp_escrow_amount.checked_add(amount).unwrap();

        Ok(())
    }
}

pub fn handler(ctx: Context<DepositLp>, amount: u64) -> Result<DepositLpEventV2> {
    let current_unix_timestamp = Clock::get()?.unix_timestamp as u32;

    let position_state = do_get_position_state(
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.market.cpi_accounts,
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.market.signer_seeds()],
    )?;

    let current_lp_escrow_amount = ctx.accounts.market.lp_escrow_amount;
    ctx.accounts
        .market
        .update_emissions_from_position_state(&position_state, current_lp_escrow_amount);

    ctx.accounts
        .market
        .lp_farm
        .increase_share_indexes(current_unix_timestamp, current_lp_escrow_amount);

    ctx.accounts.lp_position.stage_all(
        &ctx.accounts.market.emissions.get_last_seen_indices(),
        &ctx.accounts.market.lp_farm.get_last_seen_indices(),
    );

    // Transfer LP tokens into account
    ctx.accounts.do_transfer_lp_in(amount)?;

    // Increase LP balance
    ctx.accounts.lp_position.add_lp(amount);

    let event = DepositLpEventV2 {
        owner: ctx.accounts.owner.key(),
        market: ctx.accounts.market.key(),
        lp_position: ctx.accounts.lp_position.key(),
        token_lp_src: ctx.accounts.token_lp_src.key(),
        token_lp_escrow: ctx.accounts.token_lp_escrow.key(),
        mint_lp: ctx.accounts.mint_lp.key(),
        amount,
        new_lp_balance: ctx.accounts.lp_position.lp_balance,
        timestamp: Clock::get()?.unix_timestamp,
        emissions: ctx.accounts.lp_position.emissions.clone(),
        farms: ctx.accounts.lp_position.farms.clone(),
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct DepositLpEvent {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub lp_position: Pubkey,
    pub token_lp_src: Pubkey,
    pub token_lp_escrow: Pubkey,
    pub mint_lp: Pubkey,
    pub amount: u64,
    pub new_lp_balance: u64,
    pub timestamp: i64,
}

#[event]
pub struct DepositLpEventV2 {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub lp_position: Pubkey,
    pub token_lp_src: Pubkey,
    pub token_lp_escrow: Pubkey,
    pub mint_lp: Pubkey,
    pub amount: u64,
    pub new_lp_balance: u64,
    pub timestamp: i64,
    pub emissions: PersonalYieldTrackers,
    pub farms: PersonalYieldTrackers,
}