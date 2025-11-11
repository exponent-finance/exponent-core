use anchor_lang::prelude::*;

#[error_code]
pub enum ExponentCoreError {
    #[msg("Invalid Proxy Account")]
    InvalidProxyAccount,
    #[msg("Vault is expired")]
    VaultExpired,
    #[msg("Emission Index must be sequential")]
    EmissionIndexMustBeSequential,
    #[msg("Amount larger than staged")]
    AmountLargerThanStaged,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Duration is negative")]
    DurationNegative,
    #[msg("Farm does not exist")]
    FarmDoesNotExist,
    #[msg("Lp supply maximum exceeded")]
    LpSupplyMaximumExceeded,
    #[msg("Vault has not started yet or has ended")]
    VaultIsNotActive,
    #[msg("Operation amount too small")]
    OperationAmountTooSmall,
    #[msg("Stripping is disabled")]
    StrippingDisabled,
    #[msg("Merging is disabled")]
    MergingDisabled,
    #[msg("Depositing YT is disabled")]
    DepositingYtDisabled,
    #[msg("Withdrawing YT is disabled")]
    WithdrawingYtDisabled,
    #[msg("Collecting interest is disabled")]
    CollectingInterestDisabled,
    #[msg("Collecting Emissions is disabled")]
    CollectingEmissionsDisabled,
    #[msg("Buying PT is disabled")]
    BuyingPtDisabled,
    #[msg("Selling PT is disabled")]
    SellingPtDisabled,
    #[msg("Buying YT is disabled")]
    BuyingYtDisabled,
    #[msg("Selling YT is disabled")]
    SellingYtDisabled,
    #[msg("Depositing Liquidity is disabled")]
    DepositingLiquidityDisabled,
    #[msg("Withdrawing Liquidity is disabled")]
    WithdrawingLiquidityDisabled,
    #[msg("Vault is in emergency mode")]
    VaultInEmergencyMode,
    #[msg("Farm already exists")]
    FarmAlreadyExists,
    #[msg("Claim limit exceeded")]
    ClaimLimitExceeded,
    #[msg("Net balance change exceeds limit")]
    NetBalanceChangeExceedsLimit,
    #[msg("Min SY out not met")]
    MinSyOutNotMet,
    #[msg("Min PT out not met")]
    MinPtOutNotMet,
    #[msg("Min LP out not met")]
    MinLpOutNotMet,
}