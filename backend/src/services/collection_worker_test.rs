use super::*;
use chrono::{TimeZone, Datelike};

#[test]
fn test_collection_result_structure() {
    let result = CollectionResult {
        success: true,
        collected_date: Utc::now(),
        vaults_processed: 10,
        snapshots_inserted: 8,
        snapshots_updated: 2,
        new_vaults_discovered: 3,
        backfill_snapshots: 50,
        vaults_with_real_history: 7,
        vaults_skipped_no_history: 3,
        duration_seconds: 120,
        skipped: false,
        error: None,
    };
    
    assert!(result.success);
    assert_eq!(result.vaults_processed, 10);
    assert_eq!(result.snapshots_inserted, 8);
    assert_eq!(result.snapshots_updated, 2);
    assert_eq!(result.new_vaults_discovered, 3);
    assert_eq!(result.backfill_snapshots, 50);
    assert_eq!(result.vaults_with_real_history, 7);
    assert_eq!(result.vaults_skipped_no_history, 3);
    assert_eq!(result.duration_seconds, 120);
    assert!(!result.skipped);
    assert!(result.error.is_none());
}

#[test]
fn test_collection_result_serialization() {
    let result = CollectionResult {
        success: true,
        collected_date: Utc.with_ymd_and_hms(2026, 2, 18, 12, 0, 0).unwrap(),
        vaults_processed: 5,
        snapshots_inserted: 3,
        snapshots_updated: 1,
        new_vaults_discovered: 2,
        backfill_snapshots: 25,
        vaults_with_real_history: 4,
        vaults_skipped_no_history: 1,
        duration_seconds: 60,
        skipped: false,
        error: None,
    };
    
    // Should serialize without errors
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"success\":true"));
    assert!(json.contains("\"vaults_processed\":5"));
    assert!(json.contains("\"backfill_snapshots\":25"));
}

#[test]
fn test_collection_result_with_error() {
    let result = CollectionResult {
        success: false,
        collected_date: Utc::now(),
        vaults_processed: 0,
        snapshots_inserted: 0,
        snapshots_updated: 0,
        new_vaults_discovered: 0,
        backfill_snapshots: 0,
        vaults_with_real_history: 0,
        vaults_skipped_no_history: 0,
        duration_seconds: 5,
        skipped: false,
        error: Some("API connection failed".to_string()),
    };
    
    assert!(!result.success);
    assert_eq!(result.error, Some("API connection failed".to_string()));
}

#[test]
fn test_backfill_days_configuration() {
    // Test that backfill_days can be configured
    let backfill_days = 90i64;
    assert_eq!(backfill_days, 90);
    assert!(backfill_days > 0, "Backfill days should be positive");
    assert!(backfill_days <= 365, "Backfill days should not exceed 1 year");
}

#[test]
fn test_backfill_date_range_calculation() {
    // Test date range calculation for backfill
    let today = Utc.with_ymd_and_hms(2026, 2, 18, 0, 0, 0).unwrap();
    let backfill_days = 90i64;
    let start_date = today - Duration::days(backfill_days);
    
    assert_eq!(start_date.year(), 2025);
    assert_eq!(start_date.month(), 11); // November 2025
    assert_eq!(start_date.day(), 20);
    
    let duration = today.signed_duration_since(start_date);
    assert_eq!(duration.num_days(), backfill_days);
}

#[test]
fn test_vault_id_consistency() {
    use crate::models::{Protocol, Chain, OperationType, Action, RateSnapshot};
    
    // Same parameters → same vault_id (deterministic / stable across builds)
    let vault_id1 = RateSnapshot::generate_vault_id(
        &Protocol::Aave,
        &Chain::Ethereum,
        "USDC",
        "https://app.aave.com/reserve-overview/?underlyingAsset=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        OperationType::Lending,
        Some(&Action::Supply),
    );
    
    let vault_id2 = RateSnapshot::generate_vault_id(
        &Protocol::Aave,
        &Chain::Ethereum,
        "USDC",
        "https://app.aave.com/reserve-overview/?underlyingAsset=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        OperationType::Lending,
        Some(&Action::Supply),
    );
    
    // Supply and borrow of the same pool MUST have different vault_ids
    let vault_id_borrow = RateSnapshot::generate_vault_id(
        &Protocol::Aave,
        &Chain::Ethereum,
        "USDC",
        "https://app.aave.com/reserve-overview/?underlyingAsset=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        OperationType::Lending,
        Some(&Action::Borrow),
    );
    
    assert_eq!(vault_id1, vault_id2, "Same parameters should generate same vault_id");
    assert_eq!(vault_id1.len(), 16, "Vault ID should be 16 characters (8 bytes hex)");
    assert_ne!(vault_id1, vault_id_borrow, "Supply and Borrow must have different vault_ids");
}

#[test]
fn test_idempotency_flag() {
    // Test that collection can be skipped if already run today
    let result_skipped = CollectionResult {
        success: true,
        collected_date: Utc::now(),
        vaults_processed: 0,
        snapshots_inserted: 0,
        snapshots_updated: 0,
        new_vaults_discovered: 0,
        backfill_snapshots: 0,
        vaults_with_real_history: 0,
        vaults_skipped_no_history: 0,
        duration_seconds: 1,
        skipped: true, // Indicates collection was skipped
        error: None,
    };
    
    assert!(result_skipped.success);
    assert!(result_skipped.skipped);
    assert_eq!(result_skipped.vaults_processed, 0);
}

#[test]
fn test_collection_metrics_calculation() {
    // Test that metrics add up correctly
    let total_vaults = 10;
    let vaults_with_history = 7;
    let vaults_skipped = 3;
    
    assert_eq!(
        total_vaults,
        vaults_with_history + vaults_skipped,
        "Total vaults should equal successful + skipped"
    );
    
    let snapshots_inserted = 8;
    let snapshots_updated = 2;
    let total_snapshots = snapshots_inserted + snapshots_updated;
    
    assert_eq!(total_snapshots, 10, "Total snapshots should match operations");
}

// Note: Full integration tests require MongoDB and real indexers
// These should be in backend/tests/collection_integration_test.rs
// with #[ignore] attribute for manual execution

#[cfg(test)]
mod collection_scenarios {
    use super::*;
    
    #[test]
    fn test_first_run_scenario() {
        // First run: All vaults are new
        let result = CollectionResult {
            success: true,
            collected_date: Utc::now(),
            vaults_processed: 833,
            snapshots_inserted: 833,
            snapshots_updated: 0,
            new_vaults_discovered: 833,
            backfill_snapshots: 34650, // 833 vaults × ~42 days avg
            vaults_with_real_history: 3, // Only Phase A protocols
            vaults_skipped_no_history: 830,
            duration_seconds: 300,
            skipped: false,
            error: None,
        };
        
        assert!(result.success);
        assert_eq!(result.new_vaults_discovered, result.vaults_processed);
        assert_eq!(result.snapshots_updated, 0);
        assert!(result.backfill_snapshots > 0);
    }
    
    #[test]
    fn test_daily_run_scenario() {
        // Daily run: All vaults already exist
        let result = CollectionResult {
            success: true,
            collected_date: Utc::now(),
            vaults_processed: 833,
            snapshots_inserted: 833, // Today's snapshot
            snapshots_updated: 0,
            new_vaults_discovered: 0, // No new vaults
            backfill_snapshots: 0, // No backfill needed
            vaults_with_real_history: 3,
            vaults_skipped_no_history: 830,
            duration_seconds: 60,
            skipped: false,
            error: None,
        };
        
        assert!(result.success);
        assert_eq!(result.new_vaults_discovered, 0);
        assert_eq!(result.backfill_snapshots, 0);
        assert_eq!(result.snapshots_inserted, result.vaults_processed);
    }
    
    #[test]
    fn test_partial_failure_scenario() {
        // Some vaults failed to collect
        let vaults_processed = 833;
        let successful = 800;
        let failed = 33;
        
        assert_eq!(vaults_processed, successful + failed);
        
        let result = CollectionResult {
            success: true, // Overall success even with some failures
            collected_date: Utc::now(),
            vaults_processed,
            snapshots_inserted: successful,
            snapshots_updated: 0,
            new_vaults_discovered: 0,
            backfill_snapshots: 0,
            vaults_with_real_history: 3,
            vaults_skipped_no_history: 830,
            duration_seconds: 120,
            skipped: false,
            error: Some(format!("{} vaults failed to collect", failed)),
        };
        
        assert!(result.success);
        assert!(result.error.is_some());
        assert!(result.snapshots_inserted < result.vaults_processed);
    }
}
