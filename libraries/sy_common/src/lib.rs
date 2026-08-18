use anchor_lang::prelude::*;
use precise_number::Number;

#[derive(AnchorDeserialize, AnchorSerialize, Clone, Default)]
pub struct SyState {
    pub exchange_rate: Number,
    pub emission_indexes: Vec<Number>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PositionState {
    pub owner: Pubkey,
    pub sy_balance: u64,
    pub emissions: Vec<Emission>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct Emission {
    pub mint: Pubkey,
    pub amount_claimable: u64,
    pub last_seen_emission_index: Number,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MintSyReturnData {
    pub sy_out_amount: u64,
    pub exchange_rate: Number,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RedeemSyReturnData {
    pub base_out_amount: u64,
    pub exchange_rate: Number,
}
