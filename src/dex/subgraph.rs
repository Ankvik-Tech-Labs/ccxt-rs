//! GraphQL subgraph client for DEX data

use crate::base::errors::{CcxtError, Result};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;

/// GraphQL subgraph client
pub struct SubgraphClient {
    client: Client,
    endpoint: String,
}

impl SubgraphClient {
    /// Create a new subgraph client
    ///
    /// # Arguments
    /// * `endpoint` - GraphQL endpoint URL
    pub fn new(endpoint: String) -> Self {
        Self {
            client: Client::new(),
            endpoint,
        }
    }

    /// Execute a GraphQL query
    ///
    /// # Arguments
    /// * `query` - GraphQL query string
    /// * `variables` - Optional query variables
    pub async fn query(
        &self,
        query: &str,
        variables: Option<HashMap<String, Value>>,
    ) -> Result<Value> {
        let mut body = HashMap::new();
        body.insert("query", serde_json::json!(query));

        if let Some(vars) = variables {
            body.insert("variables", serde_json::json!(vars));
        }

        let response = self
            .client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| CcxtError::NetworkError(format!("Subgraph request failed: {}", e)))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| CcxtError::ParseError(format!("Failed to parse subgraph response: {}", e)))?;

        // Check for GraphQL errors
        if let Some(errors) = json.get("errors") {
            return Err(CcxtError::BadRequest(format!(
                "GraphQL errors: {}",
                errors
            )));
        }

        json.get("data")
            .cloned()
            .ok_or_else(|| CcxtError::ParseError("No data in subgraph response".to_string()))
    }

    /// Fetch pool data from Uniswap V3 subgraph
    pub async fn fetch_uniswap_v3_pool(&self, pool_address: &str) -> Result<Value> {
        let query = r#"
            query GetPool($poolAddress: String!) {
                pool(id: $poolAddress) {
                    id
                    token0 {
                        id
                        symbol
                        decimals
                    }
                    token1 {
                        id
                        symbol
                        decimals
                    }
                    token0Price
                    token1Price
                    liquidity
                    volumeUSD
                    feeTier
                }
            }
        "#;

        let mut variables = HashMap::new();
        variables.insert("poolAddress".to_string(), serde_json::json!(pool_address.to_lowercase()));

        self.query(query, Some(variables)).await
    }

    /// Fetch recent swaps from subgraph
    pub async fn fetch_recent_swaps(
        &self,
        pool_address: &str,
        limit: u32,
    ) -> Result<Value> {
        let query = r#"
            query GetSwaps($poolAddress: String!, $limit: Int!) {
                swaps(
                    first: $limit
                    where: { pool: $poolAddress }
                    orderBy: timestamp
                    orderDirection: desc
                ) {
                    id
                    timestamp
                    amount0
                    amount1
                    amountUSD
                    sqrtPriceX96
                    tick
                }
            }
        "#;

        let mut variables = HashMap::new();
        variables.insert("poolAddress".to_string(), serde_json::json!(pool_address.to_lowercase()));
        variables.insert("limit".to_string(), serde_json::json!(limit));

        self.query(query, Some(variables)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_subgraph_query() {
        // Example: Uniswap V3 Ethereum mainnet subgraph
        let client = SubgraphClient::new(
            "https://api.thegraph.com/subgraphs/name/uniswap/uniswap-v3".to_string(),
        );

        let query = r#"
            {
                pools(first: 1) {
                    id
                }
            }
        "#;

        let result = client.query(query, None).await;
        assert!(result.is_ok());
    }
}
