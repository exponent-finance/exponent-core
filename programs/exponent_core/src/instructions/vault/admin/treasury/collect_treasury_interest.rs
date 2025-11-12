use crate::{
    instructions::util::deserialize_lookup_table, state::*, util::token_transfer,
    utils::cpi_withdraw_sy,
};
use amount_value::Amount;
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_2022::Transfer, token_interface::TokenAccount};
use cpi_common::to_account_metas;
use exponent_admin::Admin;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum CollectTreasuryInterestKind {
    YieldPosition,
    TreasuryInterest,
}

/// This is a copy of the CollectInterest instruction, but for the treasury with admin checks
#[derive(Accounts)]
#[instruction(amount: Amount, kind: CollectTreasuryInterestKind)]
pub struct CollectTreasuryInterest<'info> {
    /// CHECK: constrained by admin
    #[account(mut)]
    pub signer: Signer<'info>,

    /// CHECK: constrained by vault
    #[account(mut)]
    pub yield_position: Box<Account<'info, YieldTokenPosition>>,

    /// The vault that holds the SY tokens in escrow
    #[account(
        mut,
        has_one = escrow_sy,
        has_one = authority,
        has_one = address_lookup_table,
        has_one = yield_position,
        has_one = sy_program,
    )]
    pub vault: Account<'info, Vault>,

    /// The receiving token account for SY withdrawn
    #[account(mut)]
    pub sy_dst: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub escrow_sy: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: constrained by vault
    pub authority: SystemAccount<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: constrained by vault
    pub sy_program: UncheckedAccount<'info>,

    /// CHECK: constrained by vault
    pub address_lookup_table: UncheckedAccount<'info>,

    pub admin: Account<'info, Admin>,
}

impl<'i> CollectTreasuryInterest<'i> {
    fn transfer_context(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.escrow_sy.to_account_info(),
                to: self.sy_dst.to_account_info(),
                authority: self.authority.to_account_info(),
            },
        )
    }

    fn transfer_sy(&self, amount: u64) -> Result<()> {
        let signer_seeds = &[&self.vault.signer_seeds()[..]];
        let ctx = self.transfer_context().with_signer(signer_seeds);

        token_transfer(ctx, amount)
    }

    fn validate(&self) -> Result<()> {
        self.admin
            .principles
            .exponent_core
            .is_admin(self.signer.key)?;

        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(
    ctx: Context<CollectTreasuryInterest>,
    amount: Amount,
    kind: CollectTreasuryInterestKind,
) -> Result<()> {
    let lookup_table = deserialize_lookup_table(&ctx.accounts.address_lookup_table);

    let amount_to_send = match kind {
        CollectTreasuryInterestKind::YieldPosition => {
            amount.to_u64(ctx.accounts.yield_position.interest.staged)?
        }
        CollectTreasuryInterestKind::TreasuryInterest => {
            amount.to_u64(ctx.accounts.vault.treasury_sy)?
        }
    };

    cpi_withdraw_sy(
        ctx.accounts.sy_program.key(),
        amount_to_send,
        ctx.remaining_accounts,
        to_account_metas(&ctx.accounts.vault.cpi_accounts.withdraw_sy, &lookup_table),
        &[&ctx.accounts.vault.signer_seeds()],
    )?;

    ctx.accounts.transfer_sy(amount_to_send)?;

    match kind {
        CollectTreasuryInterestKind::YieldPosition => {
            ctx.accounts.yield_position.interest.collect(amount_to_send);
            ctx.accounts.vault.dec_uncollected_sy(amount_to_send);
        }
        CollectTreasuryInterestKind::TreasuryInterest => {
            ctx.accounts.vault.collect_treasury_interest(amount_to_send);
        }
    }

    ctx.accounts.vault.dec_total_sy_in_escrow(amount_to_send);

    // TODO: event
    Ok(())
}
