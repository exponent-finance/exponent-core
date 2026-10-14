use anchor_lang::{prelude::*, system_program};
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

fn create_pda_account<'info>(
    payer: &AccountInfo<'info>,
    account: &AccountInfo<'info>,
    system_program_account: &AccountInfo<'info>,
    owner: &Pubkey,
    space: usize,
    rent_exempt_lamports: u64,
    signer_seeds: &[&[&[u8]]],
    program_id: &Pubkey,
) -> Result<()> {
    // Defense in depth: the account must be the PDA derived from signer_seeds.
    // Without this check, any system-owned account can be allocated and assigned
    // to the token program at a non-canonical address, breaking the PDA/authority
    // assumptions that later CPIs (initialize_mint2 / initialize_account3) rely on.
    if let Some(seeds) = signer_seeds.first() {
        let expected = Pubkey::create_program_address(seeds, program_id)?;
        require_keys_eq!(*account.key, expected);
    }

    let lamports_diff = rent_exempt_lamports.saturating_sub(account.lamports());
    if lamports_diff > 0 {
        system_program::transfer(
            CpiContext::new(
                system_program_account.to_account_info(),
                system_program::Transfer {
                    from: payer.to_account_info(),
                    to: account.to_account_info(),
                },
            ),
            lamports_diff,
        )?;
    }

    system_program::allocate(
        CpiContext::new(
            system_program_account.to_account_info(),
            system_program::Allocate {
                account_to_allocate: account.to_account_info(),
            },
        )
        .with_signer(signer_seeds),
        space as u64,
    )?;
    system_program::assign(
        CpiContext::new(
            system_program_account.to_account_info(),
            system_program::Assign {
                account_to_assign: account.to_account_info(),
            },
        )
        .with_signer(signer_seeds),
        owner,
    )
}

pub fn create_mint_2022<'i>(
    authority: &AccountInfo<'i>,
    payer: &AccountInfo<'i>,
    mint: &AccountInfo<'i>,
    token_program: &AccountInfo<'i>,
    system_program: &AccountInfo<'i>,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
    program_id: &Pubkey,
) -> Result<()> {
    let space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&[])?;
    let lamports = Rent::get()?.minimum_balance(space);

    create_pda_account(
        payer,
        mint,
        system_program,
        token_program.key,
        space,
        lamports,
        signer_seeds,
        program_id,
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
    program_id: &Pubkey,
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
            // The else branch is only valid for the classic SPL Token program.
            // Any other owner would produce a classically-sized account handed
            // to a program that cannot initialize it.
            require_keys_eq!(*mint_info.owner, anchor_spl::token::Token::id());
            anchor_spl::token::TokenAccount::LEN
        }
    };
    let lamports = Rent::get()?.minimum_balance(space);

    create_pda_account(
        payer,
        token_account,
        system_program,
        token_program.key,
        space,
        lamports,
        signer_seeds,
        program_id,
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
