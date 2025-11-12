use anchor_lang::{prelude::*, solana_program::address_lookup_table::state::AddressLookupTable};
use anchor_spl::token_2022::{self, Transfer};

pub fn deserialize_lookup_table(account: &AccountInfo) -> Vec<Pubkey> {
    AddressLookupTable::deserialize(&account.data.borrow())
        .unwrap()
        .addresses
        .to_vec()
}

/// Get around the linter for deprecated code
pub fn token_transfer<'i>(
    ctx: CpiContext<'_, '_, '_, 'i, Transfer<'i>>,
    amount: u64,
) -> Result<()> {
    #[allow(deprecated)]
    token_2022::transfer(ctx, amount)
}

/// TODO - we should convert to u64
pub fn now() -> u32 {
    Clock::get().unwrap().unix_timestamp as u32
}
