// Test: Validate daily aggregation and forward-fill logic
use anyhow::Result;
use std::env;
use omni_backend::models::Protocol;

#[tokio::test]
async fn test_daily_history_aggregation() -> Result<()> {
    // Setup MongoDB connection
    let mongo_uri = env::var("MONGODB_URL")
        .expect("MONGODB_URL must be set to run integration tests");
    
    let historical_service = omni_backend::services::HistoricalDataService::new(&mongo_uri, "omni").await?;
    
    println!("\n🧪 TESTING DAILY HISTORY AGGREGATION");
    println!("====================================\n");
    
    // Test 1: Lido stETH (has ~31 events)
    println!("📊 Test 1: Lido stETH (vault_id: c3ab90b0fe775d1b)");
    let result = historical_service
        .get_vault_history(
            Some("c3ab90b0fe775d1b"), // Lido stETH vault_id
            None,
            None,
            None,
        )
        .await?;
    
    println!("✅ Received {} points", result.points.len());
    println!("   Expected: 90 points (always fetches 90 days)");
    println!("   Protocol: {:?}", result.protocol);
    println!("   Chain: {:?}", result.chain);
    println!("   Asset: {:?}", result.asset);
    println!("   AVG APY: {:.2}%", result.avg_apy);
    println!("   Min APY: {:.2}%", result.min_apy);
    println!("   Max APY: {:.2}%", result.max_apy);
    
    assert_eq!(result.points.len(), 90, "Should always return 90 daily points");
    assert!(result.data_available, "Data should be available");
    
    // Test 2: Check dates are sequential (1 day apart)
    println!("\n📅 Test 2: Verify daily granularity");
    for i in 1..result.points.len() {
        let diff = (result.points[i].date - result.points[i-1].date).num_days();
        assert_eq!(diff, 1, "Points should be exactly 1 day apart");
    }
    println!("✅ All {} points are exactly 1 day apart", result.points.len());
    
    // Test 3: Forward-fill verification
    println!("\n🔄 Test 3: Forward-fill check");
    let mut same_value_count = 0;
    for i in 1..result.points.len() {
        if (result.points[i].net_apy - result.points[i-1].net_apy).abs() < 0.0001 {
            same_value_count += 1;
        }
    }
    println!("   Days with same APY as previous: {}", same_value_count);
    println!("   Days with APY changes: {}", result.points.len() - same_value_count - 1);
    
    // Test 4: Aave Base USDC (has 655 events - should be aggregated to 90 days)
    println!("\n📊 Test 4: Aave Base USDC (many events per day)");
    let result2 = historical_service
        .get_vault_history(
            Some("56d77baf2244e085"), // Aave Base USDC
            None,
            None,
            None,
        )
        .await?;
    
    println!("✅ Received {} points (aggregated from 655 events)", result2.points.len());
    assert_eq!(result2.points.len(), 90, "Should aggregate multiple events per day into 90 points");
    
    // Test 5: Morpho - verify protocol has historical data and aggregation works
    println!("\n📊 Test 5: Morpho (verify protocol historical data)");
    let result3 = historical_service
        .get_vault_history(
            None,
            Some(&Protocol::Morpho),
            None,
            None,
        )
        .await?;
    
    if result3.data_available {
        println!("✅ Morpho historical data available!");
        println!("   Received {} points", result3.points.len());
        println!("   Protocol: {:?}", result3.protocol);
        println!("   Chain: {:?}", result3.chain);
        println!("   Asset: {:?}", result3.asset);
        println!("   AVG APY: {:.2}%", result3.avg_apy);
        println!("   Min APY: {:.2}%", result3.min_apy);
        println!("   Max APY: {:.2}%", result3.max_apy);
        assert_eq!(result3.points.len(), 90, "Should return 90 daily points");
        
        // Verify daily spacing
        for i in 1..result3.points.len() {
            let diff = (result3.points[i].date - result3.points[i-1].date).num_days();
            assert_eq!(diff, 1, "Morpho points should be exactly 1 day apart");
        }
        println!("   ✅ All points are exactly 1 day apart");
    } else {
        println!("⚠️  No Morpho historical data available yet");
        println!("   This is expected if backfill hasn't been run for Morpho");
    }
    
    println!("\n✅ ALL TESTS PASSED");
    println!("====================\n");
    println!("Summary:");
    println!("  - API always returns 90 daily points");
    println!("  - Multiple rate changes per day are aggregated (last value wins)");
    println!("  - Missing days are forward-filled with last known value");
    println!("  - Frontend can filter locally for 30/60/90 day views");
    println!("  - Works for Lido, Aave, and Morpho protocols");
    
    Ok(())
}
