use anchor_lang::prelude::*;

use crate::ID;

fn find_pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ID).0
}

pub fn seeds_mint_pt(vault: &Pubkey) -> [&[u8]; 2] {
    [b"mint_pt", vault.as_ref()]
}

pub fn seeds_mint_yt(vault: &Pubkey) -> [&[u8]; 2] {
    [b"mint_yt", vault.as_ref()]
}

pub fn seeds_vault_escrow_sy(vault: &Pubkey) -> [&[u8]; 2] {
    [b"escrow_sy", vault.as_ref()]
}

pub fn pda_mint_pt(vault: &Pubkey) -> Pubkey {
    find_pda(&seeds_mint_pt(vault))
}

pub fn pda_mint_yt(vault: &Pubkey) -> Pubkey {
    find_pda(&seeds_mint_yt(vault))
}

pub fn pda_vault_escrow_sy(vault: &Pubkey) -> Pubkey {
    find_pda(&seeds_vault_escrow_sy(vault))
}
