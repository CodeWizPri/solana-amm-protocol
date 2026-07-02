use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Numerical overflow occurred during computation.")]
    MathOverflow,
    #[msg("Division by zero encountered.")]
    MathZeroDivision,
    #[msg("Slippage tolerance exceeded.")]
    SlippageExceeded,
    #[msg("Initial liquidity deposit is below minimum limits.")]
    InvalidInitialLiquidity,
}