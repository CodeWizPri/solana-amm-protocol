use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct PoolState {
    pub token_a_mint: Pubkey,   // 32 bytes
    pub token_b_mint: Pubkey,   // 32 bytes
    pub token_a_vault: Pubkey,  // 32 bytes
    pub token_b_vault: Pubkey,  // 32 bytes
    pub lp_mint: Pubkey,        // 32 bytes
    pub pool_bump: u8,          // 1 byte
    pub authority_bump: u8,     // 1 byte
}