//! EIP-712 signing for Hyperliquid
//!
//! Hyperliquid uses two distinct EIP-712 signing schemes:
//!
//! 1. **L1 Action signing** (trading): Hash action via MessagePack + keccak256, then sign
//!    a phantom `Agent { source, connectionId }` struct.
//! 2. **User-signed actions** (withdrawals, transfers): Standard EIP-712 with
//!    `HyperliquidSignTransaction:*` primary types.

use crate::base::errors::{CcxtError, Result};
use crate::hyperliquid::constants;
use alloy::primitives::{keccak256, Address, B256};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use alloy::sol;
use alloy::sol_types::{eip712_domain, SolStruct};
use serde_json::Value;

// Define the EIP-712 Agent struct for L1 action signing
sol! {
    struct Agent {
        string source;
        bytes32 connectionId;
    }
}

// Define the EIP-712 structs for user-signed actions
sol! {
    #[derive(Debug)]
    struct UsdSend {
        string hyperliquidChain;
        string destination;
        string amount;
        uint64 time;
    }

    #[derive(Debug)]
    struct Withdraw {
        string hyperliquidChain;
        string destination;
        string amount;
        uint64 time;
    }

    #[derive(Debug)]
    struct UsdClassTransfer {
        string hyperliquidChain;
        string destination;
        string amount;
        uint64 time;
    }
}

/// EIP-712 signer for Hyperliquid.
pub struct HyperliquidSigner {
    signer: PrivateKeySigner,
    is_mainnet: bool,
    chain_id: u64,
}

impl HyperliquidSigner {
    /// Create a new signer from a hex-encoded private key.
    ///
    /// :param private_key: Hex-encoded private key (with or without 0x prefix).
    /// :param is_mainnet: Whether to use mainnet signing parameters.
    pub fn new(private_key: &str, is_mainnet: bool) -> Result<Self> {
        let signer: PrivateKeySigner = private_key
            .parse()
            .map_err(|e| CcxtError::AuthenticationError(format!("Invalid private key: {}", e)))?;

        let chain_id = if is_mainnet {
            constants::MAINNET_CHAIN_ID
        } else {
            constants::TESTNET_CHAIN_ID
        };

        Ok(Self {
            signer,
            is_mainnet,
            chain_id,
        })
    }

    /// Get the wallet address.
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Get the wallet address as a lowercase hex string.
    pub fn address_hex(&self) -> String {
        format!("{:#x}", self.address())
    }

    /// Sign an L1 action (trading: orders, cancels, leverage updates, etc.).
    ///
    /// The signing flow:
    /// 1. Serialize the action to MessagePack bytes
    /// 2. Append nonce (8 bytes big-endian) + vault flag + optional vault address
    /// 3. keccak256 hash the combined bytes → connectionId
    /// 4. Sign an EIP-712 Agent { source, connectionId } struct
    pub async fn sign_l1_action(
        &self,
        action: &Value,
        vault_address: Option<&str>,
        nonce: u64,
    ) -> Result<SignatureComponents> {
        let connection_id = self.compute_action_hash(action, vault_address, nonce)?;

        let source = if self.is_mainnet {
            constants::MAINNET_SOURCE
        } else {
            constants::TESTNET_SOURCE
        };

        let agent = Agent {
            source: source.to_string(),
            connectionId: connection_id,
        };

        let domain = eip712_domain! {
            name: constants::L1_DOMAIN_NAME,
            version: constants::DOMAIN_VERSION,
            chain_id: self.chain_id,
            verifying_contract: Address::ZERO,
        };

        let signature = self
            .signer
            .sign_typed_data(&agent, &domain)
            .await
            .map_err(|e| CcxtError::AuthenticationError(format!("Failed to sign L1 action: {}", e)))?;

        Ok(SignatureComponents {
            r: format!("{:#066x}", signature.r()),
            s: format!("{:#066x}", signature.s()),
            v: if signature.v() { 28 } else { 27 },
        })
    }

    /// Sign a withdraw action (user-signed).
    pub async fn sign_withdraw(
        &self,
        destination: &str,
        amount: &str,
        time: u64,
    ) -> Result<SignatureComponents> {
        let chain_name = if self.is_mainnet {
            "Mainnet"
        } else {
            "Testnet"
        };

        let withdraw = Withdraw {
            hyperliquidChain: chain_name.to_string(),
            destination: destination.to_string(),
            amount: amount.to_string(),
            time,
        };

        self.sign_user_action(&withdraw).await
    }

    /// Sign a USD transfer action (user-signed).
    pub async fn sign_usd_transfer(
        &self,
        destination: &str,
        amount: &str,
        time: u64,
    ) -> Result<SignatureComponents> {
        let chain_name = if self.is_mainnet {
            "Mainnet"
        } else {
            "Testnet"
        };

        let transfer = UsdClassTransfer {
            hyperliquidChain: chain_name.to_string(),
            destination: destination.to_string(),
            amount: amount.to_string(),
            time,
        };

        self.sign_user_action(&transfer).await
    }

    /// Sign a user-signed action with the HyperliquidSignTransaction domain.
    async fn sign_user_action<T: SolStruct + Send + Sync>(&self, data: &T) -> Result<SignatureComponents> {
        let domain = eip712_domain! {
            name: constants::USER_DOMAIN_NAME,
            version: constants::DOMAIN_VERSION,
            chain_id: constants::USER_SIGNED_CHAIN_ID,
            verifying_contract: Address::ZERO,
        };

        let signature = self
            .signer
            .sign_typed_data(data, &domain)
            .await
            .map_err(|e| {
                CcxtError::AuthenticationError(format!("Failed to sign user action: {}", e))
            })?;

        Ok(SignatureComponents {
            r: format!("{:#066x}", signature.r()),
            s: format!("{:#066x}", signature.s()),
            v: if signature.v() { 28 } else { 27 },
        })
    }

    /// Compute the action hash (keccak256 of msgpack(action) + nonce + vault info).
    fn compute_action_hash(
        &self,
        action: &Value,
        vault_address: Option<&str>,
        nonce: u64,
    ) -> Result<B256> {
        let mut data = rmp_serde::to_vec(action)
            .map_err(|e| CcxtError::ParseError(format!("Failed to msgpack action: {}", e)))?;

        // Append nonce as 8 bytes big-endian
        data.extend_from_slice(&nonce.to_be_bytes());

        // Append vault flag and optional vault address
        match vault_address {
            None => {
                data.push(0x00);
            }
            Some(addr) => {
                data.push(0x01);
                let addr_bytes = hex::decode(addr.trim_start_matches("0x")).map_err(|e| {
                    CcxtError::AuthenticationError(format!("Invalid vault address: {}", e))
                })?;
                data.extend_from_slice(&addr_bytes);
            }
        }

        Ok(keccak256(&data))
    }
}

/// Extracted r, s, v components of an EIP-712 signature.
#[derive(Debug, Clone)]
pub struct SignatureComponents {
    pub r: String,
    pub s: String,
    pub v: u8,
}

impl SignatureComponents {
    /// Convert to JSON for the /exchange request.
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "r": self.r,
            "s": self.s,
            "v": self.v,
        })
    }
}

/// Remove trailing zeros from a decimal string for Hyperliquid wire format.
///
/// "29300.00000000" → "29300.0"
/// "29300.10" → "29300.1"
/// "29300" → "29300.0"
pub fn float_to_wire(value: &str) -> String {
    if !value.contains('.') {
        return format!("{}.0", value);
    }
    let trimmed = value.trim_end_matches('0');
    if trimmed.ends_with('.') {
        format!("{}0", trimmed)
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_float_to_wire() {
        assert_eq!(float_to_wire("29300.00000000"), "29300.0");
        assert_eq!(float_to_wire("29300.10"), "29300.1");
        assert_eq!(float_to_wire("29300"), "29300.0");
        assert_eq!(float_to_wire("0.001"), "0.001");
        assert_eq!(float_to_wire("100.0"), "100.0");
        assert_eq!(float_to_wire("1.20"), "1.2");
    }

    #[test]
    fn test_signer_creation() {
        // Known test private key (do not use for real funds)
        let key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let signer = HyperliquidSigner::new(key, true).unwrap();
        assert!(!signer.address_hex().is_empty());
        assert!(signer.address_hex().starts_with("0x"));
    }

    #[test]
    fn test_signer_invalid_key() {
        let result = HyperliquidSigner::new("invalid", true);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sign_l1_action() {
        let key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let signer = HyperliquidSigner::new(key, true).unwrap();

        let action = serde_json::json!({
            "type": "order",
            "orders": [{
                "a": 0,
                "b": true,
                "p": "95000.0",
                "s": "0.01",
                "r": false,
                "t": {"limit": {"tif": "Gtc"}}
            }],
            "grouping": "na"
        });

        let sig = signer.sign_l1_action(&action, None, 1707000000000).await.unwrap();
        assert!(sig.r.starts_with("0x"));
        assert!(sig.s.starts_with("0x"));
        assert!(sig.v == 27 || sig.v == 28);
    }
}
