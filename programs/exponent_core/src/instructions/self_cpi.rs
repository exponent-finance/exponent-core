use crate::{
    __cpi_client_accounts_buy_yt::BuyYt,
    __cpi_client_accounts_collect_interest::CollectInterest,
    __cpi_client_accounts_deposit_liquidity::DepositLiquidity,
    __cpi_client_accounts_deposit_lp::DepositLp,
    __cpi_client_accounts_deposit_yt::DepositYt,
    __cpi_client_accounts_merge::Merge,
    __cpi_client_accounts_sell_yt::SellYt,
    __cpi_client_accounts_strip::Strip,
    __cpi_client_accounts_trade_pt::TradePt,
    __cpi_client_accounts_withdraw_liquidity::WithdrawLiquidity,
    __cpi_client_accounts_withdraw_lp::WithdrawLp,
    __cpi_client_accounts_withdraw_yt::WithdrawYt,
    instructions::{
        CollectInterestEventV2, DepositLpEventV2, DepositYtEventV2, WithdrawLpEventV2,
        WithdrawYtEventV2,
    },
    ID,
};
use amount_value::Amount;
use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::Instruction,
        program::{get_return_data, invoke},
    },
};

use super::{
    BuyYtEvent, DepositLiquidityEvent, MergeEvent, SellYtEvent, StripEvent, TradePtEvent,
    WithdrawLiquidityEvent,
};

pub type MergeAccounts<'i> = Merge<'i>;
pub type StripAccounts<'i> = Strip<'i>;
pub type TradePtAccounts<'i> = TradePt<'i>;
pub type BuyYtAccounts<'i> = BuyYt<'i>;
pub type SellYtAccounts<'i> = SellYt<'i>;
pub type DepositLiquidityAccounts<'i> = DepositLiquidity<'i>;
pub type WithdrawLiquidityAccounts<'i> = WithdrawLiquidity<'i>;
pub type DepositYtAccounts<'i> = DepositYt<'i>;
pub type _WithdrawYtAccounts<'i> = WithdrawYt<'i>;
pub type DepositLpAccounts<'i> = DepositLp<'i>;
pub type WithdrawLpAccounts<'i> = WithdrawLp<'i>;
pub type CollectInterestAccounts<'i> = CollectInterest<'i>;

pub fn do_cpi_deposit_yt<'i>(
    accounts: DepositYtAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    amount_yt: u64,
) -> Result<DepositYtEventV2> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [7];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&amount_yt.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn _do_cpi_withdraw_yt<'i>(
    accounts: _WithdrawYtAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    amount_yt: u64,
) -> Result<WithdrawYtEventV2> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [8];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&amount_yt.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_deposit_liquidity<'i>(
    accounts: DepositLiquidityAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    pt_intent: u64,
    sy_intent: u64,
    min_lp_out: u64,
) -> Result<DepositLiquidityEvent> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [11];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&pt_intent.to_le_bytes());
    data.extend(&sy_intent.to_le_bytes());
    data.extend(&min_lp_out.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_deposit_lp<'i>(
    accounts: DepositLpAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    amount_lp: u64,
) -> Result<DepositLpEventV2> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [14];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&amount_lp.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_strip<'i>(
    accounts: Strip<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    amount_sy: u64,
) -> Result<StripEvent> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [4];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&amount_sy.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_merge<'i>(
    accounts: MergeAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    amount_py: u64,
) -> Result<MergeEvent> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [5];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&amount_py.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_trade_pt<'i>(
    accounts: TradePtAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    net_trader_pt: i64,
    sy_constraint: i64,
) -> Result<TradePtEvent> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [17];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&net_trader_pt.to_le_bytes());
    data.extend(&sy_constraint.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_collect_interest<'i>(
    accounts: CollectInterestAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    amount: Amount,
) -> Result<CollectInterestEventV2> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [6];
    data.extend_from_slice(discriminator.as_slice());
    amount.serialize(&mut data)?;

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_buy_yt<'i>(
    accounts: BuyYtAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    max_sy_in: u64,
    yt_out: u64,
) -> Result<BuyYtEvent> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [0];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&max_sy_in.to_le_bytes());
    data.extend(&yt_out.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_sell_yt<'i>(
    accounts: SellYtAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    yt_in: u64,
    min_sy_out: u64,
) -> Result<SellYtEvent> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [1];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&yt_in.to_le_bytes());
    data.extend(&min_sy_out.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_withdraw_lp<'i>(
    accounts: WithdrawLpAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    amount: u64,
) -> Result<WithdrawLpEventV2> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [15];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&amount.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

pub fn do_cpi_withdraw_liquidity<'i>(
    accounts: WithdrawLiquidityAccounts<'i>,
    remaining_accounts: &[AccountInfo<'i>],
    lp_in: u64,
    min_pt_out: u64,
    min_sy_out: u64,
) -> Result<WithdrawLiquidityEvent> {
    let mut data: Vec<u8> = vec![];

    let discriminator = [12];
    data.extend_from_slice(discriminator.as_slice());
    data.extend(&lp_in.to_le_bytes());
    data.extend(&min_pt_out.to_le_bytes());
    data.extend(&min_sy_out.to_le_bytes());

    do_cpi(accounts, remaining_accounts, data)?;

    deser_return_data()
}

/// Generic function for deserializing return data
fn deser_return_data<T>() -> Result<T>
where
    T: AnchorDeserialize,
{
    let d = get_return_data().unwrap().1;
    let t = T::try_from_slice(&d)?;
    Ok(t)
}

/// Generic function for handling CPI calls
fn do_cpi<'i, T>(accounts: T, remaining_accounts: &[AccountInfo<'i>], data: Vec<u8>) -> Result<()>
where
    T: ToAccountMetas + ToAccountInfos<'i>,
{
    let mut account_metas = accounts.to_account_metas(None);

    let mut new_account_metas = Vec::new();

    // De-duplicate account infos
    for ra in remaining_accounts {
        // check if the remaining account is already in the current account_metas
        let found_account = account_metas.iter_mut().find(|am| am.pubkey == *ra.key);

        match found_account {
            Some(_) => {
                // noop
            }
            None => {
                new_account_metas.push(AccountMeta {
                    pubkey: *ra.key,
                    is_signer: ra.is_signer,
                    is_writable: ra.is_writable,
                });
            }
        }
    }

    // finally, extend the account_metas with the new ones
    account_metas.extend(new_account_metas);

    // Collect account infos from the main accounts
    let mut unique_account_infos: Vec<AccountInfo> = accounts.to_account_infos();

    unique_account_infos.extend_from_slice(remaining_accounts);

    let ix = Instruction {
        program_id: ID,
        accounts: account_metas,
        data,
    };

    invoke(&ix, &unique_account_infos)?;

    Ok(())
}
