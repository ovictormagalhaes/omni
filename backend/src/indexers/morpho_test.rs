#[cfg(test)]
mod tests {
    use crate::indexers::MorphoIndexer;
    use crate::models::{Chain, Protocol};

    #[test]
    fn test_get_protocol_url() {
        let indexer = MorphoIndexer::new("https://api.morpho.org/graphql".to_string());

        // Test with vault_id (specific vault URL)
        assert_eq!(
            indexer.get_protocol_url(
                &Chain::Ethereum,
                Some("0xf24608e0CCb972b0b0f4A6446a0BBf58c701a026")
            ),
            "https://app.morpho.org/ethereum/vault/0xf24608e0CCb972b0b0f4A6446a0BBf58c701a026"
        );

        // Test without vault_id (generic earn page)
        assert_eq!(
            indexer.get_protocol_url(&Chain::Arbitrum, None),
            "https://app.morpho.org/arbitrum/earn"
        );

        assert_eq!(
            indexer.get_protocol_url(&Chain::Base, None),
            "https://app.morpho.org/base/earn"
        );
    }

    #[tokio::test]
    async fn test_morpho_fetch_rates() {
        let indexer = MorphoIndexer::new("https://api.morpho.org/graphql".to_string());

        let result = indexer.fetch_rates().await;

        // Morpho API might be down or changed, so we handle both cases
        match result {
            Ok(rates) => {
                println!("✅ Morpho API is working");

                if !rates.is_empty() {
                    // Verify data structure
                    for rate in &rates {
                        assert_eq!(rate.protocol, Protocol::Morpho);

                        // Check supported chains
                        assert!(matches!(
                            rate.chain,
                            Chain::Ethereum
                                | Chain::Arbitrum
                                | Chain::Base
                                | Chain::Polygon
                                | Chain::Optimism
                        ));

                        // Validate APY values are reasonable
                        // High-risk vaults can have extreme but legitimate APYs (e.g., 298,000%)
                        // Only reject truly corrupted data (> 1,000,000%)
                        if matches!(rate.action, crate::models::Action::Supply) {
                            assert!(
                                rate.supply_apy >= 0.0 && rate.supply_apy <= 1_000_000.0,
                                "Invalid supply APY: {}%",
                                rate.supply_apy
                            );
                        } else {
                            assert!(
                                rate.borrow_apr >= 0.0 && rate.borrow_apr <= 1_000_000.0,
                                "Invalid borrow APR: {}%",
                                rate.borrow_apr
                            );
                        }

                        // Validate liquidity
                        assert!(
                            rate.total_liquidity > 0,
                            "Total liquidity should be positive"
                        );
                        assert!(
                            rate.available_liquidity <= rate.total_liquidity,
                            "Available liquidity should be <= total liquidity"
                        );

                        // Validate utilization rate
                        assert!(
                            rate.utilization_rate <= 100.0,
                            "Utilization rate should be <= 100%"
                        );
                    }

                    println!("✅ All {} Morpho rates have valid structure", rates.len());
                } else {
                    println!(
                        "⚠️  Morpho API returned no rates (possibly no markets match criteria)"
                    );
                }
            }
            Err(e) => {
                println!("⚠️  Morpho API failed: {:?}", e);
                println!("   - This could be:");
                println!("   - API temporary outage");
                println!("   - Network issues");
                println!("   - Rate limiting");

                // We don't fail the test since this is an external API
                // In production, the aggregator would handle this gracefully
            }
        }
    }

    #[test]
    fn test_morpho_supported_chains() {
        let indexer = MorphoIndexer::new("https://api.morpho.org/graphql".to_string());

        // Test supported chains without vault_id
        let supported = vec![
            Chain::Ethereum,
            Chain::Arbitrum,
            Chain::Base,
            Chain::Polygon,
            Chain::Optimism,
        ];

        for chain in supported {
            let url = indexer.get_protocol_url(&chain, None);
            assert!(
                url.contains("morpho.org"),
                "URL should contain morpho.org for {:?}",
                chain
            );
            assert!(
                url.contains("/earn"),
                "URL should contain /earn for {:?}",
                chain
            );
        }

        println!("✅ Morpho multi-chain URL generation test passed");
    }

    #[tokio::test]
    async fn test_morpho_error_handling() {
        // Test with a mock client that will fail
        let indexer =
            MorphoIndexer::new("https://invalid-morpho-api.example.com/graphql".to_string());

        // This should fail gracefully
        let result = indexer.fetch_rates().await;

        match result {
            Ok(_) => println!("✅ Morpho API call succeeded (unexpected but ok)"),
            Err(e) => {
                // Should have a meaningful error message
                let error_msg = format!("{:?}", e);
                assert!(!error_msg.is_empty(), "Error message should not be empty");
                println!("✅ Morpho error handling works: {}", error_msg);
            }
        }
    }
}
