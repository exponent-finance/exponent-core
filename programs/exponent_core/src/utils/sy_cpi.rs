use crate::{
    cpi_common::{to_account_metas, CpiAccounts, CpiInterfaceContext},
    util::deserialize_lookup_table,
};
use amount_value::Amount;
use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::Instruction,
        program::{get_return_data, invoke, invoke_signed},
    },
};
use precise_number::Number;
use std::{collections::HashSet, vec};
use sy_common::{MintSyReturnData, PositionState, RedeemSyReturnData, SyState};

/// Filter rem_accounts to only include those that are in the CpiInterfaceContexts
pub fn filter_rem_accounts<'i>(
    rem_accounts: &[AccountInfo<'i>],
    contexts: &Vec<CpiInterfaceContext>,
    lookup_table: &[Pubkey],
) -> Vec<AccountInfo<'i>> {
    let needed_pubkeys: HashSet<Pubkey> = make_needed_pubkeys(contexts, lookup_table);

    rem_accounts
        .iter()
        .filter(|account| needed_pubkeys.contains(&account.key))
        .cloned()
        .collect()
}

/// Make the pubkeys that are needed for the SY program CPI
pub fn make_needed_pubkeys(
    contexts: &Vec<CpiInterfaceContext>,
    lookup_table: &[Pubkey],
) -> HashSet<Pubkey> {
    let find_pubkey = |alt_index: u8| *lookup_table.get(alt_index as usize).unwrap();

    contexts
        .iter()
        .map(|context| find_pubkey(context.alt_index))
        .collect()
}

/// CPI interface for initializing a personal account with an SY program
pub fn cpi_init_sy_personal_account<'i>(
    sy_program: Pubkey,
    rem_accounts: &[AccountInfo<'i>],
) -> Result<()> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [3];
    data.extend_from_slice(discriminator.as_slice());

    let account_metas = to_metas(rem_accounts);

    invoke(
        &Instruction {
            program_id: sy_program,
            accounts: account_metas,
            data,
        },
        rem_accounts,
    )?;

    Ok(())
}

/// Get SY state from SY program
pub fn cpi_get_sy_state<'i>(
    sy_program: Pubkey,
    account_infos: &[AccountInfo<'i>],
    account_metas: Vec<AccountMeta>,
) -> Result<SyState> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [7];
    data.extend_from_slice(discriminator.as_slice());

    invoke(
        &Instruction {
            program_id: sy_program,
            accounts: account_metas,
            data,
        },
        &account_infos,
    )?;

    let d = get_return_data().unwrap().1;

    let sy_state = SyState::try_from_slice(&d)?;

    Ok(sy_state)
}

/// Deposit SY tokens into SY program
pub fn cpi_deposit_sy<'i>(
    sy_program: Pubkey,
    amount: u64,
    account_infos: &[AccountInfo<'i>],
    account_metas: Vec<AccountMeta>,
    seeds: &[&[&[u8]]],
) -> Result<SyState> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [5];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(amount.to_le_bytes());

    invoke_signed(
        &Instruction {
            program_id: sy_program,
            accounts: account_metas,
            data,
        },
        &account_infos,
        seeds,
    )?;

    let d = get_return_data().unwrap().1;

    let sy_state = SyState::try_from_slice(&d)?;

    Ok(sy_state)
}

/// Withdraw SY tokens from the SY program
pub fn cpi_withdraw_sy<'i>(
    sy_program: Pubkey,
    amount: u64,
    account_infos: &[AccountInfo<'i>],
    account_metas: Vec<AccountMeta>,
    seeds: &[&[&[u8]]],
) -> Result<SyState> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [6];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(amount.to_le_bytes());

    invoke_signed(
        &Instruction {
            program_id: sy_program,
            accounts: account_metas,
            data,
        },
        &account_infos,
        seeds,
    )?;

    let d = get_return_data().unwrap().1;

    let sy_state = SyState::try_from_slice(&d)?;

    Ok(sy_state)
}

pub fn cpi_claim_emission<'i>(
    sy_program: Pubkey,
    amount: u64,
    account_infos: &[AccountInfo<'i>],
    account_metas: Vec<AccountMeta>,
    seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [8];
    data.extend_from_slice(discriminator.as_slice());
    Amount::Some(amount).serialize(&mut data)?;

    invoke_signed(
        &Instruction {
            program_id: sy_program,
            accounts: account_metas,
            data,
        },
        &account_infos,
        seeds,
    )?;

    Ok(())
}

pub fn cpi_get_position<'i>(
    sy_program: Pubkey,
    account_infos: &[AccountInfo<'i>],
    account_metas: Vec<AccountMeta>,
    seeds: &[&[&[u8]]],
) -> Result<PositionState> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [10];
    data.extend_from_slice(discriminator.as_slice());

    invoke_signed(
        &Instruction {
            program_id: sy_program,
            accounts: account_metas,
            data,
        },
        &account_infos,
        seeds,
    )?;

    let d = get_return_data().unwrap().1;

    let position = PositionState::try_from_slice(&d)?;

    Ok(position)
}

pub fn cpi_mint_sy<'i>(
    sy_program: Pubkey,
    amount_base: u64,
    account_infos: &[AccountInfo<'i>],
    account_metas: Vec<AccountMeta>,
) -> Result<MintSyReturnData> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [1];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(amount_base.to_le_bytes());

    invoke(
        &Instruction {
            program_id: sy_program,
            accounts: account_metas,
            data,
        },
        &account_infos,
    )?;

    let d = get_return_data().unwrap().1;

    let mint_sy_return_data = MintSyReturnData::try_from_slice(&d)?;

    Ok(mint_sy_return_data)
}

pub fn cpi_redeem_sy<'i>(
    sy_program: Pubkey,
    amount_sy: u64,
    account_infos: &[AccountInfo<'i>],
    account_metas: Vec<AccountMeta>,
) -> Result<RedeemSyReturnData> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [2];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(amount_sy.to_le_bytes());

    invoke(
        &Instruction {
            program_id: sy_program,
            accounts: account_metas,
            data,
        },
        &account_infos,
    )?;

    let d = get_return_data().unwrap().1;

    let redeem_sy_return_data = RedeemSyReturnData::try_from_slice(&d)?;

    Ok(redeem_sy_return_data)
}

/// Convert an AccountInfo into AccountMeta
pub fn to_meta(account: &AccountInfo) -> AccountMeta {
    AccountMeta {
        pubkey: *account.key,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
    }
}

pub fn to_metas(account_infos: &[AccountInfo]) -> Vec<AccountMeta> {
    account_infos
        .iter()
        .map(to_meta)
        .collect::<Vec<AccountMeta>>()
}

/// Convert PT & YT to SY
pub fn py_to_sy(sy_exchange_rate: Number, amount_py: u64) -> u64 {
    let sy = Number::from_natural_u64(amount_py) / sy_exchange_rate;
    sy.floor_u64()
}

/// Convert PT & YT to SY using ceiling division
pub fn py_to_sy_ceil(sy_exchange_rate: Number, amount_py: u64) -> u64 {
    let py_num = Number::from_natural_u64(amount_py);
    let sy = py_num / sy_exchange_rate;
    sy.ceil_u64()
}

pub fn py_to_sy_floor(sy_exchange_rate: Number, amount_py: u64) -> u64 {
    let py_num = Number::from_natural_u64(amount_py);
    let sy = py_num / sy_exchange_rate;
    sy.floor_u64()
}

/// Div-down flooring of SY tokens into PT & YT
/// Based on the current SY exchange rate
pub fn sy_to_py(sy_exchange_rate: Number, amount_sy: u64) -> u64 {
    let py = Number::from_natural_u64(amount_sy) * sy_exchange_rate;
    py.floor_u64()
}

pub fn sy_to_py_ceil(sy_exchange_rate: Number, amount_sy: u64) -> u64 {
    let py = Number::from_natural_u64(amount_sy) * sy_exchange_rate;
    py.ceil_u64()
}

/// Helper function for getting SY state
pub fn do_get_sy_state(
    alt: &AccountInfo,
    cpi_accounts: &CpiAccounts,
    rem_accounts: &[AccountInfo],
    sy_program: Pubkey,
) -> Result<SyState> {
    let t = deserialize_lookup_table(alt);
    let account_metas = to_account_metas(&cpi_accounts.get_sy_state, &t);
    cpi_get_sy_state(sy_program, rem_accounts, account_metas)
}

pub fn do_deposit_sy<'info>(
    amount: u64,
    alt: &AccountInfo,
    cpi_accounts: &CpiAccounts,
    regular_accounts: &[AccountInfo<'info>],
    rem_accounts: &[AccountInfo<'info>],
    sy_program: Pubkey,
    seeds: &[&[&[u8]]],
) -> Result<SyState> {
    let t = deserialize_lookup_table(alt);
    let account_metas = to_account_metas(&cpi_accounts.deposit_sy, &t);

    // Combine regular_accounts and rem_accounts
    let mut all_accounts = regular_accounts.to_vec();
    all_accounts.extend_from_slice(rem_accounts);

    // Filter out accounts that are not needed for the CPI
    let filtered_accounts: Vec<AccountInfo> = all_accounts
        .into_iter()
        .filter(|account| account_metas.iter().any(|meta| meta.pubkey == *account.key))
        .collect();

    cpi_deposit_sy(sy_program, amount, &filtered_accounts, account_metas, seeds)
}

pub fn do_withdraw_sy<'info>(
    amount: u64,
    alt: &AccountInfo<'info>,
    cpi_accounts: &CpiAccounts,
    regular_accounts: &[AccountInfo<'info>],
    rem_accounts: &[AccountInfo<'info>],
    sy_program: Pubkey,
    seeds: &[&[&[u8]]],
) -> Result<SyState> {
    let t = deserialize_lookup_table(alt);
    let account_metas = to_account_metas(&cpi_accounts.withdraw_sy, &t);

    // Combine regular_accounts and rem_accounts
    let mut all_accounts = regular_accounts.to_vec();
    all_accounts.extend_from_slice(rem_accounts);

    // Filter out accounts that are not needed for the CPI
    let filtered_accounts: Vec<AccountInfo> = all_accounts
        .into_iter()
        .filter(|account| account_metas.iter().any(|meta| meta.pubkey == *account.key))
        .collect();

    cpi_withdraw_sy(sy_program, amount, &filtered_accounts, account_metas, seeds)
}

pub fn do_get_position_state(
    alt: &AccountInfo,
    cpi_accounts: &CpiAccounts,
    rem_accounts: &[AccountInfo],
    sy_program: Pubkey,
    seeds: &[&[&[u8]]],
) -> Result<PositionState> {
    let t = deserialize_lookup_table(alt);
    let account_metas = to_account_metas(&cpi_accounts.get_position_state, &t);
    cpi_get_position(sy_program, rem_accounts, account_metas, seeds)
}
