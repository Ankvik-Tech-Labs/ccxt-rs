//! ERC20 token utilities

use crate::base::errors::{CcxtError, Result};
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy::sol;

// Define ERC20 ABI using alloy's sol! macro
sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function decimals() external view returns (uint8);
        function symbol() external view returns (string memory);
        function name() external view returns (string memory);
        function totalSupply() external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

/// ERC20 token helper
pub struct Erc20Token {
    address: Address,
}

impl Erc20Token {
    /// Create new ERC20 token helper
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    /// Parse address from string (with or without 0x prefix)
    pub fn from_address_str(address: &str) -> Result<Self> {
        let address: Address = address
            .parse()
            .map_err(|e| CcxtError::ConfigError(format!("Invalid token address: {}", e)))?;

        Ok(Self::new(address))
    }

    /// Get token address
    pub fn address(&self) -> Address {
        self.address
    }

    /// Get balance of an address
    pub async fn balance_of<P: Provider>(
        &self,
        provider: &P,
        account: Address,
    ) -> Result<U256> {
        let contract = IERC20::new(self.address, provider);

        contract
            .balanceOf(account)
            .call()
            .await
            .map(|result| result._0)
            .map_err(|e| CcxtError::AlloyError(format!("Failed to get balance: {}", e)))
    }

    /// Get token decimals
    pub async fn decimals<P: Provider>(&self, provider: &P) -> Result<u8> {
        let contract = IERC20::new(self.address, provider);

        contract
            .decimals()
            .call()
            .await
            .map(|result| result._0)
            .map_err(|e| CcxtError::AlloyError(format!("Failed to get decimals: {}", e)))
    }

    /// Get token symbol
    pub async fn symbol<P: Provider>(&self, provider: &P) -> Result<String> {
        let contract = IERC20::new(self.address, provider);

        contract
            .symbol()
            .call()
            .await
            .map(|result| result._0)
            .map_err(|e| CcxtError::AlloyError(format!("Failed to get symbol: {}", e)))
    }

    /// Get token name
    pub async fn name<P: Provider>(&self, provider: &P) -> Result<String> {
        let contract = IERC20::new(self.address, provider);

        contract
            .name()
            .call()
            .await
            .map(|result| result._0)
            .map_err(|e| CcxtError::AlloyError(format!("Failed to get name: {}", e)))
    }

    /// Get allowance
    pub async fn allowance<P: Provider>(
        &self,
        provider: &P,
        owner: Address,
        spender: Address,
    ) -> Result<U256> {
        let contract = IERC20::new(self.address, provider);

        contract
            .allowance(owner, spender)
            .call()
            .await
            .map(|result| result._0)
            .map_err(|e| CcxtError::AlloyError(format!("Failed to get allowance: {}", e)))
    }
}

/// Convert U256 to Decimal with decimals
pub fn u256_to_decimal(value: U256, decimals: u8) -> rust_decimal::Decimal {
    use rust_decimal::Decimal;

    let value_str = value.to_string();
    let value_dec = Decimal::from_str_exact(&value_str).unwrap_or(Decimal::ZERO);

    let divisor = Decimal::from(10u64.pow(decimals as u32));
    value_dec / divisor
}

/// Convert Decimal to U256 with decimals
pub fn decimal_to_u256(value: rust_decimal::Decimal, decimals: u8) -> U256 {
    use rust_decimal::Decimal;

    let multiplier = Decimal::from(10u64.pow(decimals as u32));
    let scaled = value * multiplier;

    // Convert to string and parse as U256
    let value_str = scaled.trunc().to_string();
    U256::from_str_radix(&value_str, 10).unwrap_or(U256::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_u256_to_decimal() {
        // 1 USDC (6 decimals) = 1000000
        let value = U256::from(1_000_000u64);
        let decimal = u256_to_decimal(value, 6);
        assert_eq!(decimal, Decimal::from(1));

        // 1 ETH (18 decimals) = 10^18
        let value = U256::from_str("1000000000000000000").unwrap();
        let decimal = u256_to_decimal(value, 18);
        assert_eq!(decimal, Decimal::from(1));
    }

    #[test]
    fn test_decimal_to_u256() {
        // 1 USDC (6 decimals)
        let decimal = Decimal::from(1);
        let value = decimal_to_u256(decimal, 6);
        assert_eq!(value, U256::from(1_000_000u64));

        // 1.5 USDC
        let decimal = Decimal::from_str("1.5").unwrap();
        let value = decimal_to_u256(decimal, 6);
        assert_eq!(value, U256::from(1_500_000u64));
    }

    #[test]
    fn test_token_address_parsing() {
        let result = Erc20Token::from_address_str("0x6B175474E89094C44Da98b954EedeAC495271d0F");
        assert!(result.is_ok());

        let result = Erc20Token::from_address_str("invalid");
        assert!(result.is_err());
    }
}
