use anchor_lang::prelude::*;

#[error_code]
pub enum VaultError {
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Insufficient vault token balance")]
    InsufficientFunds,
    #[msg("Signer is not the vault owner")]
    Unauthorized,
}