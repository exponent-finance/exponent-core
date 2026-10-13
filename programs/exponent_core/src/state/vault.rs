use crate::{
    error::ExponentCoreError, seeds::AUTHORITY_SEED, utils::math::calc_share_value, CpiAccounts,
};
use anchor_lang::prelude::*;
use precise_number::Number;
use sy_common::SyState;

pub const STATUS_CAN_STRIP: u8 = 0b0000_0001;
pub const STATUS_CAN_MERGE: u8 = 0b0000_0010;
pub const STATUS_CAN_DEPOSIT_YT: u8 = 0b0000_0100;
pub const STATUS_CAN_WITHDRAW_YT: u8 = 0b0000_1000;
pub const STATUS_CAN_COLLECT_INTEREST: u8 = 0b0001_0000;
pub const STATUS_CAN_COLLECT_EMISSIONS: u8 = 0b0010_0000;

#[derive(Default, Debug)]
#[account]
pub struct Vault {
    /// Link to SY program
    pub sy_program: Pubkey,

    /// Mint for SY token
    pub mint_sy: Pubkey,

    /// Mint for the vault-specific YT token
    pub mint_yt: Pubkey,

    /// Mint for the vault-specific PT token
    pub mint_pt: Pubkey,

    /// Escrow account for holding deposited YT
    pub escrow_yt: Pubkey,

    /// Escrow account that holds temporary SY tokens
    /// As an interchange between users and the SY program
    pub escrow_sy: Pubkey,

    /// Link to a vault-owned YT position
    /// This account collects yield from all "unstaked" YT
    pub yield_position: Pubkey,

    /// Address lookup table key for vault
    pub address_lookup_table: Pubkey,

    /// start timestamp
    pub start_ts: u32,

    /// seconds duration
    pub duration: u32,

    /// Seed for CPI signing
    pub signer_seed: Pubkey,

    /// Authority for CPI signing
    pub authority: Pubkey,

    /// bump for signer authority PDA
    pub signer_bump: [u8; 1],

    /// Last seen SY exchange rate
    /// This continues to be updated after maturity for PT redemption
    pub last_seen_sy_exchange_rate: Number,

    /// This is the all time high exchange rate for SY while the vault is not expired
    pub all_time_high_sy_exchange_rate: Number,

    /// Final PY index captured for YT interest at maturity.
    /// This is the highest observed SY exchange rate while the vault is active,
    /// not necessarily the live exchange rate at the last pre-expiry update.
    pub final_sy_exchange_rate: Number,

    /// How much SY is held in escrow
    pub total_sy_in_escrow: u64,

    /// The total SY set aside to back the PT holders
    /// This value is updated on every operation that touches the PT supply or the last seen exchange rate
    pub sy_for_pt: u64,

    /// Total supply of PT
    pub pt_supply: u64,

    /// Amount of SY staged for the treasury
    pub treasury_sy: u64,

    /// SY that has been earned by YT, but not yet collected
    pub uncollected_sy: u64,

    pub treasury_sy_token_account: Pubkey,

    pub interest_bps_fee: u16,

    pub min_op_size_strip: u64,

    pub min_op_size_merge: u64,

    pub status: u8,

    pub emissions: Vec<EmissionInfo>,

    pub cpi_accounts: CpiAccounts,

    pub claim_limits: ClaimLimits,

    pub max_py_supply: u64,
}

impl Vault {
    pub fn is_expired(&self, current_ts: u32) -> bool {
        current_ts > self.start_ts + self.duration
    }







    pub fn is_active(&self, current_ts: u32) -> bool {
        current_ts >= self.start_ts && !self.is_expired(current_ts)
    }

    /// Current pyIndex watermark used for PT conversions.
    pub fn py_index(&self) -> Number {
        self.all_time_high_sy_exchange_rate
    }

    /// Watermark capped at maturity for YT interest.
    pub fn py_index_for_yt(&self, current_ts: u32) -> Number {
        if self.is_active(current_ts) {
            self.all_time_high_sy_exchange_rate
        } else {
            self.final_sy_exchange_rate
        }
    }

    /// True when the live rate is below the watermark.
    pub fn is_below_watermark(&self) -> bool {
        self.all_time_high_sy_exchange_rate > self.last_seen_sy_exchange_rate
    }

    #[deprecated(
        note = "Use is_below_watermark() - operations should not be blocked based on this"
    )]
    pub fn is_in_emergency_mode(&self) -> bool {
        self.is_below_watermark()
    }

    /// Can stage interest if the vault is active.
    pub fn can_stage_sy_interest(&self, current_ts: u32) -> bool {
        self.is_active(current_ts)
    }

    /// Seeds for deriving signer PDA
    pub fn signer_seeds(&self) -> [&[u8]; 3] {
        [AUTHORITY_SEED, self.signer_seed.as_ref(), &self.signer_bump]
    }

    /// Calculate borsh-encoded size of the struct
    pub fn size_of(&self) -> usize {
        self.try_to_vec().unwrap().len()
    }

    /// The redemption rate for PT holders into SY tokens is their share value of the pool
    pub fn pt_redemption_rate(&self) -> Number {
        Number::from_ratio(self.sy_for_pt.into(), self.pt_supply.into())
    }

    pub fn collect_treasury_interest(&mut self, amount: u64) {
        self.treasury_sy = self.treasury_sy.checked_sub(amount).unwrap();
    }

    pub fn can_collect_treasury_sy(&self) -> bool {
        self.treasury_sy > 0
    }

    /// Static size of the struct in borsh, ignoring the dynamic CpiAccounts field
    pub fn size_of_static(emissions_length: usize) -> usize {
        // discriminator
        8 +

        // sy_program
        32 +

        // mint_sy
        32 +

        // mint_yt
        32 +

        // mint_pt
        32 +

        // escrow_yt
        32 +

        // escrow_sy
        32 +

        // yield_position
        32 +

        // address_lookup_table
        32 +

        // start_ts
        4 +

        // duration
        4 +

        // signer_seed
        32 +

        // authority
        32 +

        // signer_bump
        1 +

        // last_seen_sy_exchange_rate
        Number::SIZEOF +

        // all_time_high_sy_exchange_rate
        Number::SIZEOF +

        // final_sy_exchange_rate
        Number::SIZEOF +

        // total_sy_in_escrow
        8 +

        // sy_for_pt
        8 +

        // pt_supply
        8 +

        // treasury_sy
        8 +

        // uncollected_sy
        8 +

        // treasury_sy_token_account
        32 +

        // interest_bps_fee
        2 +

        // min_op_size_strip
        8 +

        // min_op_size_merge
        8 +

        // status
        1 +

        // emissions vec length prefix
        4 +

        // max_py_supply
        8 +

        // emissions vec
        emissions_length * EmissionInfo::size_of() +

        // claim_limits
        ClaimLimits::size_of()
    }

    pub fn check_status_flags(&self, required_flags: u8) -> bool {
        self.status & required_flags == required_flags
    }

    /// Calculate the fee on interest
    pub fn interest_fee(&self, amount_sy: u64) -> u64 {
        let fee =
            Number::from_natural_u64(amount_sy) * Number::from_bps(self.interest_bps_fee.into());
        fee.floor_u64()
    }

    /// Can only collect emissions for lambo if the vault is expired
    /// Since emissions are non-decreasing, that is the only constraint
    fn can_collect_emission_lambo(&self, now: u32) -> bool {
        self.is_expired(now)
    }

    /// Calculate the surplus emissions to store in the lambo fund
    fn calc_emission_surpluses(&self, cur_sy_state: &SyState) -> Vec<u64> {
        self.emissions
            .iter()
            .zip(cur_sy_state.emission_indexes.iter())
            .map(|(emission, index)| {
                calc_share_value(emission.last_seen_index, *index, self.total_sy_in_escrow)
            })
            .collect()
    }

    /// Increase the lambo fund for emissions
    fn increase_emission_lambo_fund(&mut self, surpluses: Vec<u64>) {
        for (i, s) in surpluses.iter().enumerate() {
            self.emissions[i].treasury_emission =
                self.emissions[i].treasury_emission.checked_add(*s).unwrap()
        }
    }

    pub fn is_min_op_size_strip(&self, amount: u64) -> bool {
        amount >= self.min_op_size_strip
    }

    pub fn is_min_op_size_merge(&self, amount: u64) -> bool {
        amount >= self.min_op_size_merge
    }

    /// Calculate the amount of SY that is not set aside in the treasury nor set aside in the uncollected_sy for YT holders
    fn active_sy(&self) -> u64 {
        self.total_sy_in_escrow
            .checked_sub(self.treasury_sy)
            .and_then(|x| x.checked_sub(self.uncollected_sy))
            .unwrap_or(0)
    }

    /// Calculate the amount of SY backing set aside for PT holders.
    fn sy_backing_pt(&self) -> u64 {
        let pt_redemption_py_index = self.py_index().max(self.last_seen_sy_exchange_rate);
        sy_backing_for_pt(pt_redemption_py_index, self.pt_supply, self.active_sy())
    }

    pub fn set_sy_for_pt(&mut self) {
        self.sy_for_pt = self.sy_backing_pt();
    }

    fn post_expiry_surplus_baseline(&self) -> Number {
        // Deployed vaults could raise ATH after expiry and credit treasury at that rate.
        self.final_sy_exchange_rate
            .max(self.all_time_high_sy_exchange_rate)
    }

    fn sy_for_py_at_rate(&self, amount_py: u64, rate: Number) -> u64 {
        if rate == Number::ZERO {
            return 0;
        }

        (Number::from_natural_u64(amount_py) / rate).floor_u64()
    }

    fn can_realize_post_expiry_sy_surplus(&self, now: u32) -> bool {
        self.is_expired(now)
            && self.final_sy_exchange_rate != Number::ZERO
            && self.last_seen_sy_exchange_rate > self.post_expiry_surplus_baseline()
    }

    /// Only settle surplus that can be removed while preserving baseline backing for remaining PT.
    fn collectable_post_expiry_sy_surplus(&self, surplus_sy: u64) -> u64 {
        let sy_required_for_pt =
            self.sy_for_py_at_rate(self.pt_supply, self.post_expiry_surplus_baseline());
        let collectable_sy = self.active_sy().saturating_sub(sy_required_for_pt);

        surplus_sy.min(collectable_sy)
    }

    pub fn realize_post_expiry_pt_surplus(&mut self, now: u32, amount_py: u64, amount_sy: u64) {
        if !self.can_realize_post_expiry_sy_surplus(now) {
            return;
        }

        let sy_at_baseline = self.sy_for_py_at_rate(amount_py, self.post_expiry_surplus_baseline());
        let sy_at_live_rate = self.sy_for_py_at_rate(amount_py, self.last_seen_sy_exchange_rate);
        // A payout capped by insufficient backing is not newly earned treasury yield.
        let surplus_sy = sy_at_baseline.saturating_sub(sy_at_live_rate.max(amount_sy));
        let surplus_sy = self.collectable_post_expiry_sy_surplus(surplus_sy);
        self.inc_treasury_sy(surplus_sy);
    }

    pub fn realize_post_expiry_yt_surplus(
        &mut self,
        now: u32,
        sy_at_final_rate: u64,
        amount_sy: u64,
    ) {
        if !self.can_realize_post_expiry_sy_surplus(now) {
            return;
        }

        let baseline = self.post_expiry_surplus_baseline();
        // The final-to-ATH portion may have been credited by the deployed program.
        let sy_at_baseline = Number::floor_u64_mul_div(
            sy_at_final_rate.into(),
            self.final_sy_exchange_rate.raw_u128(),
            baseline.raw_u128(),
        );
        let surplus_sy = sy_at_baseline.saturating_sub(amount_sy);
        let surplus_sy = self.collectable_post_expiry_sy_surplus(surplus_sy);
        self.inc_treasury_sy(surplus_sy);
    }

    /// Public function to update the vault from a new SY state
    /// If the vault is expired, then update emission lambo fund from total SY held in escrow
    /// Update the last seen exchange rate, and all time high exchange rate
    /// Update the emission indexes and final indexes
    /// And then update the amount of SY backing for PT holders
    pub fn update_from_sy_state(&mut self, sy_state: &SyState, now: u32) {
        // check if vault will collect treasury emissions from post-maturity emissions
        if self.can_collect_emission_lambo(now) {
            let surplus_emissions = self.calc_emission_surpluses(&sy_state);
            self.increase_emission_lambo_fund(surplus_emissions);
        }

        let cur_rate = sy_state.exchange_rate;
        self.last_seen_sy_exchange_rate = cur_rate;

        if !self.is_expired(now) {
            let py_index = cur_rate.max(self.all_time_high_sy_exchange_rate);
            self.all_time_high_sy_exchange_rate = py_index;

            // If the vault is active, keep the final PY index aligned with the
            // monotonic watermark. Once expired, it remains sealed.
            if self.is_active(now) {
                self.final_sy_exchange_rate = py_index;
            }
        }

        for (index, x) in sy_state.emission_indexes.iter().enumerate() {
            self.emissions[index].last_seen_index = *x;

            // if the vault is active, then update the final index
            if self.is_active(now) {
                self.emissions[index].final_index = *x;
            }
        }

        // After changing the rate, we need to update the amount of SY backing for PT holders
        self.set_sy_for_pt();
    }

    pub fn add_emission(
        &mut self,
        token_account: Pubkey,
        sy_state: &SyState,
        treasury_token_account: Pubkey,
        fee_bps: u16,
    ) {
        // emissions are assumed to be added 1 at a time, and so the SY state should have the same number of indexes
        let last_seen_index = sy_state.emission_indexes[self.emissions.len()];
        self.emissions.push(EmissionInfo::new(
            token_account,
            treasury_token_account,
            last_seen_index,
            fee_bps,
        ));
    }

    pub fn inc_total_sy_in_escrow(&mut self, amount: u64) {
        self.total_sy_in_escrow = self
            .total_sy_in_escrow
            .checked_add(amount)
            .expect("overflow adding SY to escrow for vault");
    }

    pub fn dec_total_sy_in_escrow(&mut self, amount: u64) {
        self.total_sy_in_escrow = self
            .total_sy_in_escrow
            .checked_sub(amount)
            .expect("underflow subtracting SY from escrow for vault");
    }

    pub fn inc_pt_supply(&mut self, amount: u64) {
        self.pt_supply = self
            .pt_supply
            .checked_add(amount)
            .expect("overflow adding PT supply for vault");

        assert!(self.pt_supply <= self.max_py_supply);
    }

    pub fn dec_pt_supply(&mut self, amount: u64) {
        self.pt_supply = self
            .pt_supply
            .checked_sub(amount)
            .expect("underflow subtracting PT supply for vault");
    }

    pub fn inc_treasury_sy(&mut self, amount: u64) {
        self.treasury_sy = self
            .treasury_sy
            .checked_add(amount)
            .expect("overflow adding treasury SY for vault");
    }

    pub fn dec_treasury_sy(&mut self, amount: u64) {
        self.treasury_sy = self
            .treasury_sy
            .checked_sub(amount)
            .expect("underflow subtracting treasury SY for vault");
    }

    pub fn inc_uncollected_sy(&mut self, amount: u64) {
        self.uncollected_sy = self
            .uncollected_sy
            .checked_add(amount)
            .expect("overflow adding uncollected SY for vault");
    }

    pub fn dec_uncollected_sy(&mut self, amount: u64) {
        self.uncollected_sy = self
            .uncollected_sy
            .checked_sub(amount)
            .expect("underflow subtracting uncollected SY for vault");
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, Default, Debug)]
pub struct ClaimLimits {
    pub claim_window_start_timestamp: u32,
    pub total_claim_amount_in_window: u64,
    pub max_claim_amount_per_window: u64,
    pub claim_window_duration_seconds: u32,
}

impl ClaimLimits {
    pub fn verify_claim_limits(&mut self, amount: u64, current_timestamp: u32) -> Result<()> {
        if current_timestamp
            > self.claim_window_start_timestamp + self.claim_window_duration_seconds
        {
            self.claim_window_start_timestamp = current_timestamp;
            self.total_claim_amount_in_window = 0;
        }

        require!(
            self.total_claim_amount_in_window + amount <= self.max_claim_amount_per_window,
            ExponentCoreError::ClaimLimitExceeded
        );

        self.total_claim_amount_in_window = self
            .total_claim_amount_in_window
            .checked_add(amount)
            .unwrap();

        Ok(())
    }

    pub fn size_of() -> usize {
        // claim_window_start_timestamp
        4 +
        // total_claim_amount_in_window
        8 +
        // max_claim_amount_per_window
        8 +
        // claim_window_duration_seconds
        4
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, Default, Debug)]
pub struct EmissionInfo {
    /// The token account for the emission where the vault authority is the authority
    pub token_account: Pubkey,
    // The initial index is used to track the first claimable index for yield positions after an emission has been added
    pub initial_index: Number,
    // The last seen index tracks the most recently observed emission index from the SY state
    pub last_seen_index: Number,
    /// The final index is used to track the last claimable index after the vault expires
    pub final_index: Number,
    /// The treasury token account for this reward
    pub treasury_token_account: Pubkey,
    /// The fee taken from emission collecting
    pub fee_bps: u16,
    /// The lambo fund
    pub treasury_emission: u64,
}

impl EmissionInfo {
    pub fn new(
        token_account: Pubkey,
        treasury_token_account: Pubkey,
        current_index: Number,
        fee_bps: u16,
    ) -> Self {
        Self {
            token_account,
            treasury_token_account,
            // set the initial index to the last seen index
            initial_index: current_index,
            // set the last seen index to the initial index
            last_seen_index: current_index,
            // set the final index to the last seen index
            final_index: current_index,
            treasury_emission: 0,
            fee_bps,
        }
    }

    pub fn calculate_reward_amount(self, sy_amount: u64, current_index: &Number) -> u64 {
        let delta = current_index.checked_sub(&self.last_seen_index).unwrap();
        let earned = delta * Number::from_natural_u64(sy_amount);
        earned.floor_u64()
    }

    pub fn size_of() -> usize {
        // token_account: Pubkey
        32 +
        // initial_index: Number
        32 +
        // last_seen_index: Number
        32 +
        // final_index: Number
        32 +
        // treasury_token_account: Pubkey
        32 +
        // fee_bps: u16
        2 +
        // treasury_emission: u64
        8
    }

    /// Settle the staged interest
    pub fn collect_treasury_emission(&mut self, amount: u64) {
        self.treasury_emission = self.treasury_emission.checked_sub(amount).unwrap();
    }
}

/// Calculate how much SY backing is set aside for PT holders.
fn sy_backing_for_pt(py_index: Number, pt_supply: u64, sy_in_escrow: u64) -> u64 {
    if py_index == Number::ZERO {
        return 0;
    }
    let sy_required = Number::from_natural_u64(pt_supply) / py_index;
    let sy_required = sy_required.floor_u64();

    sy_required.min(sy_in_escrow)
}
