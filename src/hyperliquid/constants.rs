//! Hyperliquid API constants

pub const MAINNET_API_URL: &str = "https://api.hyperliquid.xyz";
pub const TESTNET_API_URL: &str = "https://api.hyperliquid-testnet.xyz";

/// Hyperliquid L1 chain ID (used in EIP-712 domain for L1 action signing)
pub const MAINNET_CHAIN_ID: u64 = 1337;
pub const TESTNET_CHAIN_ID: u64 = 13371;

/// Arbitrum Sepolia chain ID (used in EIP-712 domain for user-signed actions)
pub const USER_SIGNED_CHAIN_ID: u64 = 421614;

/// EIP-712 domain name for L1 actions
pub const L1_DOMAIN_NAME: &str = "Exchange";
/// EIP-712 domain name for user-signed actions
pub const USER_DOMAIN_NAME: &str = "HyperliquidSignTransaction";
/// EIP-712 domain version
pub const DOMAIN_VERSION: &str = "1";

/// Phantom agent source for mainnet
pub const MAINNET_SOURCE: &str = "a";
/// Phantom agent source for testnet
pub const TESTNET_SOURCE: &str = "b";

/// Default rate limit: 1200 requests per minute = 20 per second
pub const DEFAULT_RATE_LIMIT_PER_SECOND: u32 = 20;

/// Default taker fee (3.5 bps)
pub const DEFAULT_TAKER_FEE: &str = "0.00035";
/// Default maker fee (1 bp)
pub const DEFAULT_MAKER_FEE: &str = "0.0001";
