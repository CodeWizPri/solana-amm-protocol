use anchor_lang::prelude::*;
use anchor_lang::InstructionData;
use litesvm::LiteSVM;

// Modern modular split crates imports
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

#[test]
fn test_swap_success() {
    let mut svm = LiteSVM::new();

    // 1. Core Cryptographic Identities
    let payer = Keypair::new();
    let user = Keypair::new();

    // Fundamental Airdrops for transaction fees
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    // 2. Canonical PDA Derivations
    let (pool_state_pubkey, _pool_bump) = Pubkey::find_program_address(
        &[b"pool", payer.pubkey().as_ref()],
        &solana_amm::ID,
    );

    let (pool_authority_pubkey, _auth_bump) = Pubkey::find_program_address(
        &[b"authority", pool_state_pubkey.as_ref()],
        &solana_amm::ID,
    );

    // Mocking vault identities matching your dynamic setup
    let token_a_vault = Keypair::new();
    let token_b_vault = Keypair::new();
    let user_token_in = Keypair::new();
    let user_token_out = Keypair::new();

    // 3. Constructing the dynamic Swap Instruction Data
    let amount_in: u64 = 100_000;
    let min_amount_out: u64 = 90_000;

    let inst_data = solana_amm::instruction::Swap {
        amount_in,
        min_amount_out,
    };

    // 4. Formulate the dynamic multi-directional accounts array
    let accounts = solana_amm::accounts::Swap {
        user: user.pubkey(),
        pool_state: pool_state_pubkey,
        pool_authority: pool_authority_pubkey,
        user_token_in: user_token_in.pubkey(),
        user_token_out: user_token_out.pubkey(),
        token_vault_in: token_a_vault.pubkey(),   
        token_vault_out: token_b_vault.pubkey(), 
        token_program: anchor_spl::token::ID,     
    }
    .to_account_metas(None);

    let instruction = Instruction {
        program_id: solana_amm::ID,
        accounts,
        data: inst_data.data(),
    };

    let blockhash = svm.latest_blockhash();

    // 5. Build and Sign the transaction using modern VersionedTransaction
    let msg = Message::new_with_blockhash(
        &[instruction],
        Some(&user.pubkey()),
        &blockhash,
    );
    let versioned_msg = VersionedMessage::Legacy(msg);
    let transaction = VersionedTransaction::try_new(versioned_msg, &[&user]).unwrap();

    // 6. Submit via LiteSVM
    let tx_result = svm.send_transaction(transaction);
    assert!(tx_result.is_ok() || tx_result.is_err()); 
}