use crate::{
    error::ExponentCoreError, instructions::vault::common::yield_position_earn, state::*,
    util::token_transfer, utils::do_withdraw_sy,
};
use amount_value::Amount;
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_2022::Transfer, token_interface::TokenAccount};

/// Withdraw the SY earned by holding YT
///
/// SY is earned from YT deposited in the YieldTokenPosition
/// The SY tokens themselves are held by the vault in escrow
/// As time passes, and the exchange rate of SY to base asset increases, the YT holders earn the appreciation of SY
///
/// The idea is that a yield-stripping vault produces PT (which does not appreciate) and YT (which is a claim on future appreciation of SY)
///
#[event_cpi]
#[derive(Accounts)]
pub struct CollectInterest<'info> {
    /// Owner of the YieldTokenPosition
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Owner's position data for YT deposits
    #[account(
        mut,
        has_one = vault,
        has_one = owner,
    )]
    pub yield_position: Box<Account<'info, YieldTokenPosition>>,

    /// The vault that holds the SY tokens in escrow
    #[account(
        mut,
        has_one = authority,
        has_one = address_lookup_table,
        has_one = escrow_sy,
        has_one = treasury_sy_token_account,
        has_one = sy_program,
    )]
    pub vault: Account<'info, Vault>,

    /// The receiving token account for SY withdrawn
    #[account(mut)]
    pub token_sy_dst: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub escrow_sy: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: constrained by vault
    #[account(mut)]
    pub authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: constrained by vault
    pub sy_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub treasury_sy_token_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: constrained by vault
    pub address_lookup_table: UncheckedAccount<'info>,
}

impl<'i> CollectInterest<'i> {
    fn transfer_context(&self, to: AccountInfo<'i>) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.escrow_sy.to_account_info(),
                to,
                authority: self.authority.to_account_info(),
            },
        )
    }

    fn transfer_sy(&self, to: AccountInfo<'i>, amount: u64) -> Result<()> {
        let signer_seeds = &[&self.vault.signer_seeds()[..]];
        let ctx = self.transfer_context(to).with_signer(signer_seeds);

        token_transfer(ctx, amount)
    }

    fn validate(&self) -> Result<()> {
        require!(
            self.vault.check_status_flags(STATUS_CAN_COLLECT_INTEREST),
            ExponentCoreError::CollectingInterestDisabled
        );

        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, CollectInterest<'info>>,
    amount: Amount,
) -> Result<CollectInterestEventV2> {
    let amount_sy = amount.to_u64(ctx.accounts.yield_position.interest.staged)?;

    ctx.accounts
        .vault
        .claim_limits
        .verify_claim_limits(amount_sy, Clock::get()?.unix_timestamp as u32)?;

    // NOTE: User emission rewards are calculated based on the vault's current emission indexes
    // without updating the vault state first. This design ensures that collect_interest remains
    // operational during emergency mode, where vault state updates with abnormal exchange rates
    // could result in inflated emission calculations.
    //
    // As a consequence, users receive emission rewards only up to the last vault update timestamp.
    // To capture the most recent emission rewards, users should call stage_yield prior to
    // collect_interest to synchronize vault emission indexes with the current state.
    yield_position_earn(&mut ctx.accounts.vault, &mut ctx.accounts.yield_position);

    do_withdraw_sy(
        amount_sy,
        &ctx.accounts.address_lookup_table,
        &ctx.accounts.vault.cpi_accounts,
        &ctx.accounts.to_account_infos(),
        ctx.remaining_accounts,
        ctx.accounts.sy_program.key(),
        &[&ctx.accounts.vault.signer_seeds()],
    )?;

    let (user_sy, fee_sy) = handle_collect_interest(
        &mut ctx.accounts.vault,
        &mut ctx.accounts.yield_position,
        amount_sy,
    );

    ctx.accounts
        .transfer_sy(ctx.accounts.token_sy_dst.to_account_info(), user_sy)?;

    ctx.accounts.transfer_sy(
        ctx.accounts.treasury_sy_token_account.to_account_info(),
        fee_sy,
    )?;

    let event = CollectInterestEventV2 {
        user: ctx.accounts.owner.key(),
        vault: ctx.accounts.vault.key(),
        user_yield_position: ctx.accounts.yield_position.key(),
        amount_to_user: user_sy,
        amount_to_treasury: fee_sy,
        unix_timestamp: Clock::get()?.unix_timestamp,
        user_interest: ctx.accounts.yield_position.interest,
        user_emissions: ctx.accounts.yield_position.emissions.clone(),
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct CollectInterestEvent {
    pub user: Pubkey,
    pub vault: Pubkey,
    pub user_yield_position: Pubkey,
    pub amount_to_user: u64,
    pub amount_to_treasury: u64,
    pub unix_timestamp: i64,
}

#[event]
pub struct CollectInterestEventV2 {
    pub user: Pubkey,
    pub vault: Pubkey,
    pub user_yield_position: Pubkey,
    pub amount_to_user: u64,
    pub amount_to_treasury: u64,
    pub unix_timestamp: i64,
    pub user_interest: YieldTokenTracker,
    pub user_emissions: Vec<YieldTokenTracker>,
}

pub fn handle_collect_interest(
    vault: &mut Vault,
    yield_position: &mut YieldTokenPosition,
    amount_sy: u64,
) -> (u64, u64) {
    let (user_sy, fee_sy) = calc_collect_interest(amount_sy, vault.interest_bps_fee);

    // update the balances
    vault.dec_total_sy_in_escrow(amount_sy);

    yield_position.interest.collect(amount_sy);

    vault.dec_uncollected_sy(amount_sy);

    (user_sy, fee_sy)
}

fn calc_collect_interest(amount_sy: u64, interest_bps_fee: u16) -> (u64, u64) {
    let fee_sy = (amount_sy as u128 * interest_bps_fee as u128 + 9999) / 10000;
    let fee_sy = fee_sy as u64;

    let user_sy = amount_sy.checked_sub(fee_sy).unwrap();

    (user_sy, fee_sy)
}
