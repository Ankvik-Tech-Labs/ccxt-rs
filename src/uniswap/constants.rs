//! Uniswap constants and chain configurations

use serde::{Deserialize, Serialize};

/// Chain configuration for Uniswap deployments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    /// Chain ID (e.g., 1 for Ethereum mainnet)
    pub chain_id: u64,
    /// Chain name
    pub name: &'static str,
    /// Default RPC URL
    pub rpc_url: &'static str,

    // Uniswap V3 addresses
    /// V3 Factory contract address
    pub v3_factory: &'static str,
    /// V3 Router contract address (SwapRouter v1)
    pub v3_router: &'static str,
    /// V3 SwapRouter02 contract address (universal router, preferred for swaps)
    pub v3_swap_router02: &'static str,
    /// V3 QuoterV2 contract address
    pub v3_quoter: &'static str,
    /// V3 NFT Position Manager contract address
    pub v3_nft_position_manager: &'static str,

    // Uniswap V2 addresses
    /// V2 Factory contract address
    pub v2_factory: &'static str,
    /// V2 Router contract address
    pub v2_router: &'static str,

    // Subgraph endpoints
    /// V3 subgraph endpoint
    pub subgraph_v3: &'static str,
    /// V2 subgraph endpoint
    pub subgraph_v2: &'static str,
}

/// Ethereum mainnet configuration
pub const ETHEREUM: ChainConfig = ChainConfig {
    chain_id: 1,
    name: "Ethereum",
    rpc_url: "https://eth.llamarpc.com",

    // V3 addresses (from Uniswap deployment)
    v3_factory: "0x1F98431c8aD98523631AE4a59f267346ea31F984",
    v3_router: "0xE592427A0AEce92De3Edee1F18E0157C05861564",
    v3_swap_router02: "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
    v3_quoter: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6",
    v3_nft_position_manager: "0xC36442b4a4522E871399CD717aBDD847Ab11FE88",

    // V2 addresses
    v2_factory: "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
    v2_router: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D",

    // Subgraphs (The Graph Decentralized Network)
    // Note: Replace {api_key} with actual API key at runtime
    subgraph_v3: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/5zvR82QoaXYFyDEKLZ9t6v9adgnptxYpKpSbxtgVENFV",
    subgraph_v2: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/A3Np3RQbaBA6oKJgiwDJeo5T3zrYfGHPWFYayMwtNDum",
};

/// Polygon (formerly Matic) configuration
pub const POLYGON: ChainConfig = ChainConfig {
    chain_id: 137,
    name: "Polygon",
    rpc_url: "https://polygon-rpc.com",

    // V3 addresses
    v3_factory: "0x1F98431c8aD98523631AE4a59f267346ea31F984",
    v3_router: "0xE592427A0AEce92De3Edee1F18E0157C05861564",
    v3_swap_router02: "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
    v3_quoter: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6",
    v3_nft_position_manager: "0xC36442b4a4522E871399CD717aBDD847Ab11FE88",

    // V2 addresses (QuickSwap uses Uniswap V2 architecture)
    v2_factory: "0x5757371414417b8C6CAad45bAeF941aBc7d3Ab32",
    v2_router: "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff",

    // Subgraphs
    subgraph_v3: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/3hCPRGf4z88VC5rsBKU5AA9FBBq5nF3jbKJG7VZCbhjm",
    subgraph_v2: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/QmUxgXaWYNnF6X4tZy6P1nQMhERuCKYPY9X7UJDZhPZU8V", // QuickSwap
};

/// Arbitrum One configuration
pub const ARBITRUM: ChainConfig = ChainConfig {
    chain_id: 42161,
    name: "Arbitrum",
    rpc_url: "https://arb1.arbitrum.io/rpc",

    // V3 addresses
    v3_factory: "0x1F98431c8aD98523631AE4a59f267346ea31F984",
    v3_router: "0xE592427A0AEce92De3Edee1F18E0157C05861564",
    v3_swap_router02: "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
    v3_quoter: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6",
    v3_nft_position_manager: "0xC36442b4a4522E871399CD717aBDD847Ab11FE88",

    // V2 addresses (SushiSwap uses Uniswap V2 architecture)
    v2_factory: "0xc35DADB65012eC5796536bD9864eD8773aBc74C4",
    v2_router: "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",

    // Subgraphs
    subgraph_v3: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/FbCGRftH4a3yZugY7TnbYgPJVEv2LvMT6oF1fxPe9aJM",
    subgraph_v2: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/QmWxHp6dxN9p5Q2eixfCVH2oaK2P1j9kPEJKqMz3fh7Z8V", // SushiSwap
};

/// Optimism configuration
pub const OPTIMISM: ChainConfig = ChainConfig {
    chain_id: 10,
    name: "Optimism",
    rpc_url: "https://mainnet.optimism.io",

    // V3 addresses
    v3_factory: "0x1F98431c8aD98523631AE4a59f267346ea31F984",
    v3_router: "0xE592427A0AEce92De3Edee1F18E0157C05861564",
    v3_swap_router02: "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
    v3_quoter: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6",
    v3_nft_position_manager: "0xC36442b4a4522E871399CD717aBDD847Ab11FE88",

    // V2 addresses
    v2_factory: "0x0c3c1c532F1e39EdF36BE9Fe0bE1410313E074Bf",
    v2_router: "0x4A7b5Da61326A6379179b40d00F57E5bbDC962c2",

    // Subgraphs
    subgraph_v3: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/Cghf4LfVqPiFw6fp6Y5X5Ubc8UpmUhSfJL82zwiBFLaj",
    subgraph_v2: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/QmXxHp6dxN9p5Q2eixfCVH2oaK2P1j9kPEJKqMz3fh7Z8V",
};

/// Base configuration
pub const BASE: ChainConfig = ChainConfig {
    chain_id: 8453,
    name: "Base",
    rpc_url: "https://mainnet.base.org",

    // V3 addresses
    v3_factory: "0x33128a8fC17869897dcE68Ed026d694621f6FDfD",
    v3_router: "0x2626664c2603336E57B271c5C0b26F421741e481",
    v3_swap_router02: "0x2626664c2603336E57B271c5C0b26F421741e481",
    v3_quoter: "0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a",
    v3_nft_position_manager: "0x03a520b32C04BF3bEEf7BEb72E919cf822Ed34f1",

    // V2 addresses (BaseSwap uses Uniswap V2 architecture)
    v2_factory: "0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6",
    v2_router: "0x327Df1E6de05895d2ab08513aaDD9313Fe505d86",

    // Subgraphs
    subgraph_v3: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/HCBMfwBqGT73GyoHKPjD9TFbjGQEL8bQ8wmzCkNUj8Js",
    subgraph_v2: "https://gateway.thegraph.com/api/{api_key}/subgraphs/id/QmZxHp6dxN9p5Q2eixfCVH2oaK2P1j9kPEJKqMz3fh7Z8V", // BaseSwap
};

/// Get chain config by chain ID
pub fn get_chain_config(chain_id: u64) -> Option<&'static ChainConfig> {
    match chain_id {
        1 => Some(&ETHEREUM),
        137 => Some(&POLYGON),
        42161 => Some(&ARBITRUM),
        10 => Some(&OPTIMISM),
        8453 => Some(&BASE),
        _ => None,
    }
}

/// Get chain config by name (case-insensitive)
pub fn get_chain_config_by_name(name: &str) -> Option<&'static ChainConfig> {
    match name.to_lowercase().as_str() {
        "ethereum" | "eth" | "mainnet" => Some(&ETHEREUM),
        "polygon" | "matic" => Some(&POLYGON),
        "arbitrum" | "arb" => Some(&ARBITRUM),
        "optimism" | "opt" => Some(&OPTIMISM),
        "base" => Some(&BASE),
        _ => None,
    }
}

/// Uniswap V3 fee tiers (in basis points)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum FeeTier {
    /// 0.01% fee
    Lowest = 100,
    /// 0.05% fee
    Low = 500,
    /// 0.3% fee
    Medium = 3000,
    /// 1% fee
    High = 10000,
}

impl FeeTier {
    /// Get all fee tiers
    pub fn all() -> Vec<FeeTier> {
        vec![
            FeeTier::Lowest,
            FeeTier::Low,
            FeeTier::Medium,
            FeeTier::High,
        ]
    }

    /// Get fee tier from basis points
    pub fn from_basis_points(bps: u32) -> crate::base::errors::Result<Self> {
        match bps {
            100 => Ok(FeeTier::Lowest),
            500 => Ok(FeeTier::Low),
            3000 => Ok(FeeTier::Medium),
            10000 => Ok(FeeTier::High),
            _ => Err(crate::base::errors::CcxtError::BadSymbol(format!(
                "Invalid fee tier: {}. Must be 100, 500, 3000, or 10000",
                bps
            ))),
        }
    }

    /// Get fee tier value in basis points
    pub fn as_basis_points(&self) -> u32 {
        *self as u32
    }

    /// Get fee as decimal percentage
    pub fn as_percentage(&self) -> rust_decimal::Decimal {
        use rust_decimal::Decimal;
        Decimal::from(*self as u32) / Decimal::from(10000u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_chain_config() {
        let eth = get_chain_config(1);
        assert!(eth.is_some());
        assert_eq!(eth.unwrap().name, "Ethereum");

        let unknown = get_chain_config(999999);
        assert!(unknown.is_none());
    }

    #[test]
    fn test_get_chain_config_by_name() {
        let eth = get_chain_config_by_name("ethereum");
        assert!(eth.is_some());
        assert_eq!(eth.unwrap().chain_id, 1);

        let eth2 = get_chain_config_by_name("ETHEREUM");
        assert!(eth2.is_some());

        let unknown = get_chain_config_by_name("unknown");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_fee_tier() {
        assert_eq!(FeeTier::Medium as u32, 3000);

        let fee = FeeTier::from_basis_points(3000).unwrap();
        assert_eq!(fee, FeeTier::Medium);

        assert!(FeeTier::from_basis_points(999).is_err());

        use rust_decimal::Decimal;
        use std::str::FromStr;
        let pct = FeeTier::Medium.as_percentage();
        assert_eq!(pct, Decimal::from_str("0.003").unwrap());
    }
}
