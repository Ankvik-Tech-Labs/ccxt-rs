//! Alloy provider setup for EVM chains

use crate::base::errors::{CcxtError, Result};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use std::sync::Arc;

/// Provider wrapper for EVM chains
pub struct EvmProvider {
    provider: Arc<RootProvider<Http<Client>>>,
    chain_id: u64,
}

impl EvmProvider {
    /// Create a new EVM provider
    ///
    /// # Arguments
    /// * `rpc_url` - RPC endpoint URL
    /// * `chain_id` - Chain ID (1 for Ethereum mainnet, 56 for BSC, etc.)
    pub async fn new(rpc_url: &str, chain_id: u64) -> Result<Self> {
        let provider = ProviderBuilder::new()
            .on_http(rpc_url.parse().map_err(|e| {
                CcxtError::ConfigError(format!("Invalid RPC URL: {}", e))
            })?);

        Ok(Self {
            provider: Arc::new(provider),
            chain_id,
        })
    }

    /// Get the underlying alloy provider
    pub fn provider(&self) -> &RootProvider<Http<Client>> {
        &self.provider
    }

    /// Get chain ID
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get current block number
    pub async fn get_block_number(&self) -> Result<u64> {
        self.provider
            .get_block_number()
            .await
            .map_err(|e| CcxtError::AlloyError(format!("Failed to get block number: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires RPC connection
    async fn test_provider_creation() {
        // This would require a real RPC endpoint
        let result = EvmProvider::new("https://eth.llamarpc.com", 1).await;
        assert!(result.is_ok());
    }
}
