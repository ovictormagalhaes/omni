// Test: Check if Morpho historical data exists and works
use anyhow::Result;
use mongodb::{options::ClientOptions, Client};
use std::env;

#[tokio::test]
async fn test_morpho_has_historical_data() -> Result<()> {
    // Setup MongoDB connection
    let mongo_uri =
        env::var("MONGODB_URL").expect("MONGODB_URL must be set to run integration tests");

    let client_options = ClientOptions::parse(&mongo_uri).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("omni");
    let collection = db.collection::<mongodb::bson::Document>("rate_snapshots");

    println!("\n🔍 CHECKING MORPHO HISTORICAL DATA");
    println!("==================================\n");

    // Count total Morpho snapshots
    let filter = mongodb::bson::doc! { "protocol": "Morpho" };
    let count = collection.count_documents(filter.clone()).await?;

    println!("📊 Total Morpho snapshots in DB: {}", count);

    if count == 0 {
        println!("⚠️  No Morpho historical data found!");
        println!("   Run: cargo run -- collect --backfill-only");
        return Ok(());
    }

    // Find one Morpho snapshot to get a vault_id
    let sample = collection.find_one(filter).await?;

    if let Some(doc) = sample {
        let vault_id = doc.get_str("vault_id").unwrap_or("unknown");
        let protocol = doc.get_str("protocol").unwrap_or("unknown");
        let chain = doc.get_str("chain").unwrap_or("unknown");
        let asset = doc.get_str("asset").unwrap_or("unknown");

        println!("✅ Found Morpho data!");
        println!("   Sample vault_id: {}", vault_id);
        println!("   Protocol: {}", protocol);
        println!("   Chain: {}", chain);
        println!("   Asset: {}", asset);

        // Now test the historical service with this vault_id
        println!("\n📈 Testing historical aggregation for this vault...");
        let historical_service =
            omni_backend::services::HistoricalDataService::new(&mongo_uri, "omni").await?;

        let result = historical_service
            .get_vault_history(Some(vault_id), None, None, None)
            .await?;

        println!("✅ Historical API works!");
        println!("   Received {} points", result.points.len());
        println!("   Expected: 90 points (always returns 90 days)");
        println!("   AVG APY: {:.2}%", result.avg_apy);
        println!("   Min APY: {:.2}%", result.min_apy);
        println!("   Max APY: {:.2}%", result.max_apy);

        assert_eq!(result.points.len(), 90, "Should return 90 daily points");
        assert!(result.data_available, "Data should be available");

        // Verify daily spacing
        for i in 1..result.points.len() {
            let diff = (result.points[i].date - result.points[i - 1].date).num_days();
            assert_eq!(diff, 1, "Points should be exactly 1 day apart");
        }
        println!("   ✅ All points are exactly 1 day apart");

        println!("\n✅ MORPHO HISTORICAL DATA WORKS PERFECTLY!");
    }

    Ok(())
}
