#[cfg(test)]
mod tests {
    use super::super::aave::*;
    use crate::models::{Asset, Chain, KnownAsset, Protocol};

    #[test]
    fn test_get_protocol_url() {
        let indexer = AaveIndexer::new("".to_string(), "".to_string());

        // Test with underlying asset (specific reserve URL)
        assert_eq!(
            indexer.get_protocol_url(&Chain::Arbitrum, Some("0xaf88d065e77c8cC2239327C5EDb3A432268e5831")),
            "https://app.aave.com/reserve-overview/?underlyingAsset=0xaf88d065e77c8cc2239327c5edb3a432268e5831&marketName=proto_arbitrum_v3"
        );

        // Test without underlying asset (generic market URL)
        assert_eq!(
            indexer.get_protocol_url(&Chain::Base, None),
            "https://app.aave.com/?marketName=proto_base_v3"
        );

        // Test unsupported chain
        assert_eq!(indexer.get_protocol_url(&Chain::Solana, None), "");
    }

    #[tokio::test]
    async fn test_aave_fetch_rates_arbitrum() {
        let indexer = AaveIndexer::new("".to_string(), "".to_string());

        let result = indexer.fetch_rates(&Chain::Arbitrum).await;

        assert!(result.is_ok(), "Failed to fetch Aave rates for Arbitrum");
        let rates = result.unwrap();

        // Should have some rates
        assert!(!rates.is_empty(), "No rates returned for Arbitrum");

        // Verify data structure
        for rate in &rates {
            assert_eq!(rate.protocol, Protocol::Aave);
            assert_eq!(rate.chain, Chain::Arbitrum);

            // Validate APY values are reasonable (0-100%)
            assert!(
                rate.supply_apy >= 0.0 && rate.supply_apy <= 100.0,
                "Invalid supply APY: {}",
                rate.supply_apy
            );
            assert!(
                rate.borrow_apr >= 0.0 && rate.borrow_apr <= 100.0,
                "Invalid borrow APR: {}",
                rate.borrow_apr
            );

            // Validate utilization rate (0-100%)
            assert!(
                rate.utilization_rate >= 0.0 && rate.utilization_rate <= 100.0,
                "Invalid utilization rate: {}",
                rate.utilization_rate
            );

            // Validate liquidity values are positive
            assert!(
                rate.total_liquidity > 0,
                "Total liquidity should be positive, got: {}",
                rate.total_liquidity
            );

            // Available liquidity should be <= total liquidity
            assert!(
                rate.available_liquidity <= rate.total_liquidity,
                "Available liquidity ({}) exceeds total liquidity ({})",
                rate.available_liquidity,
                rate.total_liquidity
            );

            // LTV should be reasonable (0-1)
            assert!(
                rate.ltv >= 0.0 && rate.ltv <= 1.0,
                "Invalid LTV: {}",
                rate.ltv
            );
        }

        // Check for expected assets
        let assets: Vec<Asset> = rates.iter().map(|r| r.asset.clone()).collect();
        let has_usdc = assets
            .iter()
            .any(|a| matches!(a, Asset::Known(KnownAsset::USDC)));
        let has_eth = assets
            .iter()
            .any(|a| matches!(a, Asset::Known(KnownAsset::ETH)));

        assert!(
            has_usdc || has_eth,
            "Should have at least USDC or ETH in results"
        );

        println!("✅ Aave Arbitrum test passed with {} rates", rates.len());
        for rate in &rates {
            println!(
                "  {} - Supply: {:.2}% | Borrow: {:.2}% | Liquidity: ${} / ${} | Util: {:.0}%",
                rate.asset,
                rate.supply_apy,
                rate.borrow_apr,
                rate.available_liquidity,
                rate.total_liquidity,
                rate.utilization_rate
            );
        }
    }

    #[tokio::test]
    async fn test_aave_fetch_rates_base() {
        let indexer = AaveIndexer::new("".to_string(), "".to_string());

        let result = indexer.fetch_rates(&Chain::Base).await;

        assert!(result.is_ok(), "Failed to fetch Aave rates for Base");
        let rates = result.unwrap();

        // Should have some rates
        assert!(!rates.is_empty(), "No rates returned for Base");

        // Verify all rates are for Base chain
        for rate in &rates {
            assert_eq!(rate.chain, Chain::Base);
            assert_eq!(rate.protocol, Protocol::Aave);

            // Validate liquidity data is present
            assert!(
                rate.total_liquidity > 0,
                "Total liquidity missing for {}",
                rate.asset
            );
        }

        println!("✅ Aave Base test passed with {} rates", rates.len());
        for rate in &rates {
            println!(
                "  {} - Supply: {:.2}% | Liquidity: ${}",
                rate.asset, rate.supply_apy, rate.total_liquidity
            );
        }
    }

    #[tokio::test]
    async fn test_aave_solana_returns_empty() {
        let indexer = AaveIndexer::new("".to_string(), "".to_string());

        let result = indexer.fetch_rates(&Chain::Solana).await;

        assert!(result.is_ok(), "Should return Ok for Solana");
        let rates = result.unwrap();

        assert!(rates.is_empty(), "Should return empty vec for Solana");

        println!("✅ Aave Solana returns empty as expected");
    }

    #[tokio::test]
    async fn test_aave_liquidity_calculation() {
        let indexer = AaveIndexer::new("".to_string(), "".to_string());

        let rates = indexer.fetch_rates(&Chain::Arbitrum).await.unwrap();

        for rate in &rates {
            // Calculate expected available liquidity
            let utilization_decimal = rate.utilization_rate / 100.0;
            let expected_available =
                (rate.total_liquidity as f64 * (1.0 - utilization_decimal)).round() as u64;

            // Allow small rounding differences (within 1%)
            let diff_percent = if expected_available > 0 {
                ((rate.available_liquidity as f64 - expected_available as f64).abs()
                    / expected_available as f64)
                    * 100.0
            } else {
                0.0
            };

            assert!(diff_percent < 1.0,
                "Liquidity calculation mismatch for {}: available={}, total={}, util={:.2}%, expected={}, diff={:.2}%",
                rate.asset, rate.available_liquidity, rate.total_liquidity,
                rate.utilization_rate, expected_available, diff_percent);
        }

        println!("✅ Aave liquidity calculations are correct");
    }
}
