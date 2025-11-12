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

    fn earn_sy_interest(&mut self, vault: &mut Vault) {
        // If vault is in emergency mode, do nothing
        if vault.is_in_emergency_mode() {
            return;
        }

        // use the final_sy_exchange_rate on the vault, since this is the last snapshot
        // final_sy_exchange_rate gets updated while the vault is active - but stops getting updated once the vault has matured
        let rate = vault.final_sy_exchange_rate;

        // calculate the earned SY
        // according to the final rate of the vault
        let sy_earned = self.calc_earned_sy(rate);

        // if the current rate is higher than the final rate, scale down the earned SY
        let sy_earned = scale_sy_to_current_rate(sy_earned, rate, vault.last_seen_sy_exchange_rate);

        // stage the earned SY
        self.interest.inc_staged(sy_earned);

        // update the vault's uncollected SY
        vault.inc_uncollected_sy(sy_earned);

        // update the last seen index
        self.interest.last_seen_index = rate;
    }

    /// Stage earned SY and rewards
    fn earn_all(&mut self, vault: &mut Vault) {
        self.earn_sy_interest(vault);

        self.earn_emissions(vault);
    }

    fn total_sy_balance(&self, vault: &Vault) -> u64 {
        self.interest.staged + py_to_sy(vault.final_sy_exchange_rate, self.yt_balance)
    }

    fn earn_emissions(&mut self, vault: &Vault) {
        // TODO - consider negative rates
        let sy_balance = self.total_sy_balance(vault);

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
    pub fn earn_all_with_tracking(&mut self, vault: &mut Vault) {
        self.ensure_trackers(vault);

        self.earn_all(vault)
    }

    /// Calculate earned interest based on the "final" exchange rate
    /// This "final" rate gets updated while the vault is active - but stops getting updated once the vault has matured
    fn calc_earned_sy(&self, final_rate: Number) -> u64 {
        calc_earned_sy(self.yt_balance, self.interest.last_seen_index, final_rate)
    }
}

/// If the current SY rate is higher than the final rate used to compute the YT's SY earnings
/// For example:
/// A user has 200 YT when SY was 2.0
/// The final rate is 3.0
/// The current rate is 4.0
/// The user has earned $100 of interest
/// The normal calculation would use the final rate, to conclude that the user has earned 33 SY
/// But, at the current rate of 4.0, the user should receive 25 SY
/// So we scale the earned SY down by the current rate.  33 * (3.0 / 4.0) = 25
fn scale_sy_to_current_rate(sy_amount: u64, final_rate: Number, current_rate: Number) -> u64 {
    if current_rate <= final_rate {
        return sy_amount;
    }

    let scale = final_rate / current_rate;

    (scale * sy_amount.into()).floor_u64()
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

/// Calculate the amount of SY earned from an appreciation in the exchange rate
fn calc_earned_sy(
    yt_balance: u64,
    sy_exchange_rate_last_seen: Number,
    sy_exchange_rate_cur: Number,
) -> u64 {
    // If the last seen exchange rate is equal (or higher?!) than the final rate, we're done
    // or if there is 0 yt balance
    // or if the last seen exchange rate is 0 (which should not happen if YT balance is not 0 anyway)
    if sy_exchange_rate_last_seen >= sy_exchange_rate_cur
        || yt_balance == 0
        || sy_exchange_rate_last_seen == Number::ZERO
    {
        return 0;
    }

    let delta = Number::ONE / sy_exchange_rate_last_seen - Number::ONE / sy_exchange_rate_cur;

    // number of "SY shares" earned from the pool
    let sy_earned = delta * yt_balance.into();

    sy_earned.floor_u64()
}
