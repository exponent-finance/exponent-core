use crate::{utils::math::calc_share_value, Vault};
use anchor_lang::prelude::*;
use precise_number::Number;

use crate::utils::py_to_sy;

#[derive(Debug, Default)]
#[account]
pub struct YieldTokenPosition {
    /// Link to address that owns this position
    pub owner: Pubkey,

    /// Link to vault that manages the YT
    pub vault: Pubkey,

    /// Track the YT balance of the user here
    pub yt_balance: u64,

    /// Tracker for interest earned (paid in SY)
    pub interest: YieldTokenTracker,

    /// Tracker for emissions earned (paid in emission tokens)
    pub emissions: Vec<YieldTokenTracker>,
}

/// Generic tracker for interest and emissions earned by YT deposits
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Copy, Default, Debug)]
pub struct YieldTokenTracker {
    /// The index is the per-share value of the SY token
    /// Note that the YT balance must be converted to the equivalent SY balance
    pub last_seen_index: Number,

    /// Staged tokens that may be withdrawn
    pub staged: u64,
}

impl YieldTokenPosition {
    /// Default length includes 4 emission YieldTrackers
    pub fn size_of(num_emissions: usize) -> usize {
        // discriminator
        8 +

        // owner
        32 +

        // vault
        32 +

        // yt_balance
        8 +

        // interest
        YieldTokenTracker::size_of() +

        // emissions vec length
        4 + (num_emissions * YieldTokenTracker::size_of())
    }

    pub fn inc_yt_balance(&mut self, amount: u64) {
        self.yt_balance = self.yt_balance.checked_add(amount).unwrap();
    }

    pub fn dec_yt_balance(&mut self, amount: u64) {
        self.yt_balance = self.yt_balance.checked_sub(amount).unwrap();
    }

    /// Calculate and stage SY interest earned by this YT position.
    fn earn_sy_interest(&mut self, vault: &mut Vault, now: u32, py_index: Number) -> u64 {
        let sy_earned_at_py_index = self.calc_earned_sy(py_index);
        // Legacy vaults may have credited treasury up to a post-expiry ATH.
        let settlement_rate = if vault.is_expired(now) {
            vault.py_index().max(vault.last_seen_sy_exchange_rate)
        } else {
            vault.last_seen_sy_exchange_rate
        };
        let sy_earned = scale_sy_to_current_rate(sy_earned_at_py_index, py_index, settlement_rate);

        // stage the earned SY
        self.interest.inc_staged(sy_earned);

        // update the vault's uncollected SY
        vault.inc_uncollected_sy(sy_earned);
        vault.realize_post_expiry_yt_surplus(now, sy_earned_at_py_index, sy_earned);
        vault.set_sy_for_pt();

        // Preserve the denominator that funded the final post-maturity interval.
        if self.interest.last_seen_index < py_index {
            self.interest.last_seen_index = if vault.is_expired(now) {
                py_index.max(settlement_rate)
            } else {
                py_index
            };
        }

        sy_earned_at_py_index
    }

    /// Stage earned SY and rewards
    fn earn_all(&mut self, vault: &mut Vault, now: u32) {
        let py_index = vault.py_index_for_yt(now);
        let staged_interest_before = self.interest.staged;

        let newly_earned_interest_at_py_index = self.earn_sy_interest(vault, now, py_index);
        // Claimable SY is scaled to the live rate, but emissions for a capped
        // index interval use the same capped PY index as YT interest.
        let staged_interest_for_emissions = staged_interest_before
            .checked_add(newly_earned_interest_at_py_index)
            .unwrap();
        let sy_balance = self.total_sy_balance(py_index, staged_interest_for_emissions);

        self.earn_emissions(vault, sy_balance);
    }

    /// Calculate total SY balance for emission calculations.
    fn total_sy_balance(&self, py_index: Number, staged_interest_for_emissions: u64) -> u64 {
        staged_interest_for_emissions
            .checked_add(py_to_sy(py_index, self.yt_balance))
            .unwrap()
    }

    fn earn_emissions(&mut self, vault: &Vault, sy_balance: u64) {

        for (index, emission) in vault.emissions.iter().enumerate() {
            let e = &mut self.emissions[index];
            let earned_emission =
                calc_share_value(e.last_seen_index, emission.final_index, sy_balance);

            e.inc_staged(earned_emission);
            e.last_seen_index = emission.final_index;
        }
    }

    /// Ensure that there is 1 tracker per emission index
    fn ensure_trackers(&mut self, vault: &Vault) {
        for (index, _emission) in vault.emissions.iter().enumerate() {
            match self.emissions.get_mut(index) {
                Some(_) => {}
                None => {
                    self.emissions.push(YieldTokenTracker::new(Number::ZERO, 0));
                }
            }
        }
    }

    /// Main public function for updating the staged earnings for the position
    pub fn earn_all_with_tracking(&mut self, vault: &mut Vault, now: u32) {
        self.ensure_trackers(vault);

        self.earn_all(vault, now)
    }

    fn calc_earned_sy(&self, py_index: Number) -> u64 {
        calc_earned_sy(self.yt_balance, self.interest.last_seen_index, py_index)
    }
}

/// Scale earned SY to current rate when post-maturity appreciation exceeds pyIndex.
pub fn scale_sy_to_current_rate(sy_amount: u64, py_index: Number, current_rate: Number) -> u64 {
    if current_rate <= py_index {
        return sy_amount;
    }

    Number::floor_u64_mul_div(
        sy_amount.into(),
        py_index.raw_u128(),
        current_rate.raw_u128(),
    )
}

impl YieldTokenTracker {
    fn size_of() -> usize {
        // last_seen_index
        Number::SIZEOF +

        // staged
        8
    }

    /// Settle the staged interest
    pub fn collect(&mut self, amount: u64) {
        self.staged = self.staged.checked_sub(amount).unwrap();
    }

    pub fn new(last_seen_index: Number, staged: u64) -> Self {
        Self {
            last_seen_index,
            staged,
        }
    }

    pub fn inc_staged(&mut self, amount: u64) {
        self.staged = self.staged.checked_add(amount).unwrap();
    }

    pub fn dec_staged(&mut self, amount: u64) {
        self.staged = self.staged.checked_sub(amount).unwrap();
    }
}

pub struct EarnSyInterestResult {
    /// How much SY is "surplus" (earned from after the vault matures)
    pub earned_sy_surplus: u64,
    /// How much SY does the user earn
    pub earned_sy_user: u64,
}

/// Calculate SY earned from an increase in pyIndex.
pub fn calc_earned_sy(
    yt_balance: u64,
    last_seen_py_index: Number,
    current_py_index: Number,
) -> u64 {
    if last_seen_py_index >= current_py_index
        || yt_balance == 0
        || last_seen_py_index == Number::ZERO
    {
        return 0;
    }

    let last_seen_raw = last_seen_py_index.raw_u128();
    let current_raw = current_py_index.raw_u128();
    let raw_delta = current_raw - last_seen_raw;

    Number::floor_u64_mul_div_3_by_2(
        yt_balance.into(),
        Number::DENOM,
        raw_delta,
        last_seen_raw,
        current_raw,
    )
}
