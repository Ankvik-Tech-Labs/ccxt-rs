//! Wallet and signing utilities for DEX transactions

use crate::base::errors::{CcxtError, Result};
use alloy::signers::local::PrivateKeySigner;

/// Wallet wrapper for signing DEX transactions
pub struct DexWallet {
    signer: PrivateKeySigner,
}

impl DexWallet {
    /// Create wallet from private key
    ///
    /// # Arguments
    /// * `private_key` - Private key hex string (with or without 0x prefix)
    pub fn from_private_key(private_key: &str) -> Result<Self> {
        let signer = private_key
            .parse::<PrivateKeySigner>()
            .map_err(|e| CcxtError::ConfigError(format!("Invalid private key: {}", e)))?;

        Ok(Self { signer })
    }

    /// Get the wallet address
    pub fn address(&self) -> alloy::primitives::Address {
        self.signer.address()
    }

    /// Get the underlying signer
    pub fn signer(&self) -> &PrivateKeySigner {
        &self.signer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_creation() {
        // Test private key (DO NOT use in production!)
        let private_key = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let wallet = DexWallet::from_private_key(private_key);
        assert!(wallet.is_ok());

        let wallet = wallet.unwrap();
        let address = wallet.address();
        assert_ne!(format!("{:?}", address), "0x0000000000000000000000000000000000000000");
    }

    #[test]
    fn test_invalid_private_key() {
        let result = DexWallet::from_private_key("invalid");
        assert!(result.is_err());
    }
}
