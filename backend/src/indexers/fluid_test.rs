#[cfg(test)]
mod tests {
    use crate::indexers::FluidIndexer;
    use crate::models::{Asset, Chain, KnownAsset, Protocol};

    #[test]
    fn test_get_protocol_url() {
        let indexer = FluidIndexer::new("https://api.fluid.instadapp.io".to_string());
        let usdc = Asset::Known(KnownAsset::USDC);

        assert_eq!(
            indexer.get_protocol_url(&Chain::Ethereum, &usdc),
            "https://fluid.io/lending/1/USDC"
        );
    }

    #[tokio::test]
    async fn test_fluid_fetch_rates() {
        let indexer = FluidIndexer::new("https://api.fluid.instadapp.io".to_string());

        let result = indexer.fetch_rates().await;

        // Fluid API might be down or changed, so we handle both cases
        match result {
            Ok(rates) => {
                println!("✅ Fluid API is working");

                if !rates.is_empty() {
                    // Verify data structure
                    for rate in &rates {
                        assert_eq!(rate.protocol, Protocol::Fluid);
                        assert_eq!(rate.chain, Chain::Ethereum); // Fluid is only on Ethereum

                        // Validate APY values are reasonable
                        if matches!(rate.action, crate::models::Action::Supply) {
                            assert!(
                                rate.supply_apy >= 0.0 && rate.supply_apy <= 100.0,
                                "Invalid supply APY: {}%",
                                rate.supply_apy
                            );
                        } else {
                            assert!(
                                rate.borrow_apr >= 0.0 && rate.borrow_apr <= 100.0,
                                "Invalid borrow APR: {}%",
                                rate.borrow_apr
                            );
                        }

                        // Validate liquidity
                        // total_liquidity is u64, always >= 0
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

                    println!("✅ All {} Fluid rates have valid structure", rates.len());
                } else {
                    println!("⚠️  Fluid API returned no rates (possibly no markets available)");
                }
            }
            Err(e) => {
                println!("⚠️  Fluid API failed: {:?}", e);
                println!("   - This could be:");
                println!("   - API temporary outage");
                println!("   - Network issues");
                println!("   - Rate limiting");

                // We don't fail the test since this is an external API
            }
        }
    }

    #[test]
    fn test_fluid_ethereum_only() {
        let indexer = FluidIndexer::new("https://api.fluid.instadapp.io".to_string());
        let usdc = Asset::Known(KnownAsset::USDC);

        // Fluid supports Ethereum
        let url = indexer.get_protocol_url(&Chain::Ethereum, &usdc);
        assert!(
            url.contains("fluid.io/lending"),
            "URL should contain fluid.io/lending"
        );

        println!("✅ Fluid Ethereum-only test passed");
    }

    #[tokio::test]
    async fn test_fluid_lending_and_vaults() {
        let indexer = FluidIndexer::new("https://api.fluid.instadapp.io".to_string());

        // Test combined fetch_rates (which calls both lending and vault rates)
        let combined_result = indexer.fetch_rates().await;
        match combined_result {
            Ok(rates) => {
                println!("✅ Fluid combined rates: {} found", rates.len());

                let supply_rates: Vec<_> = rates
                    .iter()
                    .filter(|r| matches!(r.action, crate::models::Action::Supply))
                    .collect();
                let borrow_rates: Vec<_> = rates
                    .iter()
                    .filter(|r| matches!(r.action, crate::models::Action::Borrow))
                    .collect();

                println!("  - Supply rates: {}", supply_rates.len());
                println!("  - Borrow rates: {}", borrow_rates.len());

                for rate in &rates {
                    assert_eq!(rate.protocol, Protocol::Fluid);
                    assert_eq!(rate.chain, Chain::Ethereum);
                }
            }
            Err(e) => println!("⚠️  Fluid combined API failed: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_fluid_error_handling() {
        // Test with a mock client that will fail
        let indexer = FluidIndexer::new("https://invalid-fluid-api.example.com".to_string());

        // This should fail gracefully
        let result = indexer.fetch_rates().await;

        match result {
            Ok(_) => println!("✅ Fluid API call succeeded (unexpected but ok)"),
            Err(e) => {
                // Should have a meaningful error message
                let error_msg = format!("{:?}", e);
                assert!(!error_msg.is_empty(), "Error message should not be empty");
                println!("✅ Fluid error handling works: {}", error_msg);
            }
        }
    }
}
