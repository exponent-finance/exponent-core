use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{
        self,
        spl_token_2022::{
            self,
            extension::{ExtensionType, StateWithExtensions},
        },
        InitializeMint2,
    },
    token_interface::{
        initialize_account3, initialize_mint2, spl_token_2022::extension::BaseStateWithExtensions,
        InitializeAccount3,
    },
};

pub fn create_mint_2022<'i>(
    authority: &AccountInfo<'i>,
    payer: &AccountInfo<'i>,
    mint: &AccountInfo<'i>,
    token_program: &AccountInfo<'i>,
    system_program: &AccountInfo<'i>,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&[])?;
    let lamports = Rent::get()?.minimum_balance(space);

    let cpi_accounts = anchor_lang::system_program::CreateAccount {
        from: payer.to_account_info(),
        to: mint.to_account_info(),
    };

    let cpi_context = CpiContext::new(system_program.to_account_info(), cpi_accounts);

    anchor_lang::system_program::create_account(
        cpi_context.with_signer(signer_seeds),
        lamports,
        space as u64,
        token_program.key,
    )?;

    initialize_mint2(
        CpiContext::new(
            token_program.to_account_info(),
            InitializeMint2 {
                mint: mint.to_account_info(),
            },
        ),
        decimals,
        authority.key,
        None,
    )?;

    Ok(())
}

pub fn create_token_account<'a>(
    authority: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    token_account: &AccountInfo<'a>,
    mint_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let space = {
        let mint_info = mint_account;
        if *mint_info.owner == token_2022::Token2022::id() {
            let mint_data = mint_info.try_borrow_data()?;
            let mint_state =
                StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
            let mint_extensions = mint_state.get_extension_types()?;
            let required_extensions =
                ExtensionType::get_required_init_account_extensions(&mint_extensions);
            ExtensionType::try_calculate_account_len::<spl_token_2022::state::Account>(
                &required_extensions,
            )?
        } else {
            anchor_spl::token::TokenAccount::LEN
        }
    };
    let lamports = Rent::get()?.minimum_balance(space);
    let cpi_accounts = anchor_lang::system_program::CreateAccount {
        from: payer.to_account_info(),
        to: token_account.to_account_info(),
    };
    let cpi_context = CpiContext::new(system_program.to_account_info(), cpi_accounts);
    anchor_lang::system_program::create_account(
        cpi_context.with_signer(signer_seeds),
        lamports,
        space as u64,
        token_program.key,
    )?;

    initialize_account3(CpiContext::new(
        token_program.to_account_info(),
        InitializeAccount3 {
            account: token_account.to_account_info(),
            mint: mint_account.to_account_info(),
            authority: authority.to_account_info(),
        },
    ))
}

pub fn create_associated_token_account_2022<'i>(
    payer: &AccountInfo<'i>,
    authority: &AccountInfo<'i>,
    mint: &AccountInfo<'i>,
    token_account: &AccountInfo<'i>,
    token_2022_program: &AccountInfo<'i>,
    associated_token_program: &AccountInfo<'i>,
    system_program: &AccountInfo<'i>,
) -> Result<()> {
    let cpi_accounts = anchor_spl::associated_token::Create {
        payer: payer.to_account_info(),
        associated_token: token_account.to_account_info(),
        mint: mint.to_account_info(),
        authority: authority.to_account_info(),
        system_program: system_program.to_account_info(),
        token_program: token_2022_program.to_account_info(),
    };
    let cpi_ctx = anchor_lang::context::CpiContext::new(
        associated_token_program.to_account_info(),
        cpi_accounts,
    );

    anchor_spl::associated_token::create(cpi_ctx)?;

    Ok(())
}
