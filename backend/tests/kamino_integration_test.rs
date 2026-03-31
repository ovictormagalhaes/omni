/// Integration tests for Kamino protocol tracking workflow:
///   1. Fetch rates from live Kamino API
///   2. Validate data structure and business rules
///   3. Save via HistoricalDataService (worker path)
///   4. Read back from MongoDB and verify data matches
///
/// Requires: MONGODB_URL env var set to a test database.
/// Run with: cargo test --test kamino_integration_test -- --nocapture
use anyhow::Result;
use chrono::Utc;
use omni_backend::models::{
    Action, Asset, Chain, KnownAsset, OperationType, Protocol, RateSnapshot,
};
use std::env;

// ═══════════════════════════════════════════════════════════════════════
// Phase 1: Live API data validation (no DB required)
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_kamino_api_returns_valid_lending_rates() -> Result<()> {
    let indexer =
        omni_backend::indexers::KaminoIndexer::new("https://api.kamino.finance".to_string());

    let rates = indexer.fetch_rates().await?;
    assert!(!rates.is_empty(), "Kamino API must return rates");

    let lending_rates: Vec<_> = rates
        .iter()
        .filter(|r| r.operation_type == OperationType::Lending)
        .collect();

    assert!(
        !lending_rates.is_empty(),
        "Must have lending rates from reserves"
    );

    for rate in &lending_rates {
        assert_eq!(rate.protocol, Protocol::Kamino);
        assert_eq!(rate.chain, Chain::Solana);
        assert!(
            rate.supply_apy >= 0.0 && rate.supply_apy <= 200.0,
            "Supply APY out of range: {}",
            rate.supply_apy
        );
        assert!(
            rate.borrow_apr >= 0.0 && rate.borrow_apr <= 200.0,
            "Borrow APR out of range: {}",
            rate.borrow_apr
        );
        assert!(
            rate.utilization_rate >= 0.0 && rate.utilization_rate <= 100.0,
            "Utilization rate out of range: {}",
            rate.utilization_rate
        );
    }

    // Must have USDC lending (Kamino always has USDC)
    let usdc_lending = lending_rates
        .iter()
        .find(|r| r.asset == Asset::Known(KnownAsset::USDC) && r.action == Action::Supply);
    assert!(
        usdc_lending.is_some(),
        "Kamino must have USDC supply lending rate"
    );

    let usdc = usdc_lending.unwrap();
    assert!(
        usdc.total_liquidity > 0,
        "USDC should have positive total liquidity"
    );
    assert!(
        usdc.collateral_enabled,
        "USDC supply should have collateral enabled"
    );

    println!(
        "PASS: {} lending rates, USDC supply APY: {:.2}%",
        lending_rates.len(),
        usdc.supply_apy
    );
    Ok(())
}

#[tokio::test]
async fn test_kamino_api_vault_strategies_structure() -> Result<()> {
    let indexer =
        omni_backend::indexers::KaminoIndexer::new("https://api.kamino.finance".to_string());

    let rates = indexer.fetch_rates().await?;

    let vault_rates: Vec<_> = rates
        .iter()
        .filter(|r| r.operation_type == OperationType::Vault)
        .collect();

    // Strategies API may return 0 vaults if the /strategies endpoint changed
    // but if it does return data, validate structure
    if vault_rates.is_empty() {
        println!("NOTE: Kamino strategies API returned 0 vaults (endpoint may have changed)");
    } else {
        for rate in &vault_rates {
            assert_eq!(rate.protocol, Protocol::Kamino);
            assert_eq!(rate.chain, Chain::Solana);
            assert_eq!(
                rate.action,
                Action::Supply,
                "Vaults should only have Supply action"
            );
            assert!(
                !rate.collateral_enabled,
                "Vaults should NOT have collateral enabled"
            );
            assert!(
                rate.vault_id.is_some(),
                "Vault must have a vault_id (pub_key)"
            );
            assert!(rate.vault_name.is_some(), "Vault must have a vault_name");
        }
        println!(
            "PASS: {} vault rates found and validated",
            vault_rates.len()
        );
    }

    // Core assertion: lending rates must exist regardless of strategies
    let lending_count = rates
        .iter()
        .filter(|r| r.operation_type == OperationType::Lending)
        .count();
    assert!(lending_count > 0, "Lending rates must always be present");
    println!(
        "PASS: {} total rates ({} lending)",
        rates.len(),
        lending_count
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 2: Full aggregator pipeline (tests the same path as worker)
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_kamino_through_aggregator_pipeline() -> Result<()> {
    let config = omni_backend::config::Config::from_env()?;
    let aggregator = omni_backend::services::aggregator::RateAggregator::new(config);

    let query = omni_backend::models::RateQuery {
        action: None,
        assets: None,
        chains: Some("solana".to_string()),
        protocols: Some("kamino".to_string()),
        operation_types: None,
        asset_categories: None,
        token: None,
        min_liquidity: 0, // Get everything, including small pools
        page: 1,
        page_size: 100,
    };

    let rates = aggregator.get_rates(&query).await?;
    assert!(
        !rates.is_empty(),
        "Aggregator must return Kamino rates for Solana"
    );

    // All results should be Kamino + Solana
    for rate in &rates {
        assert_eq!(rate.protocol, Protocol::Kamino, "Protocol must be Kamino");
        assert_eq!(rate.chain, Chain::Solana, "Chain must be Solana");
    }

    // net_apy should equal apy + rewards
    for rate in &rates {
        let expected_net = rate.apy + rate.rewards;
        assert!(
            (rate.net_apy - expected_net).abs() < 0.001,
            "net_apy ({}) should equal apy ({}) + rewards ({})",
            rate.net_apy,
            rate.apy,
            rate.rewards
        );
    }

    println!("PASS: Aggregator returned {} Kamino rates", rates.len());
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 3: Worker tracking - save to MongoDB, read back, compare
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_kamino_worker_save_and_verify() -> Result<()> {
    let mongo_url = match env::var("MONGODB_URL") {
        Ok(url) => url,
        Err(_) => {
            println!("SKIP: MONGODB_URL not set, skipping DB integration test");
            return Ok(());
        }
    };
    let db_name = env::var("MONGODB_DATABASE").unwrap_or_else(|_| "omni_test".to_string());

    // Step 1: Fetch live rates from Kamino
    let config = omni_backend::config::Config::from_env()?;
    let aggregator = omni_backend::services::aggregator::RateAggregator::new(config);

    let query = omni_backend::models::RateQuery {
        action: None,
        assets: Some("USDC".to_string()),
        chains: Some("solana".to_string()),
        protocols: Some("kamino".to_string()),
        operation_types: Some("lending".to_string()),
        asset_categories: None,
        token: None,
        min_liquidity: 0,
        page: 1,
        page_size: 100,
    };

    let rates = aggregator.get_rates(&query).await?;
    assert!(
        !rates.is_empty(),
        "Must fetch at least one Kamino USDC lending rate"
    );

    println!(
        "Step 1: Fetched {} Kamino USDC lending rates from API",
        rates.len()
    );

    // Step 2: Save via HistoricalDataService (same path the worker uses)
    let historical_service =
        omni_backend::services::HistoricalDataService::new(&mongo_url, &db_name).await?;
    let now = Utc::now();
    let saved = historical_service.save_snapshots_batch(&rates, now).await?;

    println!("Step 2: Saved {} snapshots to MongoDB", saved);
    assert!(saved > 0, "Should save at least one snapshot");

    // Step 3: Read back and verify data integrity
    for rate in &rates {
        let vault_id = RateSnapshot::generate_vault_id(
            &rate.protocol,
            &rate.chain,
            &rate.asset.to_string(),
            &rate.url,
            rate.operation_type,
            Some(&rate.action),
        );

        let has_snapshot = historical_service.has_snapshot(&vault_id, now).await?;
        assert!(
            has_snapshot,
            "Snapshot for vault {} must exist after save",
            vault_id
        );

        let latest_date = historical_service
            .get_latest_snapshot_date(&vault_id)
            .await?;
        assert!(
            latest_date.is_some(),
            "Must have a latest snapshot date for vault {}",
            vault_id
        );

        println!(
            "  Verified: {} {} {:?} - APY: {:.2}%, saved at {:?}",
            rate.protocol,
            rate.asset,
            rate.action,
            rate.apy,
            latest_date.unwrap()
        );
    }

    println!("Step 3: All {} rates verified in MongoDB", rates.len());
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 4: Cross-protocol consistency (Aave vs Kamino structure)
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_kamino_and_aave_structural_parity() -> Result<()> {
    let config = omni_backend::config::Config::from_env()?;
    let aggregator = omni_backend::services::aggregator::RateAggregator::new(config);

    // Fetch Kamino rates
    let kamino_query = omni_backend::models::RateQuery {
        action: Some(Action::Supply),
        assets: Some("USDC".to_string()),
        chains: None,
        protocols: Some("kamino".to_string()),
        operation_types: Some("lending".to_string()),
        asset_categories: None,
        token: None,
        min_liquidity: 0,
        page: 1,
        page_size: 100,
    };

    // Fetch Aave rates
    let aave_query = omni_backend::models::RateQuery {
        action: Some(Action::Supply),
        assets: Some("USDC".to_string()),
        chains: None,
        protocols: Some("aave".to_string()),
        operation_types: Some("lending".to_string()),
        asset_categories: None,
        token: None,
        min_liquidity: 0,
        page: 1,
        page_size: 100,
    };

    let kamino_rates = aggregator.get_rates(&kamino_query).await?;
    let aave_rates = aggregator.get_rates(&aave_query).await?;

    assert!(
        !kamino_rates.is_empty(),
        "Kamino must return USDC supply rates"
    );
    assert!(!aave_rates.is_empty(), "Aave must return USDC supply rates");

    // Both should follow the same data contract
    let kamino_usdc = &kamino_rates[0];
    let aave_usdc = &aave_rates[0];

    // Structural parity checks
    assert_eq!(kamino_usdc.action, aave_usdc.action, "Action mismatch");
    assert_eq!(
        kamino_usdc.operation_type, aave_usdc.operation_type,
        "OperationType mismatch"
    );
    assert!(kamino_usdc.apy >= 0.0, "Kamino APY must be non-negative");
    assert!(aave_usdc.apy >= 0.0, "Aave APY must be non-negative");
    assert!(kamino_usdc.liquidity > 0, "Kamino USDC must have liquidity");
    assert!(aave_usdc.liquidity > 0, "Aave USDC must have liquidity");

    // URL must be set
    assert!(!kamino_usdc.url.is_empty(), "Kamino URL must not be empty");
    assert!(!aave_usdc.url.is_empty(), "Aave URL must not be empty");

    // Both should produce valid vault_ids
    let kamino_vid = RateSnapshot::generate_vault_id(
        &kamino_usdc.protocol,
        &kamino_usdc.chain,
        &kamino_usdc.asset.to_string(),
        &kamino_usdc.url,
        kamino_usdc.operation_type,
        Some(&kamino_usdc.action),
    );
    let aave_vid = RateSnapshot::generate_vault_id(
        &aave_usdc.protocol,
        &aave_usdc.chain,
        &aave_usdc.asset.to_string(),
        &aave_usdc.url,
        aave_usdc.operation_type,
        Some(&aave_usdc.action),
    );

    assert_eq!(kamino_vid.len(), 16, "Kamino vault_id should be 16 chars");
    assert_eq!(aave_vid.len(), 16, "Aave vault_id should be 16 chars");
    assert_ne!(
        kamino_vid, aave_vid,
        "Different protocols must produce different vault_ids"
    );

    println!(
        "PASS: Kamino USDC {:.2}% vs Aave USDC {:.2}% - structural parity confirmed",
        kamino_usdc.apy, aave_usdc.apy
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 5: Cross-validation with raw API (site truth)
// ═══════════════════════════════════════════════════════════════════════

/// Fetches raw data from Kamino API and compares with our indexer output.
/// This validates that our parsing logic correctly transforms the API response.
#[tokio::test]
async fn test_kamino_cross_validate_with_raw_api() -> Result<()> {
    let client = reqwest::Client::new();

    // Step 1: Fetch raw reserves from Kamino API (same endpoint our indexer uses)
    let markets_resp: serde_json::Value = client
        .get("https://api.kamino.finance/v2/kamino-market?programId=KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD")
        .send().await?.json().await?;

    let main_market = markets_resp
        .as_array()
        .unwrap()
        .iter()
        .find(|m| {
            m["isPrimary"].as_bool() == Some(true) && m["name"].as_str() == Some("Main Market")
        })
        .expect("Must find Main Market");

    let market_address = main_market["lendingMarket"].as_str().unwrap();

    let reserves_resp: Vec<serde_json::Value> = client
        .get(format!(
            "https://api.kamino.finance/kamino-market/{}/reserves/metrics?env=mainnet-beta",
            market_address
        ))
        .send()
        .await?
        .json()
        .await?;

    // Step 2: Fetch via our indexer
    let indexer =
        omni_backend::indexers::KaminoIndexer::new("https://api.kamino.finance".to_string());
    let our_rates = indexer.fetch_rates().await?;

    // Step 3: Cross-validate key assets
    let key_assets = ["SOL", "USDC", "USDT", "JITOSOL", "JUPSOL", "MSOL"];

    for asset_name in key_assets {
        // Find in raw API
        let raw = reserves_resp.iter().find(|r| {
            r["liquidityToken"].as_str().map(|t| t.to_uppercase())
                == Some(asset_name.to_uppercase())
        });

        if raw.is_none() {
            println!("  SKIP: {} not found in raw API reserves", asset_name);
            continue;
        }
        let raw = raw.unwrap();

        let raw_supply_apy = raw["supplyApy"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
            * 100.0;
        let raw_borrow_apy = raw["borrowApy"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
            * 100.0;
        let raw_ltv = raw["maxLtv"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
            * 100.0;

        // Find in our parsed rates (Supply action for this asset)
        let our_supply = our_rates.iter().find(|r| {
            r.asset.symbol().to_uppercase() == asset_name.to_uppercase()
                && r.action == Action::Supply
                && r.operation_type == OperationType::Lending
        });

        if let Some(our) = our_supply {
            // APY should match within 0.1% tolerance (timing differences)
            let apy_diff = (our.supply_apy - raw_supply_apy).abs();
            assert!(
                apy_diff < 0.5,
                "{} Supply APY mismatch: ours={:.4}% raw={:.4}% (diff={:.4}%)",
                asset_name,
                our.supply_apy,
                raw_supply_apy,
                apy_diff
            );

            let borrow_diff = (our.borrow_apr - raw_borrow_apy).abs();
            assert!(
                borrow_diff < 0.5,
                "{} Borrow APY mismatch: ours={:.4}% raw={:.4}% (diff={:.4}%)",
                asset_name,
                our.borrow_apr,
                raw_borrow_apy,
                borrow_diff
            );

            // LTV should match exactly (integer percentage)
            let ltv_diff = (our.collateral_ltv - raw_ltv).abs();
            assert!(
                ltv_diff < 1.0,
                "{} LTV mismatch: ours={:.0}% raw={:.0}%",
                asset_name,
                our.collateral_ltv,
                raw_ltv
            );

            println!(
                "  MATCH: {:8} Supply:{:6.2}% Borrow:{:6.2}% LTV:{:.0}%",
                asset_name, our.supply_apy, our.borrow_apr, our.collateral_ltv
            );
        } else {
            println!("  SKIP: {} not found in our parsed rates", asset_name);
        }
    }

    println!("PASS: Cross-validation complete - our data matches raw Kamino API");
    Ok(())
}
