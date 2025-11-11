use amount_value::Amount;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface, Transfer};
use exponent_admin::Admin;

use crate::{
    cpi_common::to_account_metas, instructions::util::deserialize_lookup_table,
    util::token_transfer, utils::cpi_claim_emission, Vault, YieldTokenPosition,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum CollectTreasuryEmissionKind {
    YieldPosition,
    TreasuryEmission,
}

#[derive(Accounts)]
#[instruction(index: u16, amount: Amount, kind: CollectTreasuryEmissionKind)]
pub struct CollectTreasuryEmission<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// constrained by vault
    #[account(mut)]
    pub yield_position: Account<'info, YieldTokenPosition>,

    #[account(
        mut,
        has_one = authority,
        has_one = yield_position,
        has_one = address_lookup_table,
        has_one = sy_program
    )]
    pub vault: Account<'info, Vault>,

    /// CHECK: constrained by vault
    pub sy_program: UncheckedAccount<'info>,

    /// constrained by vault
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

    pub token_program: Interface<'info, TokenInterface>,

    pub admin: Account<'info, Admin>,
}

impl<'i> CollectTreasuryEmission<'i> {
    fn transfer_context(&self) -> CpiContext<'_, '_, '_, 'i, Transfer<'i>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.emission_escrow.to_account_info(),
                to: self.emission_dst.to_account_info(),
                authority: self.authority.to_account_info(),
            },
        )
    }

    fn transfer_emission(&self, amount: u64) -> Result<()> {
        let signer_seeds = &[&self.vault.signer_seeds()[..]];
        let ctx = self.transfer_context().with_signer(signer_seeds);

        token_transfer(ctx, amount)
    }

    fn validate(&self) -> Result<()> {
        self.admin
            .principles
            .exponent_core
            .is_admin(&self.signer.key)?;

        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(
    ctx: Context<CollectTreasuryEmission>,
    index: u16,
    amount: Amount,
    kind: CollectTreasuryEmissionKind,
) -> Result<()> {
    let lookup_table = deserialize_lookup_table(&ctx.accounts.address_lookup_table);
    let signer_seeds = &[&ctx.accounts.vault.signer_seeds()[..]];

    let amount_to_send = match kind {
        CollectTreasuryEmissionKind::YieldPosition => {
            amount.to_u64(ctx.accounts.yield_position.emissions[index as usize].staged)?
        }
        CollectTreasuryEmissionKind::TreasuryEmission => {
            amount.to_u64(ctx.accounts.vault.emissions[index as usize].treasury_emission)?
        }
    };

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

    ctx.accounts.transfer_emission(amount_to_send)?;

    match kind {
        CollectTreasuryEmissionKind::YieldPosition => {
            ctx.accounts.yield_position.emissions[index as usize].collect(amount_to_send);
        }
        CollectTreasuryEmissionKind::TreasuryEmission => {
            ctx.accounts.vault.emissions[index as usize].collect_treasury_emission(amount_to_send);
        }
    }

    Ok(())
}