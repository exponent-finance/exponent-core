use anchor_lang::prelude::*;
use exponent_admin::Admin;

#[derive(Accounts)]
pub struct ReallocMarket<'info> {
    /// CHECK: high trust instruction
    #[account(mut)]
    pub market: UncheckedAccount<'info>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub admin_state: Account<'info, Admin>,

    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,
}

impl ReallocMarket<'_> {
    pub fn validate(&self) -> Result<()> {
        self.admin_state
            .principles
            .exponent_core
            .is_admin(&self.signer.key())?;

        Ok(())
    }
}

/// This instruction is used to reallocate the market account to a new size with an additional byte.
#[access_control(ctx.accounts.validate())]
pub fn handler(ctx: Context<ReallocMarket>, additional_bytes: u64) -> Result<()> {
    let market = &mut ctx.accounts.market;

    let current_size = market.to_account_info().data_len();
    let new_size = current_size + additional_bytes as usize;

    let lamports_required = Rent::get()?.minimum_balance(new_size);
    let lamports_to_transfer = lamports_required
        .checked_sub(market.to_account_info().lamports())
        .unwrap_or(0);

    if lamports_to_transfer > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.signer.to_account_info(),
                    to: market.to_account_info(),
                },
            ),
            lamports_to_transfer,
        )?;
    }

    market.realloc(new_size, false)?;

    Ok(())
}
