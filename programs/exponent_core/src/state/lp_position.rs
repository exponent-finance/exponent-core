use super::{EmissionIndexes, MarketTwo, PersonalYieldTracker, PersonalYieldTrackers};
use anchor_lang::prelude::*;

#[account]
pub struct LpPosition {
    /// Link to address that owns this position
    pub owner: Pubkey,

    /// Link to market that manages the LP
    pub market: Pubkey,

    /// Track the LP balance of the user here
    pub lp_balance: u64,

    /// Tracker for emissions earned (paid in emission tokens)
    pub emissions: PersonalYieldTrackers,

    pub farms: PersonalYieldTrackers,
}

impl LpPosition {
    pub fn static_size_of(emission_trackers_len: usize, farm_emissions_len: usize) -> usize {
        // discriminator
        8 +
        // owner
        32 +
        // market
        32 +
        // lp_balance
        8 +
        // emissions
        PersonalYieldTrackers::static_size_of(emission_trackers_len) +
        // farms
        PersonalYieldTrackers::static_size_of(farm_emissions_len)
    }

    pub fn size_of(&self) -> usize {
        LpPosition::static_size_of(self.emissions.trackers.len(), self.farms.trackers.len())
    }

    pub fn new_from_market(m: &MarketTwo, owner: Pubkey) -> Self {
        let trackers = m
            .emissions
            .trackers
            .iter()
            .map(|e| PersonalYieldTracker {
                last_seen_index: e.lp_share_index,
                staged: 0,
            })
            .collect();

        let emissions = PersonalYieldTrackers { trackers };

        let farm_trackers = m
            .lp_farm
            .farm_emissions
            .iter()
            .map(|e| PersonalYieldTracker {
                last_seen_index: e.index,
                staged: 0,
            })
            .collect();

        let farms = PersonalYieldTrackers {
            trackers: farm_trackers,
        };

        Self {
            owner,
            market: m.self_address,
            lp_balance: 0,
            emissions,
            farms,
        }
    }

    pub fn stage_all(
        &mut self,
        emission_indexes: &EmissionIndexes,
        farm_indexes: &EmissionIndexes,
    ) {
        self.emissions
            .ensure_trackers_and_earn_all(emission_indexes, self.lp_balance);

        self.farms
            .ensure_trackers_and_earn_all(farm_indexes, self.lp_balance);
    }

    pub fn rm_lp(&mut self, amount: u64) {
        self.lp_balance = self
            .lp_balance
            .checked_sub(amount)
            .expect("insufficent LP token balance");
    }

    pub fn add_lp(&mut self, amount: u64) {
        self.lp_balance = self
            .lp_balance
            .checked_add(amount)
            .expect("lp balance overflow");
    }
}
