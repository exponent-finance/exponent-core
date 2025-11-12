use precise_number::Number;

/// Calculate earned emissions based on the final index
pub fn calc_share_value(last_seen_index: Number, cur_index: Number, share_balance: u64) -> u64 {
    if cur_index <= last_seen_index {
        return 0;
    }

    let delta = cur_index - last_seen_index;
    let earned = delta * share_balance.into();

    earned.floor_u64()
}
