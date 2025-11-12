use anchor_lang::prelude::*;
use anchor_lang::prelude::InterfaceAccount;
use anchor_spl::token_interface::Mint;
use exponent_admin::Admin;
use mpl_token_metadata::{
    instructions::{CreateMetadataAccountV3Cpi, CreateMetadataAccountV3CpiAccounts},
    types::DataV2,
};
use crate::MarketTwo;

#[derive(Accounts)]
pub struct AddLpTokensMetadata<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub admin: Box<Account<'info, Admin>>,

    #[account(has_one = mint_lp)]
    pub market: Box<Account<'info, MarketTwo>>,

    pub mint_lp: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: created through CPI
    #[account(
        mut,
        seeds = [
            b"metadata",
            mpl_token_metadata::ID.as_ref(),
            mint_lp.key().as_ref()
        ],
        seeds::program = mpl_token_metadata::ID,
        bump
    )]
    pub metadata: UncheckedAccount<'info>,

    /// CHECK: validated by address constraint
    #[account(address = mpl_token_metadata::ID)]
    pub token_metadata_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> AddLpTokensMetadata<'info> {
    fn validate(&self) -> Result<()> {
        self.admin
            .principles
            .exponent_core
            .is_admin(self.payer.key)?;

        Ok(())
    }

    fn create_metadata(&self, name: String, symbol: String, uri: String) -> Result<()> {
        let accounts = CreateMetadataAccountV3CpiAccounts {
            metadata: &self.metadata.to_account_info(),
            mint: &self.mint_lp.to_account_info(),
            mint_authority: &self.market.to_account_info(),
            payer: &self.payer.to_account_info(),
            update_authority: (&self.payer.to_account_info(), true),
            system_program: &self.system_program.to_account_info(),
            rent: None,
        };

        let token_metadata_program = self.token_metadata_program.to_account_info();
        let create_metadata_cpi = CreateMetadataAccountV3Cpi::new(
            &token_metadata_program,
            accounts,
            mpl_token_metadata::instructions::CreateMetadataAccountV3InstructionArgs {
                data: DataV2 {
                    uri,
                    name,
                    symbol,
                    seller_fee_basis_points: 0,
                    collection: None,
                    creators: None,
                    uses: None,
                },
                is_mutable: true,
                collection_details: None,
            },
        );

        create_metadata_cpi.invoke_signed(&[&self.market.signer_seeds()])?;

        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(
    ctx: Context<AddLpTokensMetadata>,
    name: String,
    symbol: String,
    uri: String,
) -> Result<()> {
    ctx.accounts.create_metadata(name, symbol, uri)
}
