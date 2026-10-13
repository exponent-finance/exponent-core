use crate::{
    cpi_common::to_account_metas, error::ExponentCoreError,
    instructions::util::deserialize_lookup_table, util::token_transfer, utils::cpi_claim_emission,
    Vault, YieldTokenPosition, YieldTokenTracker, STATUS_CAN_COLLECT_EMISSIONS,
};
use amount_value::Amount;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface, Transfer};

#[event_cpi]
#[derive(Accounts)]
#[instruction(index: u16, amount: Amount)]
pub struct CollectEmission<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = authority,
        has_one = address_lookup_table,
        has_one = sy_program,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        mut,
        has_one = owner,
        has_one = vault
    )]
    pub position: Account<'info, YieldTokenPosition>,

    /// CHECK: constrained by vault
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: constrained by vault
    pub authority: SystemAccount<'info>,

    #[account(
        mut,
        address = vault.emissions[index as usize].token_account,
    )]
    pub emission_escrow: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub emission_dst: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: constrained by vault
    pub address_lookup_table: UncheckedAccount<'info>,

    #[account(
        mut,
        address = vault.emissions[index as usize].treasury_token_account,
    )]
    pub treasury_emission_token_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: constrained by token accounts
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'i> CollectEmission<'i> {
    fn transfer_context(&self, to: AccountInfo<'i>) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.emission_escrow.to_account_info(),
                to,
                authority: self.authority.to_account_info(),
            },
        )
    }

    fn transfer_emission(&self, to: AccountInfo<'i>, amount: u64) -> Result<()> {
        let signer_seeds = &[&self.vault.signer_seeds()[..]];
        let ctx = self.transfer_context(to).with_signer(signer_seeds);

        token_transfer(ctx, amount)
    }

    fn validate(&self) -> Result<()> {
        require!(
            self.vault.check_status_flags(STATUS_CAN_COLLECT_EMISSIONS),
            ExponentCoreError::CollectingEmissionsDisabled
        );
        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(
    ctx: Context<CollectEmission>,
    index: u16,
    amount: Amount,
) -> Result<CollectEmissionEventV2> {
    let lookup_table = deserialize_lookup_table(&ctx.accounts.address_lookup_table);
    let signer_seeds = &[&ctx.accounts.vault.signer_seeds()[..]];

    let amount_to_send = amount.to_u64(ctx.accounts.position.emissions[index as usize].staged)?;

    cpi_claim_emission(
        ctx.accounts.sy_program.key(),
        amount_to_send,
        ctx.remaining_accounts,
        to_account_metas(
            &ctx.accounts.vault.cpi_accounts.claim_emission[index as usize],
            &lookup_table,
        ),
        signer_seeds,
    )?;

    let (user_amount, treasury_amount) = handle_collect_emission(
        amount_to_send,
        ctx.accounts.vault.emissions[index as usize].fee_bps,
    )?;

    ctx.accounts.position.emissions[index as usize].collect(amount_to_send);

    // Transfer emissions to the user
    ctx.accounts
        .transfer_emission(ctx.accounts.emission_dst.to_account_info(), user_amount)?;

    if treasury_amount > 0 {
        // Transfer fee emissions to the treasury
        ctx.accounts.transfer_emission(
            ctx.accounts
                .treasury_emission_token_account
                .to_account_info(),
            treasury_amount,
        )?;
    }

    let event = CollectEmissionEventV2 {
        user: *ctx.accounts.owner.to_account_info().key,
        vault: *ctx.accounts.vault.to_account_info().key,
        position: *ctx.accounts.position.to_account_info().key,
        emission_index: index,
        amount_to_user: user_amount,
        amount_to_treasury: treasury_amount,
        unix_timestamp: Clock::get()?.unix_timestamp,
        user_interest: ctx.accounts.position.interest,
        user_emissions: ctx.accounts.position.emissions.clone(),
    };

    emit_cpi!(event);

    Ok(event)
}

#[event]
pub struct CollectEmissionEvent {
    pub user: Pubkey,
    pub vault: Pubkey,
    pub position: Pubkey,
    pub emission_index: u16,
    pub amount_to_user: u64,
    pub amount_to_treasury: u64,
    pub unix_timestamp: i64,
}

#[event]
pub struct CollectEmissionEventV2 {
    pub user: Pubkey,
    pub vault: Pubkey,
    pub position: Pubkey,
    pub emission_index: u16,
    pub amount_to_user: u64,
    pub amount_to_treasury: u64,
    pub unix_timestamp: i64,
    pub user_interest: YieldTokenTracker,
    pub user_emissions: Vec<YieldTokenTracker>,
}

fn handle_collect_emission(amount_to_send: u64, fee_bps: u16) -> Result<(u64, u64)> {
    let fee_bps_u64 = fee_bps as u64;

    let treasury_amount = (amount_to_send * fee_bps_u64 + 9999) / 10000;

    // Calculate the user amount rounded down
    let user_amount = amount_to_send.saturating_sub(treasury_amount);

    // Assert that the sum of user_amount and treasury_amount equals amount_to_send
    assert!(
        user_amount + treasury_amount == amount_to_send,
        "The sum of user_amount and treasury_amount should be equal to amount_to_send"
    );

    Ok((user_amount, treasury_amount))
}
