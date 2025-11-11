use anchor_lang::prelude::*;
use dec_num::DNum;
use exponent_time_curve::math::{exchange_rate_from_ln_implied_rate, fee_rate};
use precise_number::Number;
use sy_common::PositionState;

use crate::{cpi_common::CpiAccounts, error::ExponentCoreError};

/// Minimum size of market operations
/// Used to protect against rounding errors
pub const MIN_TX_SIZE: u64 = 10;

pub const STATUS_CAN_DEPOSIT_LIQUIDITY: u8 = 0b0000_0001;
pub const STATUS_CAN_WITHDRAW_LIQUIDITY: u8 = 0b0000_0010;
pub const STATUS_CAN_BUY_PT: u8 = 0b0000_0100;
pub const STATUS_CAN_SELL_PT: u8 = 0b0000_1000;
pub const STATUS_CAN_BUY_YT: u8 = 0b0001_0000;
pub const STATUS_CAN_SELL_YT: u8 = 0b0010_0000;

pub const ALL_FLAGS: u8 = STATUS_CAN_DEPOSIT_LIQUIDITY
    | STATUS_CAN_WITHDRAW_LIQUIDITY
    | STATUS_CAN_BUY_PT
    | STATUS_CAN_SELL_PT
    | STATUS_CAN_BUY_YT
    | STATUS_CAN_SELL_YT;

#[account]
pub struct MarketTwo {
    /// Address to ALT
    pub address_lookup_table: Pubkey,

    /// Mint of the vault's PT token
    pub mint_pt: Pubkey,

    /// Mint of the SY program's SY token
    pub mint_sy: Pubkey,

    /// Link to yield-stripping vault
    pub vault: Pubkey,

    /// Mint for the market's LP tokens
    pub mint_lp: Pubkey,

    /// Holds the LP tokens that are earning emissions
    /// This is where LP holders "stake" their LP tokens
    pub token_lp_escrow: Pubkey,

    /// Token account that holds PT liquidity
    pub token_pt_escrow: Pubkey,

    /// Pass-through token account for SY moving from the depositor to the SY program
    pub token_sy_escrow: Pubkey,

    /// Token account that holds SY fees from trade_pt
    pub token_fee_treasury_sy: Pubkey,

    /// Fee treasury SY BPS
    pub fee_treasury_sy_bps: u16,

    /// Authority for CPI calls owned by the market struct
    pub self_address: Pubkey,

    /// Bump for signing the PDA
    pub signer_bump: [u8; 1],

    pub status_flags: u8,

    /// Link to the SY program ID
    pub sy_program: Pubkey,

    pub financials: MarketFinancials,

    pub emissions: MarketEmissions,

    pub lp_farm: LpFarm,

    pub max_lp_supply: u64,

    pub lp_escrow_amount: u64,

    /// Record of CPI accounts
    pub cpi_accounts: CpiAccounts,

    pub is_current_flash_swap: bool,

    pub liquidity_net_balance_limits: LiquidityNetBalanceLimits,

    /// Unique seed id for the market
    pub seed_id: [u8; 1],
}

/// Financial parameters for the market
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Default)]
pub struct MarketFinancials {
    /// Expiration timestamp, which is copied from the vault associated with the PT
    pub expiration_ts: u64,

    /// Balance of PT in the market
    /// This amount is tracked separately to prevent bugs from token transfers directly to the market
    pub pt_balance: u64,

    /// Balance of SY in the market
    /// This amount is tracked separately to prevent bugs from token transfers directly to the market
    pub sy_balance: u64,

    /// Initial log of fee rate, which decreases over time
    pub ln_fee_rate_root: f64,

    /// Last seen log of implied rate (APY) for PT
    /// Used to maintain continuity of the APY between trades over time
    pub last_ln_implied_rate: f64,

    /// Initial rate scalar, which increases over time
    pub rate_scalar_root: f64,
}

impl MarketTwo {
    /// Seeds for deriving market PDA
    pub fn signer_seeds(&self) -> [&[u8]; 4] {
        if self.seed_id[0] == 0 {
            [b"market", self.vault.as_ref(), &[], &self.signer_bump]
        } else {
            [
                b"market",
                self.vault.as_ref(),
                &self.seed_id,
                &self.signer_bump,
            ]
        }
    }

    pub fn check_status_flags(&self, required_flags: u8) -> bool {
        self.status_flags & required_flags == required_flags
    }

    pub fn check_supply_lp(&self, lp_supply: u64) -> bool {
        lp_supply <= self.max_lp_supply
    }

    pub fn size_of(
        cpi_accounts: &CpiAccounts,
        emissions_len: usize,
        farm_emissions_len: usize,
    ) -> usize {
        // Get size of dynamic vectors in the CpiAccounts struct
        let cpi_accounts_size = cpi_accounts.try_to_vec().unwrap().len();

        let emissions_size = MarketEmissions::size_of_static(emissions_len);

        let farms_size = LpFarm::size_of_static(farm_emissions_len);

        // discriminator
        8 +

        // address_lookup_table
        32 +

        // mint_pt
        32 +

        // mint_sy
        32 +

        // vault
        32 +

        // mint_lp
        32 +

        // escrow_lp
        32 +

        // token_escrow_pt
        32 +

        // token_escrow_sy
        32 +

        // token_fee_treasury_sy
        32 +

        // fee_treasury_sy_bps
        2 +

        // self_address
        32 +

        // signer_bump
        1 +

        // status_flags
        1 +

        // link to sy program
        32 +

        // market_financials size
        MarketFinancials::SIZE_OF +

        emissions_size +

        farms_size +

        // max_lp_supply
        8 +

        // lp_escrow_amount
        8 +

        // cpi_accounts size
        cpi_accounts_size +

        // liquidity_net_balance_limits
        LiquidityNetBalanceLimits::SIZE_OF +

        // Seed id
        1
    }

    pub fn is_expired(&self, now: u64) -> bool {
        now > self.financials.expiration_ts
    }

    pub fn is_active(&self, now: u64) -> bool {
        !self.is_expired(now)
    }

    pub fn new(
        self_address: Pubkey,
        signer_bump: [u8; 1],
        expiration_ts: u64,
        ln_fee_rate_root: f64,
        rate_scalar_root: f64,
        init_rate_anchor: f64,
        pt_init: u64,
        sy_init: u64,
        sy_exchange_rate: Number,
        mint_pt: Pubkey,
        mint_sy: Pubkey,
        vault: Pubkey,
        mint_lp: Pubkey,
        token_pt_escrow: Pubkey,
        token_sy_escrow: Pubkey,
        token_lp_escrow: Pubkey,
        address_lookup_table: Pubkey,
        token_fee_treasury_sy: Pubkey,
        sy_program: Pubkey,
        cpi_accounts: CpiAccounts,
        treasury_fee_bps: u16,
        seed_id: u8,
    ) -> Self {
        // Calculate quantity of asset represented by SY tokens
        let asset = Number::from(sy_init) * sy_exchange_rate;
        // floor down
        let asset = asset.floor_u64();

        // get seconds remaining until expiry
        let sec_remaining = expiration_ts
            .checked_sub(Clock::get().unwrap().unix_timestamp as u64)
            .expect("Vault expired");
        // current rate scalar is based on time remaining
        let rate_scalar =
            exponent_time_curve::math::rate_scalar::<f64>(rate_scalar_root.into(), sec_remaining);

        // calculate implied rate (APY) based on state of curve
        let ln_implied_rate = exponent_time_curve::math::ln_implied_rate(
            pt_init,
            asset,
            rate_scalar,
            init_rate_anchor.into(),
            sec_remaining,
        );

        let emissions = MarketEmissions::default();

        let financials = MarketFinancials {
            expiration_ts,
            pt_balance: pt_init,
            sy_balance: sy_init,
            rate_scalar_root,
            ln_fee_rate_root,
            last_ln_implied_rate: ln_implied_rate,
        };

        // Make sure treasury fee bps is less than 100%
        assert!(treasury_fee_bps < 10000, "Treasury fee BPS is too high");
        assert!(seed_id != 0, "New seed id cannot be zero");
        Self {
            self_address,
            signer_bump,
            mint_pt,
            mint_sy,
            vault,
            mint_lp,
            token_lp_escrow,
            token_pt_escrow,
            token_sy_escrow,
            token_fee_treasury_sy,
            cpi_accounts,
            address_lookup_table,
            sy_program,
            financials,
            emissions,
            max_lp_supply: u64::MAX,
            // default status is all on
            status_flags: ALL_FLAGS,
            lp_farm: LpFarm::default(),
            lp_escrow_amount: 0,
            fee_treasury_sy_bps: treasury_fee_bps,
            is_current_flash_swap: false,
            liquidity_net_balance_limits: LiquidityNetBalanceLimits {
                max_net_balance_change_negative_percentage: 10000,
                max_net_balance_change_positive_percentage: u32::MAX,
                window_start_timestamp: Clock::get().unwrap().unix_timestamp as u32,
                window_duration_seconds: 0,
                window_start_net_balance: 0,
            },
            seed_id: [seed_id],
        }
    }

    pub fn update_emissions_from_position_state(
        &mut self,
        position_state: &PositionState,
        lp_staked: u64,
    ) {
        for (index, current_position) in position_state.emissions.iter().enumerate() {
            let difference =
                current_position.amount_claimable - self.emissions.trackers[index].last_seen_staged;

            let amount_to_increase = Number::from_natural_u64(difference)
                .checked_div(&Number::from_natural_u64(lp_staked))
                .unwrap_or(Number::ZERO);

            self.emissions.trackers[index].lp_share_index += amount_to_increase;

            self.emissions.trackers[index].last_seen_staged = current_position.amount_claimable;
        }
    }

    pub fn add_farm(&mut self, token_rate: u64, expiry_ts: u32, token_mint: &Pubkey) {
        self.lp_farm.farm_emissions.push(FarmEmission {
            mint: *token_mint,
            token_rate,
            expiry_timestamp: expiry_ts,
            index: Number::ZERO,
        })
    }
}

impl MarketFinancials {
    pub const SIZE_OF: usize =
        // expiration_ts
        8 +
        // pt_balance
        8 +
        // sy_balance
        8 +
        // ln_fee_rate_root 
        16 +
        // last_ln_implied_rate
        16 +
        // rate_scalar_root
        16;

    /// Used for direct manipulation when swapping with borrowed funds
    pub fn dec_pt_balance(&mut self, amt: u64) {
        self.pt_balance = self
            .pt_balance
            .checked_sub(amt)
            .expect("pt balance underflow");
    }

    pub fn inc_pt_balance(&mut self, amt: u64) {
        self.pt_balance = self
            .pt_balance
            .checked_add(amt)
            .expect("pt balance overflow");
    }

    pub fn dec_sy_balance(&mut self, amt: u64) {
        self.sy_balance = self
            .sy_balance
            .checked_sub(amt)
            .expect("sy balance underflow");
    }

    pub fn inc_sy_balance(&mut self, amt: u64) {
        self.sy_balance = self
            .sy_balance
            .checked_add(amt)
            .expect("sy balance overflow");
    }

    fn sec_remaining(&self, now: u64) -> u64 {
        self.expiration_ts.saturating_sub(now)
    }

    /// Calculate asset balance from the SY balance and exchange rate
    fn asset_balance(&self, sy_exchange_rate: Number) -> Number {
        Number::from_natural_u64(self.sy_balance) * sy_exchange_rate
    }

    /// Calculate the current rate anchor
    fn current_rate_anchor(&self, sy_exchange_rate: Number, now: u64) -> f64 {
        let sec_remaining = self.sec_remaining(now);
        let asset = self.asset_balance(sy_exchange_rate).floor_u64();
        let current_rate_scalar = self.current_rate_scalar(now);
        exponent_time_curve::math::find_rate_anchor(
            self.pt_balance,
            asset,
            current_rate_scalar,
            self.last_ln_implied_rate.into(),
            sec_remaining,
        )
    }

    /// Calculate the current rate scalar
    fn current_rate_scalar(&self, now: u64) -> f64 {
        let sec_remaining = self.sec_remaining(now);
        exponent_time_curve::math::rate_scalar::<f64>(self.rate_scalar_root, sec_remaining)
    }

    /// Calculate the current fee rate base on the decay from the initial fee rate
    fn cur_fee_rate(&self, now: u64) -> f64 {
        fee_rate(self.ln_fee_rate_root.into(), self.sec_remaining(now))
    }

    /// Calculate SY change from PT trade
    /// And update the state of the market
    /// - change sy balance
    /// - change pt balance
    /// - change last_ln_implied_rate
    ///
    /// # Arguments
    /// - `sy_exchange_rate` - The exchange rate of the SY token to the base asset
    /// - `net_trader_pt` - The net PT change to the trader
    /// - `now` - The current unix timestamp
    /// - `fee_treasury_sy_bps` - The treasury fee in basis points
    pub fn trade_pt(
        &mut self,
        sy_exchange_rate: Number,
        net_trader_pt: i64,
        now: u64,
        is_current_flash_swap: bool,
        fee_treasury_sy_bps: u16,
    ) -> TradeResult {
        // if the net pt to the trader is positive, he is buying
        let is_buy = net_trader_pt > 0;
        // Get the market liquidity in terms of base asset

        // ceil on asset balance when buying PT (make asset cheaper)
        // floor on asset balance when selling PT (make asset more expensive)
        let asset_balance = self.asset_balance(sy_exchange_rate);

        let asset_balance = if is_buy {
            asset_balance.ceil_u64()
        } else {
            asset_balance.floor_u64()
        };

        // Pre-compute the current rate scalar and rate anchor
        let current_rate_scalar = self.current_rate_scalar(now);
        let current_rate_anchor = self.current_rate_anchor(sy_exchange_rate, now);
        let current_fee_rate = self.cur_fee_rate(now);

        // Calculate the trade result
        let trade_result = exponent_time_curve::math::trade(
            self.pt_balance,
            asset_balance,
            current_rate_scalar,
            current_rate_anchor,
            current_fee_rate,
            net_trader_pt as f64,
            is_current_flash_swap,
        );

        // calc the abs magnitude of the trade in
        let net_trader_sy =
            net_trader_sy_from_net_trader_asset(trade_result.net_trader_asset, sy_exchange_rate);

        // the actual change to the market's sy balance is the same as the net change to the trader
        // if (eventually) a platform fee is taken from the trade, then the market's change in SY balance needs to account for this withdrawal
        let market_sy_change = net_trader_sy.abs() as u64;

        // Convert fee to SY units
        let sy_fee = sy_fee_from_asset_fee(trade_result.asset_fee, sy_exchange_rate);

        // Calculate treasury fee amount
        let treasury_fee_amount = (sy_fee * fee_treasury_sy_bps as u64) / 10_000;

        // Handle changes to market liquidity balances
        if is_buy {
            // Buying PT

            // market PT balance goes down
            self.dec_pt_balance(net_trader_pt as u64);

            // market SY balance goes up
            self.inc_sy_balance(market_sy_change);
        } else {
            // Selling PT

            // market PT balance goes up
            self.inc_pt_balance((-net_trader_pt) as u64);

            // market SY balance goes down
            self.dec_sy_balance(market_sy_change);
        }

        // Deduct treasury fee from SY balance
        self.dec_sy_balance(treasury_fee_amount);

        // set the new ln implied rate based on the new proportion AFTER all balance adjustments
        let new_ln_implied_rate = exponent_time_curve::math::ln_implied_rate(
            self.pt_balance,
            self.asset_balance(sy_exchange_rate).floor_u64(),
            current_rate_scalar,
            current_rate_anchor,
            self.sec_remaining(now),
        );

        self.last_ln_implied_rate = new_ln_implied_rate.into();

        TradeResult {
            sy_fee,
            net_trader_sy,
            net_trader_pt,
            treasury_fee_amount,
        }
    }

    pub fn exchange_rate(&self, unix_timestamp: u64) -> f64 {
        exchange_rate_from_ln_implied_rate::<f64>(
            self.last_ln_implied_rate.into(),
            self.sec_remaining(unix_timestamp),
        )
    }

    pub fn lp_price_in_asset(
        &self,
        unix_timestamp: u64,
        sy_exchange_rate: Number,
        lp_supply: u64,
    ) -> f64 {
        // Convert SY balance to asset value
        let sy_asset_value = Number::from_natural_u64(self.sy_balance) * sy_exchange_rate;

        let exchange_rate = self.exchange_rate(unix_timestamp);
        let pt_exchange_rate =
            Number::from_ratio((exchange_rate * 1e18) as u128, 1_000_000_000_000_000_000);

        // Convert PT balance to asset value using Number for precision
        let pt_balance = Number::from_natural_u64(self.pt_balance);
        let pt_asset_value = pt_balance / pt_exchange_rate;

        // Calculate total TVL and price per LP token using Number
        let liquidity_pool_tvl = sy_asset_value + pt_asset_value;
        let lp_supply = Number::from_natural_u64(lp_supply);
        let price = liquidity_pool_tvl / lp_supply;

        price.to_f64().unwrap()
    }

    pub fn add_liquidity(
        &mut self,
        sy_intent: u64,
        pt_intent: u64,
        lp_supply: u64,
    ) -> LiqAddResult {
        // assert!(sy_intent >= MIN_TX_SIZE, "SY intent too small");
        // assert!(pt_intent >= MIN_TX_SIZE, "PT intent too small");

        let r = exponent_time_curve::math::add_liquidity::<f64>(
            sy_intent,
            pt_intent,
            lp_supply,
            self.sy_balance,
            self.pt_balance,
        );

        self.inc_pt_balance(r.pt_in);
        self.inc_sy_balance(r.sy_in);

        LiqAddResult {
            pt_in: r.pt_in,
            sy_in: r.sy_in,
            lp_out: r.lp_tokens_out,
        }
    }

    pub fn rm_liquidity(&mut self, lp_in: u64, lp_supply: u64) -> LiqRmResult {
        // assert!(lp_in >= MIN_TX_SIZE, "LP intent too small");
        assert!(lp_in <= lp_supply, "LP intent too large");

        let r = exponent_time_curve::math::rm_liquidity::<f64>(
            lp_in,
            lp_supply,
            self.sy_balance,
            self.pt_balance,
        );

        self.dec_pt_balance(r.pt_out);
        self.dec_sy_balance(r.sy_out);

        LiqRmResult {
            pt_out: r.pt_out,
            sy_out: r.sy_out,
        }
    }

    /// Calc amount of SY owned by LP tokens
    pub fn lp_to_sy(&self, lp_amount: u64, lp_supply: u64) -> u64 {
        exponent_time_curve::math::lp_to_sy::<f64>(
            lp_amount,
            lp_supply,
            self.sy_balance,
            self.pt_balance,
        )
    }
}

fn sy_magnitude_from_net_trader_asset(net_trader_asset: f64, sy_exchange_rate: Number) -> u64 {
    // taking the floor before the absolute value is important
    // if net_trader_asset is negative, we want to floor down towards -inf
    // the reason for this is that: the trader is buying PT with asset, and so should be charged more asset

    // if net_trader_asset is positive, we want to floor down towards 0
    // this is because the trader is selling PT for asset, and so should be paid less asset

    // the floor function returns the largest integer less than or equal to the number
    // Example: -8.45 goes to -9

    let is_negative = net_trader_asset.is_sign_negative();

    let asset_magnitude: u64 =
        f64_to_u64_checked(net_trader_asset.floor().abs()).expect("f64 overflow for u64");

    let sy_magnitude = Number::from_natural_u64(asset_magnitude) / sy_exchange_rate;

    if is_negative {
        sy_magnitude.ceil_u64()
    } else {
        sy_magnitude.floor_u64()
    }
}

fn f64_to_u64_checked(value: f64) -> Option<u64> {
    // Check for invalid values: NaN, infinity, or negative numbers
    if !value.is_finite() || value < 0.0 {
        return None;
    }

    // Check if the value exceeds the maximum u64 value
    if value > u64::MAX as f64 {
        return None; // Overflow
    }

    // Perform the conversion safely
    Some(value as u64)
}

fn net_trader_sy_from_net_trader_asset(net_trader_asset: f64, sy_exchange_rate: Number) -> i64 {
    let sy_magnitude = sy_magnitude_from_net_trader_asset(net_trader_asset, sy_exchange_rate);
    // buying PT means the trader is losing SY
    let is_buy = net_trader_asset.is_sign_negative();

    if is_buy {
        // the trader is buying PT
        // so their net sy change is negative
        <u64 as TryInto<i64>>::try_into(sy_magnitude).expect("u64 overflow for i64") * -1
    } else {
        // the trader is selling PT
        // so their net sy change is positive
        sy_magnitude.try_into().expect("u64 overflow for i64")
    }
}

/// Convert fee units from asset to SY units
fn sy_fee_from_asset_fee(asset_fee: f64, sy_exchange_rate: Number) -> u64 {
    let sy_exchange_rate = sy_exchange_rate.to_f64().unwrap();
    let sy_fee = (asset_fee / sy_exchange_rate).floor();
    f64_to_u64_checked(sy_fee).expect("f64 overflow for u64")
}

#[derive(Debug)]
pub struct TradeResult {
    /// The change to trader's PT balance and market's PT liquidity
    pub net_trader_pt: i64,
    /// The change to the trader's SY balance and market's SY liquidity
    pub net_trader_sy: i64,
    /// The part of the trade that was a fee
    pub sy_fee: u64,
    /// The treasury fee amount that was deducted from SY balance
    pub treasury_fee_amount: u64,
}

/// Binary serialized DNum
#[derive(Clone, Copy, Default, Debug, AnchorDeserialize, AnchorSerialize)]
pub struct AnchorDecNum(pub [u8; 16]);

impl From<DNum> for AnchorDecNum {
    fn from(value: DNum) -> Self {
        let bs = value.value.serialize();
        AnchorDecNum(bs)
    }
}

impl Into<DNum> for AnchorDecNum {
    fn into(self) -> DNum {
        DNum::deserialize(&self.0)
    }
}

#[derive(AnchorDeserialize, AnchorSerialize, Default, Clone)]
pub struct LiquidityNetBalanceLimits {
    pub window_start_timestamp: u32,
    pub window_start_net_balance: u64,
    /// Maximum allowed negative change in basis points (10000 = 100%)
    pub max_net_balance_change_negative_percentage: u16,
    /// Maximum allowed positive change in basis points (10000 = 100%)
    /// Using u32 to allow for very large increases (up to ~429,496%)
    pub max_net_balance_change_positive_percentage: u32,
    pub window_duration_seconds: u32,
}

impl LiquidityNetBalanceLimits {
    pub const SIZE_OF: usize =
        // window_start_timestamp
        4 +
        // window_start_net_balance
        8 +
        // max_net_balance_change_negative_percentage
        2 +
        // max_net_balance_change_positive_percentage
        4 +
        // window_duration_seconds
        4;

    /// Verifies that the proposed change in net balance doesn't exceed limits
    /// * `current_timestamp` - Current timestamp
    /// * `current_net_balance` - Current net balance before the proposed change
    /// * `proposed_change` - Signed amount to be added/subtracted (positive for deposits, negative for withdrawals)
    pub fn verify_limits(
        &mut self,
        current_timestamp: u32,
        current_net_balance: u64,
        proposed_change: i64,
    ) -> Result<()> {
        // Reset window if duration has elapsed
        if current_timestamp > self.window_start_timestamp + self.window_duration_seconds {
            self.window_start_timestamp = current_timestamp;
            self.window_start_net_balance = current_net_balance;
        }

        // Calculate what the new balance would be after the proposed change
        let new_balance = current_net_balance
            .checked_add_signed(proposed_change)
            .unwrap();

        // Calculate absolute and percentage changes from window start
        let start_balance = self.window_start_net_balance;
        if new_balance >= start_balance {
            // Handle positive change
            let balance_increase = new_balance - start_balance;
            let percentage_increase =
                (balance_increase as f64 / start_balance as f64 * 10000.0) as u32;

            require!(
                percentage_increase <= self.max_net_balance_change_positive_percentage,
                ExponentCoreError::NetBalanceChangeExceedsLimit
            );
        } else {
            // Handle negative change
            let balance_decrease = start_balance - new_balance;
            let percentage_decrease =
                (balance_decrease as f64 / start_balance as f64 * 10000.0) as u16;

            require!(
                percentage_decrease <= self.max_net_balance_change_negative_percentage,
                ExponentCoreError::NetBalanceChangeExceedsLimit
            );
        }

        Ok(())
    }
}

#[derive(AnchorDeserialize, AnchorSerialize, Default, Clone)]
pub struct MarketEmissions {
    pub trackers: Vec<MarketEmission>,
}

impl MarketEmissions {
    pub fn size_of(&self) -> usize {
        Self::size_of_static(self.trackers.len())
    }

    fn size_of_static(tracker_len: usize) -> usize {
        // vec len
        4 + tracker_len * MarketEmission::SIZE
    }

    pub fn get_last_seen_indices(&self) -> Vec<Number> {
        self.trackers
            .iter()
            .map(|emission| emission.lp_share_index)
            .collect()
    }

    pub fn add_emission(&mut self, token_escrow: Pubkey) {
        self.trackers.push(MarketEmission {
            token_escrow,
            lp_share_index: Number::ZERO,
            last_seen_staged: 0,
        });
    }
}

#[derive(AnchorDeserialize, AnchorSerialize, Default, Clone)]
pub struct MarketEmission {
    /// Escrow account that receives the emissions from the SY program
    /// And then passes them through to the user
    pub token_escrow: Pubkey,

    /// Index for converting LP shares into earned emissions
    pub lp_share_index: Number,

    /// The difference between the staged amount and collected emission amount
    pub last_seen_staged: u64,
}

impl MarketEmission {
    const SIZE: usize =
        // token_escrow
        32 +

        // lp_share_index
        Number::SIZEOF +

        // last_seen_staged
        8;
}

pub struct LiqAddResult {
    pub pt_in: u64,
    pub sy_in: u64,
    pub lp_out: u64,
}

pub struct LiqRmResult {
    pub pt_out: u64,
    pub sy_out: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct LpFarm {
    pub last_seen_timestamp: u32,
    pub farm_emissions: Vec<FarmEmission>,
}

impl LpFarm {
    pub fn size_of_static(farm_emissions_len: usize) -> usize {
        // last_seen_timestamp
        4 +
        // vec len
        4 + farm_emissions_len * FarmEmission::SIZE
    }

    pub fn get_last_seen_indices(&self) -> Vec<Number> {
        self.farm_emissions
            .iter()
            .map(|emission| emission.index)
            .collect()
    }

    pub fn find_farm_emission_position(&self, mint: Pubkey) -> Option<usize> {
        self.farm_emissions
            .iter()
            .position(|emission| emission.mint == mint)
    }

    /// Increase the share indexes for all the farm's emissions
    pub fn increase_share_indexes(&mut self, current_timestamp: u32, lp_staked: u64) {
        for emission in self.farm_emissions.iter_mut() {
            // calculate the time delta based on the emission's expiration
            // the global last seen timestamp
            // and the current timestamp

            // if last_seen_timestamp >= min(expiry_timestamp, current_timestamp), this function returns 0
            let time_delta = delta_time_farm(
                emission.expiry_timestamp,
                self.last_seen_timestamp,
                current_timestamp,
            );

            emission.inc_index(time_delta, lp_staked);
        }

        self.last_seen_timestamp = current_timestamp;
    }
}

/// Calculate the delta given the farm's expiration, the farm's last seen timestamp, and the current timestamp
fn delta_time_farm(expiry_timestamp: u32, last_seen_timestamp: u32, current_timestamp: u32) -> u32 {
    // treat "now" as the lesser of the expiry or the current timestamp
    // this handles the case where the farm has expired
    let now = expiry_timestamp.min(current_timestamp);

    // if last seen is greater than or equal to now, return 0
    if last_seen_timestamp >= now {
        return 0;
    }

    now - last_seen_timestamp
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct FarmEmission {
    /// Mint for the emission token
    pub mint: Pubkey,
    /// Rate at which the emission token is emitted per second
    pub token_rate: u64,
    /// Expiration timestamp for the emission token
    pub expiry_timestamp: u32,
    /// Index for converting LP shares into earned emissions
    pub index: Number,
}

impl FarmEmission {
    pub const SIZE: usize =
        // mint
        32 +
        // token_rate
        8 +
        // expiry_timestamp
        4 +
        // index
        Number::SIZEOF;

    fn inc_index(&mut self, time_delta: u32, lp_staked: u64) {
        let tokens_emitted = self.token_rate * time_delta as u64;
        let increase_amount = Number::from_ratio(tokens_emitted.into(), lp_staked.into());
        self.index += increase_amount;
    }
}