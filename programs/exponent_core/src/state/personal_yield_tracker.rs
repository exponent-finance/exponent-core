use anchor_lang::prelude::*;
use precise_number::Number;

pub type EmissionIndexes = Vec<Number>;

#[derive(AnchorDeserialize, AnchorSerialize, Clone, Default)]
pub struct PersonalYieldTrackers {
    pub trackers: Vec<PersonalYieldTracker>,
}

/// Generic tracker for interest and emissions earned by deposits
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Default)]
pub struct PersonalYieldTracker {
    /// The index is the per-share value of the SY token
    /// Note that the YT balance must be converted to the equivalent SY balance
    pub last_seen_index: Number,

    /// Staged tokens that may be withdrawn
    pub staged: u64,
}

impl PersonalYieldTrackers {
    pub fn static_size_of(tracker_len: usize) -> usize {
        // u32 vec length
        4 + tracker_len * PersonalYieldTracker::SIZE
    }

    pub fn size_of(&self) -> usize {
        PersonalYieldTrackers::static_size_of(self.trackers.len())
    }

    /// Ensure that there is 1 tracker per emission index
    fn ensure_trackers(&mut self, emission_indexes: &EmissionIndexes) {
        while self.trackers.len() < emission_indexes.len() {
            self.trackers.push(PersonalYieldTracker {
                last_seen_index: Number::ZERO,
                staged: 0,
            });
        }
    }

    /// Stage all earnings for emission trackers
    fn earn_all(&mut self, emission_indexes: &EmissionIndexes, lp_balance_user: u64) {
        for (pos, emission_index) in emission_indexes.iter().enumerate() {
            let emission_index = *emission_index;
            let tracker = &mut self.trackers[pos];
            let earned_emission = tracker.calc_earned_emissions(emission_index, lp_balance_user);

            tracker.last_seen_index = emission_index;
            tracker.staged = tracker
                .staged
                .checked_add(earned_emission)
                .expect("overflow on staging emission");
        }
    }

    /// Public method for earning all emissions & ensuring there are sufficient trackers
    pub fn ensure_trackers_and_earn_all(
        &mut self,
        emission_indexes: &EmissionIndexes,
        lp_balance_user: u64,
    ) {
        self.ensure_trackers(emission_indexes);
        self.earn_all(emission_indexes, lp_balance_user);
    }
}

impl PersonalYieldTracker {
    pub const SIZE: usize =
        // last_seen_index
        32 +
        // staged
        8;

    fn calc_earned_emissions(&self, current_index: Number, lp_amount_user: u64) -> u64 {
        let delta = current_index - self.last_seen_index;
        let earned = delta * Number::from_natural_u64(lp_amount_user);
        earned.floor_u64()
    }

    pub fn dec_staged(&mut self, amount: u64) {
        self.staged = self
            .staged
            .checked_sub(amount)
            .expect("insufficient staged balance");
    }
}
