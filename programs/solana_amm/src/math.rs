use crate::error::ErrorCode;
use anchor_lang::prelude::*;

/// Calculates token swap output amount based on the constant-product formula:
/// dy = (y * dx * (1 - fee)) / (x + dx * (1 - fee))
/// Rounding discipline: DOWN (favors the pool)
pub fn calculate_swap_output(
    input_amount: u64,
    pool_input_reserve: u64,
    pool_output_reserve: u64,
    fee_bps: u16,
) -> Result<u64> {
    let input_amount_u128 = input_amount as u128;
    let pool_input_reserve_u128 = pool_input_reserve as u128;
    let pool_output_reserve_u128 = pool_output_reserve as u128;

    // Apply fee reduction (e.g., 30 bps = 0.3% fee)
    let fee_multiplier = 10_000_u128
        .checked_sub(fee_bps as u128)
        .ok_or(ErrorCode::MathOverflow)?;
    
    let net_input_amount = input_amount_u128
        .checked_mul(fee_multiplier)
        .ok_or(ErrorCode::MathOverflow)?;

    let numerator = pool_output_reserve_u128
        .checked_mul(net_input_amount)
        .ok_or(ErrorCode::MathOverflow)?;

    let denominator = pool_input_reserve_u128
        .checked_mul(10_000)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_add(net_input_amount)
        .ok_or(ErrorCode::MathOverflow)?;

    let output_amount = numerator
        .checked_div(denominator)
        .ok_or(ErrorCode::MathZeroDivision)?;

    Ok(output_amount as u64)
}

/// Calculates LP tokens to mint on subsequent deposits:
/// shares = min((dx / x) * total_shares, (dy / y) * total_shares)
/// Rounding discipline: DOWN (favors the pool)
pub fn calculate_deposit_shares(
    amount_a: u64,
    amount_b: u64,
    reserve_a: u64,
    reserve_b: u64,
    total_lp_supply: u64,
) -> Result<u64> {
    let shares_a = (amount_a as u128)
        .checked_mul(total_lp_supply as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(reserve_a as u128)
        .ok_or(ErrorCode::MathZeroDivision)?;

    let shares_b = (amount_b as u128)
        .checked_mul(total_lp_supply as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(reserve_b as u128)
        .ok_or(ErrorCode::MathZeroDivision)?;

    let final_shares = std::cmp::min(shares_a, shares_b);
    Ok(final_shares as u64)
}