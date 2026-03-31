#[cfg(test)]
mod tests {
    use super::super::kamino::*;
    use crate::models::{Action, Asset, Chain, KnownAsset, OperationType, Protocol};

    // ─── URL construction tests ───────────────────────────────────────

    #[test]
    fn test_base_url_must_not_contain_path() {
        // The KAMINO_API_URL must be a base URL (no trailing path).
        // The indexer appends /v2/kamino-market, /kamino-market/... and /v2/strategies itself.
        let base = "https://api.kamino.finance";
        let indexer = KaminoIndexer::new(base.to_string());

        // Verify internal api_url does not duplicate paths
        assert_eq!(
            indexer.api_url, base,
            "api_url should be the exact base URL"
        );
        assert!(
            !indexer.api_url.ends_with('/'),
            "api_url should not have trailing slash, got: {}",
            indexer.api_url
        );
    }

    #[test]
    fn test_get_protocol_url() {
        let indexer = KaminoIndexer::new("https://api.kamino.finance".to_string());
        assert_eq!(
            indexer.get_protocol_url(),
            "https://app.kamino.finance/lending"
        );
    }

    // ─── Asset identification tests ───────────────────────────────────

    #[test]
    fn test_identify_known_solana_mints() {
        let indexer = KaminoIndexer::new("https://api.kamino.finance".to_string());

        assert_eq!(
            indexer.identify_asset_from_mint("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
            Asset::Known(KnownAsset::USDC)
        );
        assert_eq!(
            indexer.identify_asset_from_mint("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"),
            Asset::Known(KnownAsset::USDT)
        );
        assert_eq!(
            indexer.identify_asset_from_mint("So11111111111111111111111111111111111111112"),
            Asset::Known(KnownAsset::SOL)
        );
        assert_eq!(
            indexer.identify_asset_from_mint("mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So"),
            Asset::Known(KnownAsset::SOL)
        );
        assert_eq!(
            indexer.identify_asset_from_mint("7dHbWXmci3dT8UFYWYZweBLXgycu7Y3iL6trKn1Y7ARj"),
            Asset::Known(KnownAsset::SOL)
        );
    }

    #[test]
    fn test_identify_unknown_mint_falls_back_to_unknown_asset() {
        let indexer = KaminoIndexer::new("https://api.kamino.finance".to_string());
        let asset = indexer.identify_asset_from_mint("SomeRandomMintAddress123");
        assert!(
            matches!(asset, Asset::Unknown(_)),
            "Unknown mint should produce Asset::Unknown, got: {:?}",
            asset
        );
    }

    // ─── Reserve parsing tests ────────────────────────────────────────

    #[test]
    fn test_parse_reserves_produces_supply_and_borrow() {
        let indexer = KaminoIndexer::new("https://api.kamino.finance".to_string());

        let reserves = vec![ReserveMetrics {
            reserve: "reserve_pubkey_1".to_string(),
            liquidity_token: "USDC".to_string(),
            supply_apy: 0.05, // 5%
            borrow_apy: 0.08, // 8%
            total_supply: 1_000_000.0,
            total_borrow: 600_000.0,
            max_ltv: 0.75,
        }];

        let rates = indexer.parse_reserves(reserves);

        assert_eq!(
            rates.len(),
            2,
            "Each reserve should produce Supply + Borrow"
        );

        // Supply rate
        let supply = rates.iter().find(|r| r.action == Action::Supply).unwrap();
        assert_eq!(supply.protocol, Protocol::Kamino);
        assert_eq!(supply.chain, Chain::Solana);
        assert_eq!(supply.asset, Asset::Known(KnownAsset::USDC));
        assert!(
            (supply.supply_apy - 5.0).abs() < 0.01,
            "Supply APY should be 5%, got {}",
            supply.supply_apy
        );
        assert!(
            (supply.borrow_apr - 8.0).abs() < 0.01,
            "Borrow APR should be 8%, got {}",
            supply.borrow_apr
        );
        assert!(
            supply.collateral_enabled,
            "Supply should have collateral enabled"
        );
        assert!((supply.collateral_ltv - 75.0).abs() < 0.01);
        assert_eq!(supply.operation_type, OperationType::Lending);
        assert_eq!(supply.available_liquidity, 400_000); // supply - borrow
        assert_eq!(supply.total_liquidity, 1_000_000);
        let expected_util = (600_000.0 / 1_000_000.0) * 100.0;
        assert!((supply.utilization_rate - expected_util).abs() < 0.01);

        // Borrow rate
        let borrow = rates.iter().find(|r| r.action == Action::Borrow).unwrap();
        assert!(
            !borrow.collateral_enabled,
            "Borrow should NOT have collateral enabled"
        );
        assert_eq!(borrow.operation_type, OperationType::Lending);
    }

    #[test]
    fn test_parse_reserves_zero_supply_no_panic() {
        let indexer = KaminoIndexer::new("https://api.kamino.finance".to_string());

        let reserves = vec![ReserveMetrics {
            reserve: "empty_reserve".to_string(),
            liquidity_token: "SOL".to_string(),
            supply_apy: 0.0,
            borrow_apy: 0.0,
            total_supply: 0.0,
            total_borrow: 0.0,
            max_ltv: 0.5,
        }];

        let rates = indexer.parse_reserves(reserves);
        assert_eq!(rates.len(), 2);
        let supply = &rates[0];
        assert_eq!(supply.utilization_rate, 0.0);
    }

    // ─── Live API integration test ────────────────────────────────────

    #[tokio::test]
    async fn test_kamino_live_api_returns_valid_rates() {
        let indexer = KaminoIndexer::new("https://api.kamino.finance".to_string());

        let result = indexer.fetch_rates().await;
        assert!(
            result.is_ok(),
            "Kamino API should return Ok, got: {:?}",
            result.err()
        );

        let rates = result.unwrap();
        assert!(!rates.is_empty(), "Kamino should return at least 1 rate");

        // All rates must be Kamino + Solana
        for rate in &rates {
            assert_eq!(rate.protocol, Protocol::Kamino);
            assert_eq!(rate.chain, Chain::Solana);
            assert!(
                rate.supply_apy >= 0.0,
                "APY must be non-negative: {}",
                rate.supply_apy
            );
            assert!(
                rate.borrow_apr >= 0.0,
                "APR must be non-negative: {}",
                rate.borrow_apr
            );
        }

        // Must contain at least one known Solana asset (USDC, USDT, or SOL)
        let has_known_asset = rates.iter().any(|r| {
            matches!(
                r.asset,
                Asset::Known(KnownAsset::USDC)
                    | Asset::Known(KnownAsset::USDT)
                    | Asset::Known(KnownAsset::SOL)
            )
        });
        assert!(has_known_asset, "Must have at least one of USDC/USDT/SOL");

        // Must contain Lending rates from reserves
        let has_lending = rates
            .iter()
            .any(|r| r.operation_type == OperationType::Lending);
        assert!(has_lending, "Should have Lending rates from reserves");

        // Vault rates from strategies are optional (API may not return APY data)
        let vault_count = rates
            .iter()
            .filter(|r| r.operation_type == OperationType::Vault)
            .count();
        println!(
            "Vault rates: {} (strategies endpoint may have changed)",
            vault_count
        );

        // Lending rates should have both Supply and Borrow
        let lending_rates: Vec<_> = rates
            .iter()
            .filter(|r| r.operation_type == OperationType::Lending)
            .collect();
        let has_supply = lending_rates.iter().any(|r| r.action == Action::Supply);
        let has_borrow = lending_rates.iter().any(|r| r.action == Action::Borrow);
        assert!(has_supply, "Lending should include Supply rates");
        assert!(has_borrow, "Lending should include Borrow rates");

        println!(
            "Kamino returned {} rates ({} lending, {} vault)",
            rates.len(),
            lending_rates.len(),
            rates
                .iter()
                .filter(|r| r.operation_type == OperationType::Vault)
                .count()
        );
    }

    // ─── Data consistency test ────────────────────────────────────────

    #[tokio::test]
    async fn test_kamino_data_consistency_across_fetches() {
        // Fetch twice and verify structural consistency
        let indexer = KaminoIndexer::new("https://api.kamino.finance".to_string());

        let result1 = indexer.fetch_rates().await;
        assert!(result1.is_ok(), "First fetch failed: {:?}", result1.err());

        let result2 = indexer.fetch_rates().await;
        assert!(result2.is_ok(), "Second fetch failed: {:?}", result2.err());

        let rates1 = result1.unwrap();
        let rates2 = result2.unwrap();

        // Same number of rates (API is deterministic for same state)
        assert_eq!(
            rates1.len(),
            rates2.len(),
            "Two consecutive fetches should return the same number of rates"
        );

        // Same set of assets
        let mut assets1: Vec<String> = rates1
            .iter()
            .map(|r| format!("{}-{:?}", r.asset, r.action))
            .collect();
        let mut assets2: Vec<String> = rates2
            .iter()
            .map(|r| format!("{}-{:?}", r.asset, r.action))
            .collect();
        assets1.sort();
        assets2.sort();
        assert_eq!(assets1, assets2, "Asset sets should match across fetches");
    }

    #[tokio::test]
    async fn test_kamino_error_on_bad_url() {
        let indexer = KaminoIndexer::new("https://invalid.example.com".to_string());
        let result = indexer.fetch_rates().await;
        assert!(
            result.is_err(),
            "Bad URL should produce an error, not silently succeed"
        );
    }
}
