use sy_common::SyState;

use crate::state::{Vault, YieldTokenPosition};

/// Apply earnings for a yield position
pub fn yield_position_earn(vault: &mut Vault, yield_position: &mut YieldTokenPosition) {
    yield_position.earn_all_with_tracking(vault);
}

/// Update the vault's SY rate and the emission tracker indexes
fn update_indexes(vault: &mut Vault, sy_state: &SyState, now: u32) {
    vault.update_from_sy_state(sy_state, now);
}

pub fn update_vault_yield(
    vault: &mut Vault,
    vault_yield_position: &mut YieldTokenPosition,
    now: u32,
    sy_state: &SyState,
) {
    // First, set the latest indexes from the SY state
    update_indexes(vault, sy_state, now);

    // then stage an earnings with the vault's YT position
    yield_position_earn(vault, vault_yield_position);
}