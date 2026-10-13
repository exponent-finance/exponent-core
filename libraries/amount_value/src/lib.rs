use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum Amount {
    All,
    Some(u64),
}

impl Amount {
    pub fn to_u64(self, position_balance: u64) -> Result<u64> {
        match self {
            Amount::All => Ok(position_balance),
            Amount::Some(requested_amount) => {
                require!(
                    requested_amount <= position_balance,
                    AmountError::AmountLargerThanAvailable
                );
                Ok(requested_amount)
            }
        }
    }
}

#[error_code]
pub enum AmountError {
    #[msg("Amount is larger than available")]
    AmountLargerThanAvailable,
}
