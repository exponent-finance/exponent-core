use anchor_lang::prelude::*;
pub mod error;
mod instructions;
pub mod seeds;
pub mod state;
pub mod utils;
use amount_value::Amount;
use cpi_common::CpiAccounts;
use instructions::*;
use precise_number::Number;
#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;
pub use state::*;

#[cfg(all(not(feature = "idl-build"), not(test)))]
mod allocator;

declare_id!("ExponentnaRg3CQbW6dqQNZKXp7gtZ9DGMp1cwC4HAS7");

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Exponent Finance",
    project_url: "https://exponent.finance",
    contacts: "email:v@exponentlabs.xyz,email:security@exponentlabs.xyz,link:https://docs.exponent.finance/security/bug-bounty,telegram:valentinmadrid",
    policy: "https://docs.exponent.finance/security/bug-bounty",
    preferred_languages: "en,de,fr",
    auditors: "Ottersec, Offside Labs"
}

#[program]
pub mod exponent_core {

    use super::*;

    /// High-trust function to init vault
    #[instruction(discriminator = [2])]
    pub fn initialize_vault(
        ctx: Context<InitializeVault>,
        start_timestamp: u32,
        duration: u32,
        interest_bps_fee: u16,
        cpi_accounts: CpiAccounts,
        min_op_size_strip: u64,
        min_op_size_merge: u64,
        pt_metadata_name: String,
        pt_metadata_symbol: String,
        pt_metadata_uri: String,
    ) -> Result<()> {
        initialize_vault::handler(
            ctx,
            start_timestamp,
            duration,
            interest_bps_fee,
            cpi_accounts,
            min_op_size_strip,
            min_op_size_merge,
            pt_metadata_name,
            pt_metadata_symbol,
            pt_metadata_uri,
        )
    }

    #[instruction(discriminator = [3])]
    pub fn initialize_yield_position(
        ctx: Context<InitializeYieldPosition>,
    ) -> Result<InitializeYieldPositionEvent> {
        initialize_yield_position::handler(ctx)
    }

    /// Strip SY into PT + YT
    #[instruction(discriminator = [4])]
    pub fn strip<'info>(
        ctx: Context<'_, '_, '_, 'info, Strip<'info>>,
        amount: u64,
    ) -> Result<StripEvent> {
        strip::handler(ctx, amount)
    }

    /// Merge PT + YT into SY
    /// Redeems & burns them, in exchange for SY
    #[instruction(discriminator = [5])]
    pub fn merge<'info>(
        ctx: Context<'_, '_, '_, 'info, Merge<'info>>,
        amount: u64,
    ) -> Result<MergeEvent> {
        merge::handler(ctx, amount)
    }

    #[instruction(discriminator = [6])]
    pub fn collect_interest<'info>(
        ctx: Context<'_, '_, '_, 'info, CollectInterest<'info>>,
        amount: Amount,
    ) -> Result<CollectInterestEventV2> {
        collect_interest::handler(ctx, amount)
    }

    /// Deposit YT into escrow in order to earn rewards & SY interest
    #[instruction(discriminator = [7])]
    pub fn deposit_yt(ctx: Context<DepositYt>, amount: u64) -> Result<DepositYtEventV2> {
        deposit_yt::handler(ctx, amount)
    }

    #[instruction(discriminator = [8])]
    pub fn withdraw_yt(ctx: Context<WithdrawYt>, amount: u64) -> Result<WithdrawYtEventV2> {
        withdraw_yt::handler(ctx, amount)
    }

    #[instruction(discriminator = [9])]
    pub fn stage_yt_yield(ctx: Context<StageYield>) -> Result<StageYieldEventV2> {
        stage_yield::handler(ctx)
    }

    #[instruction(discriminator = [10])]
    pub fn init_market_two<'info>(
        ctx: Context<'_, '_, '_, 'info, MarketTwoInit<'info>>,
        ln_fee_rate_root: f64,
        rate_scalar_root: f64,
        init_rate_anchor: f64,
        sy_exchange_rate: Number,
        pt_init: u64,
        sy_init: u64,
        fee_treasury_sy_bps: u16,
        cpi_accounts: CpiAccounts,
        seed_id: u8,
    ) -> Result<()> {
        market_two_init::handler(
            ctx,
            ln_fee_rate_root,
            rate_scalar_root,
            init_rate_anchor,
            sy_exchange_rate,
            pt_init,
            sy_init,
            fee_treasury_sy_bps,
            cpi_accounts,
            seed_id,
        )
    }

    #[instruction(discriminator = [11])]
    pub fn market_two_deposit_liquidity<'info>(
        ctx: Context<'_, '_, '_, 'info, DepositLiquidity<'info>>,
        pt_intent: u64,
        sy_intent: u64,
        min_lp_out: u64,
    ) -> Result<DepositLiquidityEvent> {
        instructions::market_two::deposit_liquidity::handler(ctx, pt_intent, sy_intent, min_lp_out)
    }

    #[instruction(discriminator = [12])]
    pub fn market_two_withdraw_liquidity<'info>(
        ctx: Context<'_, '_, '_, 'info, WithdrawLiquidity<'info>>,
        lp_in: u64,
        min_pt_out: u64,
        min_sy_out: u64,
    ) -> Result<WithdrawLiquidityEvent> {
        instructions::market_two::withdraw_liquidity::handler(ctx, lp_in, min_pt_out, min_sy_out)
    }

    /// Initialize a LP position for a user to deposit LP tokens into
    #[instruction(discriminator = [13])]
    pub fn init_lp_position(ctx: Context<InitLpPosition>) -> Result<InitLpPositionEvent> {
        instructions::market_two::init_lp_position::handler(ctx)
    }

    /// Deposit LP tokens into a personal LP position account
    #[instruction(discriminator = [14])]
    pub fn market_deposit_lp(ctx: Context<DepositLp>, amount: u64) -> Result<DepositLpEventV2> {
        instructions::market_two::deposit_lp::handler(ctx, amount)
    }

    /// Withdraw LP tokens from personal LP position account
    #[instruction(discriminator = [15])]
    pub fn market_withdraw_lp(ctx: Context<WithdrawLp>, amount: u64) -> Result<WithdrawLpEventV2> {
        instructions::market_two::withdraw_lp::handler(ctx, amount)
    }

    /// Collect a staged emission
    #[instruction(discriminator = [16])]
    pub fn market_collect_emission(
        ctx: Context<MarketCollectEmission>,
        emission_index: u16,
    ) -> Result<MarketCollectEmissionEventV2> {
        instructions::market_two::market_collect_emission::handler(ctx, emission_index)
    }

    #[instruction(discriminator = [17])]
    pub fn trade_pt<'info>(
        ctx: Context<'_, '_, '_, 'info, TradePt<'info>>,
        net_trader_pt: i64,
        sy_constraint: i64,
    ) -> Result<TradePtEvent> {
        instructions::market_two::trade_pt::handler(ctx, net_trader_pt, sy_constraint)
    }

    /// Sell YT for SY
    #[instruction(discriminator = [1])]
    pub fn sell_yt<'i>(
        ctx: Context<'_, '_, '_, 'i, SellYt<'i>>,
        yt_in: u64,
        min_sy_out: u64,
    ) -> Result<SellYtEvent> {
        sell_yt::handler(ctx, yt_in, min_sy_out)
    }

    /// Buy YT with SY
    #[instruction(discriminator = [0])]
    pub fn buy_yt<'i>(
        ctx: Context<'_, '_, '_, 'i, BuyYt<'i>>,
        sy_in: u64,
        yt_out: u64,
    ) -> Result<BuyYtEvent> {
        buy_yt::handler(ctx, sy_in, yt_out)
    }

    #[instruction(discriminator = [18])]
    pub fn add_emission<'info>(
        ctx: Context<'_, '_, '_, 'info, AddEmission<'info>>,
        cpi_accounts: CpiAccounts,
        treasury_fee_bps: u16,
    ) -> Result<()> {
        add_emission::handler(ctx, cpi_accounts, treasury_fee_bps)
    }

    #[instruction(discriminator = [19])]
    pub fn collect_emission<'info>(
        ctx: Context<'_, '_, '_, 'info, CollectEmission<'info>>,
        index: u16,
        amount: Amount,
    ) -> Result<CollectEmissionEventV2> {
        collect_emission::handler(ctx, index, amount)
    }

    #[instruction(discriminator = [20])]
    pub fn collect_treasury_emission(
        ctx: Context<CollectTreasuryEmission>,
        emission_index: u16,
        amount: Amount,
        kind: CollectTreasuryEmissionKind,
    ) -> Result<()> {
        collect_treasury_emission::handler(ctx, emission_index, amount, kind)
    }

    #[instruction(discriminator = [21])]
    pub fn collect_treasury_interest<'i>(
        ctx: Context<'_, '_, '_, 'i, CollectTreasuryInterest<'i>>,
        amount: Amount,
        kind: CollectTreasuryInterestKind,
    ) -> Result<()> {
        collect_treasury_interest::handler(ctx, amount, kind)
    }

    #[instruction(discriminator = [22])]
    pub fn add_farm<'i>(
        ctx: Context<'_, '_, '_, 'i, AddFarm>,
        token_rate: u64,
        until_timestamp: u32,
    ) -> Result<()> {
        add_farm::handler(ctx, token_rate, until_timestamp)
    }

    #[instruction(discriminator = [23])]
    pub fn modify_farm<'i>(
        ctx: Context<'_, '_, '_, 'i, ModifyFarm>,
        until_timestamp: u32,
        new_rate: u64,
    ) -> Result<()> {
        modify_farm::handler(ctx, until_timestamp, new_rate)
    }

    #[instruction(discriminator = [24])]
    pub fn claim_farm_emissions<'i>(
        ctx: Context<'_, '_, '_, 'i, ClaimFarmEmissions>,
        amount: Amount,
    ) -> Result<ClaimFarmEmissionsEventV2> {
        claim_farm_emissions::handler(ctx, amount)
    }

    #[instruction(discriminator = [25])]
    pub fn add_market_emission<'i>(
        ctx: Context<'_, '_, '_, 'i, AddMarketEmission>,
        cpi_accounts: CpiAccounts,
    ) -> Result<()> {
        add_market_emission::handler(ctx, cpi_accounts)
    }

    #[instruction(discriminator = [26])]
    pub fn modify_vault_setting(
        ctx: Context<ModifyVaultSetting>,
        action: AdminAction,
    ) -> Result<()> {
        modify_vault_setting::handler(ctx, action)
    }

    #[instruction(discriminator = [27])]
    pub fn modify_market_setting<'i>(
        ctx: Context<'_, '_, '_, 'i, ModifyMarketSetting>,
        action: MarketAdminAction,
    ) -> Result<()> {
        modify_market_setting::handler(ctx, action)
    }

    // Wrappers

    /// Provide liquidity to a market starting with a base asset
    /// This instruction
    /// - deposits base asset for SY
    /// - strips a portion of the SY into PT & YT
    /// - provides the remaining SY with the PT into the market
    /// - keeps the YT
    ///
    /// # Arguments
    /// - `amount_base` - The amount of base asset to deposit
    /// - `min_lp_out` - The minimum amount of LP tokens to receive
    /// - `mint_base_accounts_until` - The index of the account to use for the base asset mint
    #[instruction(discriminator = [28])]
    pub fn wrapper_provide_liquidity<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperProvideLiquidity<'info>>,
        amount_base: u64,
        min_lp_out: u64,
        mint_base_accounts_until: u8,
    ) -> Result<()> {
        wrapper_provide_liquidity::handler(ctx, amount_base, min_lp_out, mint_base_accounts_until)
    }

    #[instruction(discriminator = [29])]
    pub fn wrapper_buy_pt<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperBuyPt<'info>>,
        pt_amount: u64,
        max_base_amount: u64,
        mint_sy_rem_accounts_until: u8,
    ) -> Result<()> {
        buy_pt::handler(ctx, pt_amount, max_base_amount, mint_sy_rem_accounts_until)
    }

    #[instruction(discriminator = [30])]
    pub fn wrapper_sell_pt<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperSellPt<'info>>,
        amount_pt: u64,
        min_base_amount: u64,
        redeem_sy_rem_accounts_until: u8,
    ) -> Result<()> {
        sell_pt::handler(
            ctx,
            amount_pt,
            min_base_amount,
            redeem_sy_rem_accounts_until,
        )
    }

    #[instruction(discriminator = [31])]
    pub fn wrapper_buy_yt<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperBuyYt<'info>>,
        // exact amount of YT the trader wants to buy
        yt_out: u64,
        // max base amount the trader is willing to spend
        max_base_amount: u64,
        // The number of accounts to be used for minting SY
        mint_sy_accounts_length: u8,
    ) -> Result<()> {
        wrapper_buy_yt::handler(ctx, yt_out, max_base_amount, mint_sy_accounts_length)
    }

    #[instruction(discriminator = [32])]
    pub fn wrapper_sell_yt<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperSellYt<'info>>,
        yt_amount: u64,
        min_base_amount: u64,
        redeem_sy_accounts_until: u8,
    ) -> Result<()> {
        wrapper_sell_yt::handler(ctx, yt_amount, min_base_amount, redeem_sy_accounts_until)
    }

    #[instruction(discriminator = [33])]
    pub fn wrapper_collect_interest<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperCollectInterest<'info>>,
        redeem_sy_accounts_length: u8,
    ) -> Result<()> {
        wrapper_collect_interest::handler(ctx, redeem_sy_accounts_length)
    }

    #[instruction(discriminator = [34])]
    pub fn wrapper_withdraw_liquidity<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperWithdrawLiquidity<'info>>,
        amount_lp: u64,
        sy_constraint: u64,
        redeem_sy_accounts_length: u8,
    ) -> Result<()> {
        wrapper_withdraw_liquidity::handler(
            ctx,
            amount_lp,
            sy_constraint,
            redeem_sy_accounts_length,
        )
    }

    #[instruction(discriminator = [35])]
    pub fn wrapper_withdraw_liquidity_classic<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperWithdrawLiquidityClassic<'info>>,
        amount_lp: u64,
        redeem_sy_accounts_length: u8,
    ) -> Result<()> {
        wrapper_withdraw_liquidity_classic::handler(ctx, amount_lp, redeem_sy_accounts_length)
    }

    #[instruction(discriminator = [36])]
    pub fn wrapper_provide_liquidity_base<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperProvideLiquidityBase<'info>>,
        amount_base: u64,
        min_lp_out: u64,
        mint_sy_accounts_until: u8,
        external_pt_to_buy: u64,
        external_sy_constraint: u64,
    ) -> Result<()> {
        wrapper_provide_liquidity_base::handler(
            ctx,
            amount_base,
            min_lp_out,
            mint_sy_accounts_until,
            external_pt_to_buy,
            external_sy_constraint,
        )
    }

    #[instruction(discriminator = [37])]
    pub fn wrapper_provide_liquidity_classic<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperProvideLiquidityClassic<'info>>,
        amount_base: u64,
        amount_pt: u64,
        min_lp_out: u64,
        mint_sy_accounts_until: u8,
    ) -> Result<()> {
        wrapper_provide_liquidity_classic::handler(
            ctx,
            amount_base,
            amount_pt,
            min_lp_out,
            mint_sy_accounts_until,
        )
    }

    #[instruction(discriminator = [38])]
    pub fn wrapper_strip<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperStrip<'info>>,
        amount_base: u64,
        mint_sy_accounts_until: u8,
    ) -> Result<()> {
        wrapper_strip::handler(ctx, amount_base, mint_sy_accounts_until)
    }

    #[instruction(discriminator = [39])]
    pub fn wrapper_merge<'info>(
        ctx: Context<'_, '_, '_, 'info, WrapperMerge<'info>>,
        amount_py: u64,
        redeem_sy_accounts_until: u8,
    ) -> Result<()> {
        wrapper_merge::handler(ctx, amount_py, redeem_sy_accounts_until)
    }

    #[instruction(discriminator = [40])]
    pub fn realloc_market<'info>(
        ctx: Context<'_, '_, '_, 'info, ReallocMarket<'info>>,
        additional_bytes: u64,
    ) -> Result<()> {
        realloc_market::handler(ctx, additional_bytes)
    }

    #[instruction(discriminator = [41])]
    pub fn add_lp_tokens_metadata(
        ctx: Context<AddLpTokensMetadata>,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        add_lp_tokens_metadata::handler(ctx, name, symbol, uri)
    }

}
