use anchor_lang::prelude::*;
use exponent_admin::Admin;

use crate::{cpi_common::CpiAccounts, LiquidityNetBalanceLimits, MarketTwo};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum MarketAdminAction {
    SetStatus(u8),
    SetMaxLpSupply(u64),
    ChangeTreasuryTradeSyBpsFee(u16),
    ChangeLnFeeRateRoot(f64),
    ChangeRateScalarRoot(f64),
    ChangeCpiAccounts {
        cpi_accounts: CpiAccounts,
    },
    ChangeLiquidityNetBalanceLimits {
        max_net_balance_change_negative_percentage: u16,
        max_net_balance_change_positive_percentage: u32,
        window_duration_seconds: u32,
    },
    ChangeAddressLookupTable(Pubkey),
    RemoveMarketEmission(u8),
}

#[derive(Accounts)]
pub struct ModifyMarketSetting<'info> {
    #[account(mut)]
    pub market: Account<'info, MarketTwo>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub admin_state: Account<'info, Admin>,

    pub system_program: Program<'info, System>,
}

impl ModifyMarketSetting<'_> {
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }
}

#[access_control(ctx.accounts.validate())]
pub fn handler(ctx: Context<ModifyMarketSetting>, action: MarketAdminAction) -> Result<()> {
    let market = &mut ctx.accounts.market;

    match action {
        MarketAdminAction::SetStatus(new_status) => {
            ctx.accounts
                .admin_state
                .principles
                .change_status_flags
                .is_admin(ctx.accounts.signer.key)?;

            market.status_flags = new_status;
        }
        MarketAdminAction::SetMaxLpSupply(max_supply) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            market.max_lp_supply = max_supply;
        }
        MarketAdminAction::ChangeTreasuryTradeSyBpsFee(new_treasury_trade_sy_bps_fee) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            assert!(
                new_treasury_trade_sy_bps_fee <= 10000,
                "Treasury trade SY BPS fee must be less than or equal to 10000"
            );

            market.fee_treasury_sy_bps = new_treasury_trade_sy_bps_fee;
        }
        MarketAdminAction::ChangeLnFeeRateRoot(new_ln_fee_rate_root) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            market.financials.ln_fee_rate_root = new_ln_fee_rate_root;
        }
        MarketAdminAction::ChangeRateScalarRoot(new_rate_scalar_root) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            market.financials.rate_scalar_root = new_rate_scalar_root;
        }
        MarketAdminAction::ChangeCpiAccounts { cpi_accounts } => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            let old_size = market.to_account_info().data_len();
            let new_size = MarketTwo::size_of(
                &cpi_accounts,
                market.emissions.trackers.len(),
                market.lp_farm.farm_emissions.len(),
            );

            if new_size > old_size {
                let additional_rent = Rent::get()?.minimum_balance(new_size - old_size);
                anchor_lang::system_program::transfer(
                    CpiContext::new(
                        ctx.accounts.system_program.to_account_info(),
                        anchor_lang::system_program::Transfer {
                            from: ctx.accounts.signer.to_account_info(),
                            to: market.to_account_info(),
                        },
                    ),
                    additional_rent,
                )?;
            }

            market.to_account_info().realloc(new_size, false)?;
            market.cpi_accounts = cpi_accounts;
        }
        MarketAdminAction::ChangeLiquidityNetBalanceLimits {
            max_net_balance_change_negative_percentage,
            max_net_balance_change_positive_percentage,
            window_duration_seconds,
        } => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            market.liquidity_net_balance_limits = LiquidityNetBalanceLimits {
                max_net_balance_change_negative_percentage,
                max_net_balance_change_positive_percentage,
                window_duration_seconds,
                window_start_timestamp: Clock::get()?.unix_timestamp as u32,
                window_start_net_balance: market
                    .liquidity_net_balance_limits
                    .window_start_net_balance,
            };
        }
        MarketAdminAction::ChangeAddressLookupTable(address_lookup_table) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            market.address_lookup_table = address_lookup_table;
        }
        MarketAdminAction::RemoveMarketEmission(emission_index) => {
            ctx.accounts
                .admin_state
                .principles
                .exponent_core
                .is_admin(ctx.accounts.signer.key)?;

            market.emissions.trackers.remove(emission_index as usize);
        }
    }
    Ok(())
}