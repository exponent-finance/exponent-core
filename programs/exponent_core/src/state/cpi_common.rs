use anchor_lang::prelude::*;

/// Account lists for validating CPI calls to the SY program
#[derive(AnchorDeserialize, AnchorSerialize, Default, Clone, Debug)]
pub struct CpiAccounts {
    /// Fetch SY state
    pub get_sy_state: Vec<CpiInterfaceContext>,

    /// Deposit SY into personal account owned by vault
    pub deposit_sy: Vec<CpiInterfaceContext>,

    /// Withdraw SY from personal account owned by vault
    pub withdraw_sy: Vec<CpiInterfaceContext>,

    /// Settle rewards for vault to accounts owned by the vault
    pub claim_emission: Vec<Vec<CpiInterfaceContext>>,

    /// Get personal yield position
    pub get_position_state: Vec<CpiInterfaceContext>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CpiInterfaceContext {
    /// Address-lookup-table index
    pub alt_index: u8,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl CpiAccounts {
    /// Delegate to borsh to get length in bytes
    pub fn size_of(&self) -> usize {
        self.try_to_vec().unwrap().len()
    }
}

/// Static function for converting a list of CpiInterfaceContexts to a list of AccountMetas
pub fn to_account_metas(
    contexts: &Vec<CpiInterfaceContext>,
    lookup_table: &[Pubkey],
) -> Vec<AccountMeta> {
    let find_key = |alt_index: u8| *lookup_table.get(alt_index as usize).unwrap();

    contexts
        .iter()
        .map(|ctx| AccountMeta {
            pubkey: find_key(ctx.alt_index),
            is_signer: ctx.is_signer,
            is_writable: ctx.is_writable,
        })
        .collect()
}