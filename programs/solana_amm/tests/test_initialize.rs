use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;
use anchor_lang::ToAccountMetas; 
use anchor_lang::prelude::*;
use solana_program::program_pack::Pack;

anchor_lang::declare_program!(solana_amm);

#[test]
fn test_initialize() {
    let mut svm = LiteSVM::new();

    // Deploy program binary
    let program_bytes = std::fs::read("../../target/deploy/solana_amm.so")
        .expect("Failed to find compiled program binary. Did you run 'anchor build'?");
    svm.add_program(solana_amm::ID, &program_bytes).unwrap();

    // Setup Payer
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    // Generate Keypairs
    let pool_state = Keypair::new();
    let lp_mint = Keypair::new();
    let token_a_mint = Keypair::new();
    let token_b_mint = Keypair::new();
    let token_a_vault = Keypair::new();
    let token_b_vault = Keypair::new();
    let pool_authority = Keypair::new();

    let rent = rent::Rent::default();

    // FIX: Read the exact internal Account layout from LiteSVM *once* up front
    let base_account_template = svm.get_account(&payer.pubkey()).unwrap();

    // 1. Initialize LP, Token A, and Token B mint states
    for mint_keypair in &[&lp_mint, &token_a_mint, &token_b_mint] {
        let mint_space = anchor_spl::token::spl_token::state::Mint::LEN;
        let mint_lamports = rent.minimum_balance(mint_space);
        
        let mut mint_data = vec![0u8; mint_space];
        let mint_state = anchor_spl::token::spl_token::state::Mint {
            mint_authority: solana_program::program_option::COption::Some(pool_authority.pubkey()),
            supply: 0,
            decimals: 6,
            is_initialized: true,
            freeze_authority: solana_program::program_option::COption::None,
        };
        anchor_spl::token::spl_token::state::Mint::pack(mint_state, &mut mint_data).unwrap();

        // Clone the type-safe template context and swap the inner data
        let mut account = base_account_template.clone();
        account.lamports = mint_lamports;
        account.data = mint_data;
        account.owner = anchor_spl::token::ID;

        svm.set_account(mint_keypair.pubkey(), account).unwrap();
    }

    // 2. Initialize Vault Token Accounts
    for (vault_keypair, mint_pubkey) in &[(&token_a_vault, token_a_mint.pubkey()), (&token_b_vault, token_b_mint.pubkey())] {
        let account_space = anchor_spl::token::spl_token::state::Account::LEN;
        let account_lamports = rent.minimum_balance(account_space);

        let mut account_data = vec![0u8; account_space];
        let account_state = anchor_spl::token::spl_token::state::Account {
            mint: *mint_pubkey,
            owner: pool_authority.pubkey(),
            amount: 0,
            delegate: solana_program::program_option::COption::None,
            state: anchor_spl::token::spl_token::state::AccountState::Initialized,
            is_native: solana_program::program_option::COption::None,
            delegated_amount: 0,
            close_authority: solana_program::program_option::COption::None,
        };
        anchor_spl::token::spl_token::state::Account::pack(account_state, &mut account_data).unwrap();

        // Clone the type-safe template context and swap the inner data
        let mut account = base_account_template.clone();
        account.lamports = account_lamports;
        account.data = account_data;
        account.owner = anchor_spl::token::ID;

        svm.set_account(vault_keypair.pubkey(), account).unwrap();
    }

    // Build the accounts structure
    let accounts = solana_amm::client::accounts::InitializePool {
        initializer: payer.pubkey(),
        pool_state: pool_state.pubkey(),
        lp_mint: lp_mint.pubkey(),
        token_a_mint: token_a_mint.pubkey(),
        token_b_mint: token_b_mint.pubkey(),
        token_a_vault: token_a_vault.pubkey(),
        token_b_vault: token_b_vault.pubkey(),
        pool_authority: pool_authority.pubkey(),
        token_program: anchor_spl::token::ID,
        system_program: anchor_lang::system_program::ID,
        rent: solana_program::sysvar::rent::ID,
    };

    let ix = solana_program::instruction::Instruction {
        program_id: solana_amm::ID,
        accounts: accounts.to_account_metas(None),
        data: anchor_lang::InstructionData::data(&solana_amm::client::args::InitializePool {}),
    };

    let recent_blockhash = svm.latest_blockhash();
    
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &pool_state],
        recent_blockhash,
    );

    let tx_result = svm.send_transaction(tx);
    
    if let Err(ref err) = tx_result {
        println!("--- TRANSACTION FAILED ---");
        println!("Error Context: {:#?}", err);
    } else if let Ok(ref meta) = tx_result {
        println!("--- TRANSACTION SUCCESS ---");
        println!("Logs: {:?}", meta.logs);
    }

    assert!(tx_result.is_ok());
}