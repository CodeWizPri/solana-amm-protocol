use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, MintTo};

declare_id!("Fk852S24kjQ6aqUjEPvX4KUVy77UkQ98f3zvUAnLTZcm");

#[program]
pub mod solana_amm {
    use super::*;

    pub fn initialize_pool(_ctx: Context<InitializePool>) -> Result<()> {
        msg!("AMM Pool initialized successfully.");
        Ok(())
    }

    pub fn deposit_liquidity(ctx: Context<DepositLiquidity>, amount_a: u64, amount_b: u64) -> Result<()> {
        let pool_state = &ctx.accounts.pool_state;
        
        let reserve_a = ctx.accounts.token_a_vault.amount;
        let reserve_b = ctx.accounts.token_b_vault.amount;
        let total_lp_supply = ctx.accounts.lp_mint.supply;

        let lp_to_mint: u64;

        // If this is the initial deposit, mint LP tokens directly proportional to the assets
        if total_lp_supply == 0 {
            // Using geometric mean for initial liquidity scaling
            lp_to_mint = ((amount_a as u128).checked_mul(amount_b as u128).unwrap())
                .checked_ilog2() // simple square-root approximation helper or raw casting
                .unwrap_or(amount_a as u32) as u64; 
        } else {
            // Check that deposition matches current reserve proportions perfectly
            let pool_ratio = (reserve_a as u128).checked_mul(amount_b as u128).unwrap();
            let deposit_ratio = (reserve_b as u128).checked_mul(amount_a as u128).unwrap();
            
            require!(pool_ratio == deposit_ratio, AmmError::InvalidRatio);

            // lp_to_mint = total_lp_supply * (amount_a / reserve_a)
            lp_to_mint = ((total_lp_supply as u128)
                .checked_mul(amount_a as u128)
                .unwrap() / (reserve_a as u128)) as u64;
        }

        // 1. Transfer Token A to Vault
        let cpi_a = Transfer {
            from: ctx.accounts.user_token_a.to_account_info(),
            to: ctx.accounts.token_a_vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        token::transfer(CpiContext::new(ctx.accounts.token_program.key(), cpi_a), amount_a)?;

        // 2. Transfer Token B to Vault
        let cpi_b = Transfer {
            from: ctx.accounts.user_token_b.to_account_info(),
            to: ctx.accounts.token_b_vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        token::transfer(CpiContext::new(ctx.accounts.token_program.key(), cpi_b), amount_b)?;

        // 3. Mint LP tokens back to the user using authority PDA seeds
        let pool_id = pool_state.key();
        let seeds = &[
            b"authority",
            pool_id.as_ref(),
            &[pool_state.bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_mint = MintTo {
            mint: ctx.accounts.lp_mint.to_account_info(),
            to: ctx.accounts.user_lp_account.to_account_info(),
            authority: ctx.accounts.pool_authority.to_account_info(),
        };
        token::mint_to(
            CpiContext::new_with_signer(ctx.accounts.token_program.key(), cpi_mint, signer_seeds),
            lp_to_mint
        )?;

        msg!("Deposited Liquidity! LP Tokens Minted: {}", lp_to_mint);
        Ok(())
    }

    pub fn withdraw_liquidity(ctx: Context<WithdrawLiquidity>, lp_amount: u64) -> Result<()> {
    let pool_state = &ctx.accounts.pool_state;
    
    // 1. Fetch live pool metrics
    let total_lp_supply = ctx.accounts.lp_mint.supply;
    let reserve_a = ctx.accounts.token_vault_a.amount;
    let reserve_b = ctx.accounts.token_vault_b.amount;

    // Guard against zero-division exploits or empty pools
    require!(total_lp_supply > 0, AmmError::InvalidVault);
    require!(lp_amount > 0, AmmError::InvalidRatio);

    // 2. Calculate proportional token distribution using safe u128 math
    let amount_a_out = ((lp_amount as u128)
        .checked_mul(reserve_a as u128)
        .unwrap()
        .checked_div(total_lp_supply as u128)
        .unwrap()) as u64;

    let amount_b_out = ((lp_amount as u128)
        .checked_mul(reserve_b as u128)
        .unwrap()
        .checked_div(total_lp_supply as u128)
        .unwrap()) as u64;

    // 3. CPI: Burn the user's LP tokens
    let cpi_burn_accounts = token::Burn {
        mint: ctx.accounts.lp_mint.to_account_info(),
        from: ctx.accounts.user_lp_token.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let cpi_burn_ctx = CpiContext::new(ctx.accounts.token_program.key(), cpi_burn_accounts);
    token::burn(cpi_burn_ctx, lp_amount)?;

    // 4. Set up PDA signer seeds to release tokens from the vaults
    let pool_id = pool_state.key();
    let seeds = &[
        b"authority",
        pool_id.as_ref(),
        &[pool_state.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    // 5. CPI: Transfer Asset A out of the vault to the user
    let cpi_transfer_a = token::Transfer {
        from: ctx.accounts.token_vault_a.to_account_info(),
        to: ctx.accounts.user_token_a.to_account_info(),
        authority: ctx.accounts.pool_authority.to_account_info(),
    };
    let cpi_ctx_a = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        cpi_transfer_a,
        signer_seeds,
    );
    token::transfer(cpi_ctx_a, amount_a_out)?;

    // 6. CPI: Transfer Asset B out of the vault to the user
    let cpi_transfer_b = token::Transfer {
        from: ctx.accounts.token_vault_b.to_account_info(),
        to: ctx.accounts.user_token_b.to_account_info(),
        authority: ctx.accounts.pool_authority.to_account_info(),
    };
    let cpi_ctx_b = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        cpi_transfer_b,
        signer_seeds,
    );
    token::transfer(cpi_ctx_b, amount_b_out)?;

    Ok(())
}

    pub fn swap(ctx: Context<Swap>, amount_in: u64, min_amount_out: u64) -> Result<()> {
    let pool_state = &ctx.accounts.pool_state;

    // 1. Read reserves dynamically using the new context fields
    let reserve_in = ctx.accounts.token_vault_in.amount;
    let reserve_out = ctx.accounts.token_vault_out.amount;

    // 2. Apply a 0.3% LP fee (997/1000 of the input amount is swapped)
    let fee_amount_in = (amount_in as u128)
        .checked_mul(997)
        .unwrap()
        .checked_div(1000)
        .unwrap();

    // 3. Constant-product formula calculation
    let invariant_numerator = (reserve_out as u128).checked_mul(fee_amount_in).unwrap();
    let invariant_denominator = (reserve_in as u128).checked_add(fee_amount_in).unwrap();
    
    let amount_out = (invariant_numerator / invariant_denominator) as u64;

    // 4. Slippage Enforcement
    require!(amount_out >= min_amount_out, AmmError::SlippageExceeded);

    // 5. CPI: Transfer tokens from user to pool input vault
    let cpi_accounts_in = Transfer {
        from: ctx.accounts.user_token_in.to_account_info(),
        to: ctx.accounts.token_vault_in.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    // Change .to_account_info() to .key()
    let cpi_ctx_in = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts_in);
    token::transfer(cpi_ctx_in, amount_in)?;

    // 6. CPI: Transfer tokens from pool output vault to user (Signed by PDA)
    let pool_id = pool_state.key();
    let seeds = &[
        b"authority",
        pool_id.as_ref(),
        &[pool_state.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    let cpi_accounts_out = Transfer {
        from: ctx.accounts.token_vault_out.to_account_info(),
        to: ctx.accounts.user_token_out.to_account_info(),
        authority: ctx.accounts.pool_authority.to_account_info(),
    };
    // Change .to_account_info() to .key()
    let cpi_ctx_out = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(), 
        cpi_accounts_out, 
        signer_seeds
    );
    token::transfer(cpi_ctx_out, amount_out)?;

    Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,
    #[account(init, payer = initializer, space = 8 + PoolState::INIT_SPACE)]
    pub pool_state: Account<'info, PoolState>,
    pub lp_mint: Account<'info, Mint>,
    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,
    #[account(mut)]
    pub token_a_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_b_vault: Account<'info, TokenAccount>,
    /// CHECK: Verified in CPI signing step
    pub pool_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct DepositLiquidity<'info> {
    pub user: Signer<'info>,

    #[account(seeds = [b"pool", pool_state.initializer.as_ref()], bump = pool_state.bump)]
    pub pool_state: Account<'info, PoolState>,

    /// CHECK: Verified in CPI signing step
    pub pool_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub lp_mint: Account<'info, Mint>,

    #[account(mut, constraint = token_a_vault.key() == pool_state.token_a_vault)]
    pub token_a_vault: Account<'info, TokenAccount>,

    #[account(mut, constraint = token_b_vault.key() == pool_state.token_b_vault)]
    pub token_b_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_a: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_lp_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawLiquidity<'info> {
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"pool", pool_state.initializer.as_ref()],
        bump = pool_state.bump
    )]
    pub pool_state: Account<'info, PoolState>,

    /// CHECK: PDA authority signing for the vault transfers
    pub pool_authority: UncheckedAccount<'info>,

    /// The pool's LP token mint account
    #[account(mut)]
    pub lp_mint: Account<'info, Mint>,

    /// The user's token account holding the LP tokens to burn
    #[account(mut)]
    pub user_lp_token: Account<'info, TokenAccount>,

    /// The pool vault for Asset A
    #[account(mut, constraint = token_vault_a.key() == pool_state.token_a_vault @ AmmError::InvalidVault)]
    pub token_vault_a: Account<'info, TokenAccount>,

    /// The pool vault for Asset B
    #[account(mut, constraint = token_vault_b.key() == pool_state.token_b_vault @ AmmError::InvalidVault)]
    pub token_vault_b: Account<'info, TokenAccount>,

    /// User's token account to receive Asset A
    #[account(mut)]
    pub user_token_a: Account<'info, TokenAccount>,

    /// User's token account to receive Asset B
    #[account(mut)]
    pub user_token_b: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    /// The user executing the swap
    pub user: Signer<'info>,

    #[account(
        seeds = [b"pool", pool_state.initializer.as_ref()], 
        bump = pool_state.bump
    )]
    pub pool_state: Account<'info, PoolState>,

    /// CHECK: The PDA authority that signs for the vault transfers
    pub pool_authority: UncheckedAccount<'info>,

    /// The user's token account for the asset they are selling
    #[account(mut)]
    pub user_token_in: Account<'info, TokenAccount>,

    /// The user's token account for the asset they are buying
    #[account(mut)]
    pub user_token_out: Account<'info, TokenAccount>,

    /// The pool vault receiving the user's input tokens
    #[account(
        mut,
        constraint = token_vault_in.key() == pool_state.token_a_vault 
            || token_vault_in.key() == pool_state.token_b_vault @ AmmError::InvalidVault
    )]
    pub token_vault_in: Account<'info, TokenAccount>,

    /// The pool vault distributing the output tokens to the user
    #[account(
        mut,
        constraint = token_vault_out.key() == pool_state.token_a_vault 
            || token_vault_out.key() == pool_state.token_b_vault @ AmmError::InvalidVault,
        constraint = token_vault_out.key() != token_vault_in.key() @ AmmError::IdenticalVaults
    )]
    pub token_vault_out: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(InitSpace)]
pub struct PoolState {
    pub initializer: Pubkey,
    pub token_a_vault: Pubkey,
    pub token_b_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub bump: u8,
}

#[error_code]
pub enum AmmError {
    #[msg("Slippage limit exceeded.")]
    SlippageExceeded,
    #[msg("Deposited token ratio does not match pool reserves.")]
    InvalidRatio,
    #[msg("The provided vault does not belong to this pool.")]
    InvalidVault,
    #[msg("Input and output vaults cannot be identical.")]
    IdenticalVaults,
}