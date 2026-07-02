use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;
use anchor_lang::ToAccountMetas; 
use anchor_lang::prelude::*;
use solana_program::program_pack::Pack;

anchor_lang::declare_program!(solana_amm);

#[test]
fn test_deposit_liquidity() {
    let mut svm = LiteSVM::new();

    // Deploy program binary
    let program_bytes = std::fs::read("../../target/deploy/solana_amm.so")
        .expect("Failed to find compiled program binary.");
    svm.add_program(solana_amm::ID, &program_bytes).unwrap();

    // Setup Payer & Depositor User
    let payer = Keypair::new();
    let depositor = Keypair::new();
    svm.airdrop(&payer.pubkey(), 5_000_000_000).unwrap();
    svm.airdrop(&depositor.pubkey(), 5_000_000_000).unwrap();

    // Setup state identities
    let token_a_mint = Keypair::new();
    let token_b_mint = Keypair::new();
    let lp_mint = Keypair::new();

    let token_a_vault = Keypair::new();
    let token_b_vault = Keypair::new();

    // Now we can dynamically derive the right PDA using our mints!
    let (pool_state_pubkey, bump_seed) = Pubkey::find_program_address(
        &[
            b"pool",
            payer.pubkey().as_ref(),
        ],
        &solana_amm::ID,
    );

    let (pool_authority_pubkey, _auth_bump) = Pubkey::find_program_address(
    &[
        b"authority",
        pool_state_pubkey.as_ref(),
    ],
    &solana_amm::ID,
);

    // User's token accounts
    let depositor_token_a = Keypair::new();
    let depositor_token_b = Keypair::new();
    let depositor_lp = Keypair::new();

    let rent = rent::Rent::default();
    let base_account_template = svm.get_account(&payer.pubkey()).unwrap();

    // 1. Setup Mint States
    for mint_keypair in &[&lp_mint, &token_a_mint, &token_b_mint] {
        let mut mint_data = vec![0u8; anchor_spl::token::spl_token::state::Mint::LEN];
        let mint_state = anchor_spl::token::spl_token::state::Mint {
            mint_authority: solana_program::program_option::COption::Some(pool_authority_pubkey),
            supply: 0,
            decimals: 6,
            is_initialized: true,
            freeze_authority: solana_program::program_option::COption::None,
        };
        anchor_spl::token::spl_token::state::Mint::pack(mint_state, &mut mint_data).unwrap();

        let mut account = base_account_template.clone();
        account.lamports = rent.minimum_balance(anchor_spl::token::spl_token::state::Mint::LEN);
        account.data = mint_data;
        account.owner = anchor_spl::token::ID;
        svm.set_account(mint_keypair.pubkey(), account).unwrap();
    }

    // 2. Setup Vault Token Accounts (Empty Pool Vaults)
    for (vault_keypair, mint_pubkey) in &[(&token_a_vault, token_a_mint.pubkey()), (&token_b_vault, token_b_mint.pubkey())] {
        let mut account_data = vec![0u8; anchor_spl::token::spl_token::state::Account::LEN];
        let account_state = anchor_spl::token::spl_token::state::Account {
            mint: *mint_pubkey,
            owner: pool_authority_pubkey,
            amount: 0,
            delegate: solana_program::program_option::COption::None,
            state: anchor_spl::token::spl_token::state::AccountState::Initialized,
            is_native: solana_program::program_option::COption::None,
            delegated_amount: 0,
            close_authority: solana_program::program_option::COption::None,
        };
        anchor_spl::token::spl_token::state::Account::pack(account_state, &mut account_data).unwrap();

        let mut account = base_account_template.clone();
        account.lamports = rent.minimum_balance(anchor_spl::token::spl_token::state::Account::LEN);
        account.data = account_data;
        account.owner = anchor_spl::token::ID;
        svm.set_account(vault_keypair.pubkey(), account).unwrap();
    }

    // 3. Setup Depositor Token Accounts with Starting Balances (e.g., 500 Tokens each)
    let initial_user_balance = 500_000_000; 
    for (user_ata, mint_pubkey, amount) in &[
        (&depositor_token_a, token_a_mint.pubkey(), initial_user_balance),
        (&depositor_token_b, token_b_mint.pubkey(), initial_user_balance),
        (&depositor_lp, lp_mint.pubkey(), 0)
    ] {
        let mut account_data = vec![0u8; anchor_spl::token::spl_token::state::Account::LEN];
        let account_state = anchor_spl::token::spl_token::state::Account {
            mint: *mint_pubkey,
            owner: depositor.pubkey(),
            amount: *amount,
            delegate: solana_program::program_option::COption::None,
            state: anchor_spl::token::spl_token::state::AccountState::Initialized,
            is_native: solana_program::program_option::COption::None,
            delegated_amount: 0,
            close_authority: solana_program::program_option::COption::None,
        };
        anchor_spl::token::spl_token::state::Account::pack(account_state, &mut account_data).unwrap();

        let mut account = base_account_template.clone();
        account.lamports = rent.minimum_balance(anchor_spl::token::spl_token::state::Account::LEN);
        account.data = account_data;
        account.owner = anchor_spl::token::ID;
        svm.set_account(user_ata.pubkey(), account).unwrap();
    }

    // 4. Pre-populate the PoolState account to simulate an initialized AMM
    let mut pool_state_data = vec![0u8; 8 + 32 + 32 + 32 + 32 + 1];
    // Dynamically extract the exact discriminator generated by your contract
    use anchor_lang::Discriminator;
    let discriminator = solana_amm::accounts::PoolState::DISCRIMINATOR;
    pool_state_data[0..8].copy_from_slice(&discriminator);
    pool_state_data[8..40].copy_from_slice(&payer.pubkey().to_bytes());        // initializer
    pool_state_data[40..72].copy_from_slice(&token_a_vault.pubkey().to_bytes()); // token_a_vault
    pool_state_data[72..104].copy_from_slice(&token_b_vault.pubkey().to_bytes()); // token_b_vault
    pool_state_data[104..136].copy_from_slice(&lp_mint.pubkey().to_bytes());     // lp_mint
    pool_state_data[136] = bump_seed; // Use the actual derived bump seed here

    let mut pool_account = base_account_template.clone();
    pool_account.lamports = rent.minimum_balance(pool_state_data.len());
    pool_account.data = pool_state_data;
    pool_account.owner = solana_amm::ID;
    svm.set_account(pool_state_pubkey, pool_account).unwrap();

    // 5. Build the deposit_liquidity Context Struct
    let accounts = solana_amm::client::accounts::DepositLiquidity {
        user: depositor.pubkey(),
        pool_state: pool_state_pubkey,
        lp_mint: lp_mint.pubkey(),
        token_a_vault: token_a_vault.pubkey(),
        token_b_vault: token_b_vault.pubkey(),
        user_token_a: depositor_token_a.pubkey(),
        user_token_b: depositor_token_b.pubkey(),
        user_lp_account: depositor_lp.pubkey(),
        pool_authority: pool_authority_pubkey,
        token_program: anchor_spl::token::ID,
    };

    // Define deposit amounts (e.g., depositing 100 Token A and 100 Token B)
    let amount_a = 100_000_000u64;
    let amount_b = 100_000_000u64;

    let ix = solana_program::instruction::Instruction {
        program_id: solana_amm::ID,
        accounts: accounts.to_account_metas(None),
        data: anchor_lang::InstructionData::data(&solana_amm::client::args::DepositLiquidity {
            amount_a,
            amount_b,
        }),
    };

    let recent_blockhash = svm.latest_blockhash();
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&depositor.pubkey()),
        &[&depositor],
        recent_blockhash,
    );

    let tx_result = svm.send_transaction(tx);
    
    if let Err(ref err) = tx_result {
        println!("--- DEPOSIT TRANSACTION FAILED ---");
        println!("Error Context: {:#?}", err);
    } else if let Ok(ref meta) = tx_result {
        println!("--- DEPOSIT TRANSACTION SUCCESS ---");
        println!("Logs: {:?}", meta.logs);
        
        // Assert the user spent tokens and received LP tokens
        let end_user_a = svm.get_account(&depositor_token_a.pubkey()).unwrap();
        let token_a_state = anchor_spl::token::spl_token::state::Account::unpack(&end_user_a.data).unwrap();
        assert_eq!(token_a_state.amount, initial_user_balance - amount_a);
    }

    assert!(tx_result.is_ok());
}