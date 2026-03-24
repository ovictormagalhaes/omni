use anyhow::Result;
use mongodb::{Client, bson::doc};
use std::env;

#[tokio::test]
async fn analyze_vaults_with_historical_data() -> Result<()> {
    // Connect to MongoDB
    let mongo_uri = env::var("MONGODB_URL")
        .expect("MONGODB_URL must be set to run integration tests");
    
    let client = Client::with_uri_str(&mongo_uri).await?;
    let db = client.database("omni");
    let collection = db.collection::<mongodb::bson::Document>("rate_snapshots");
    
    println!("\n📊 ANALYZING HISTORICAL DATA COVERAGE");
    println!("=====================================\n");
    
    // Aggregation pipeline to count snapshots per vault
    let pipeline = vec![
        doc! {
            "$group": {
                "_id": {
                    "vault_id": "$vault_id",
                    "protocol": "$protocol",
                    "chain": "$chain",
                    "asset": "$asset",
                    "operation_type": "$operation_type"
                },
                "snapshot_count": { "$sum": 1 },
                "min_date": { "$min": "$date" },
                "max_date": { "$max": "$date" }
            }
        },
        doc! {
            "$sort": { "snapshot_count": -1 }
        },
        doc! {
            "$limit": 50
        }
    ];
    
    let mut cursor = collection.aggregate(pipeline).await?;
    
    println!("Top 50 Vaults with Most Historical Data:");
    println!("{:<6} {:<12} {:<10} {:<8} {:<12} {:<10} {:<12} {:<12}", 
        "Rank", "Protocol", "Chain", "Asset", "Operation", "Count", "From", "To");
    println!("{}", "-".repeat(110));
    
    let mut rank = 1;
    let mut total_snapshots = 0;
    let mut vaults_with_history = 0;
    let mut protocol_stats: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    
    use futures::stream::TryStreamExt;
    while let Some(doc) = cursor.try_next().await? {
        use mongodb::bson::Bson;
        
        // Extract _id fields
        let id = doc.get_document("_id").ok();
            let protocol = id.and_then(|d: &mongodb::bson::Document| d.get_str("protocol").ok()).unwrap_or("N/A");
            let chain = id.and_then(|d: &mongodb::bson::Document| d.get_str("chain").ok()).unwrap_or("N/A");
            let asset = id.and_then(|d: &mongodb::bson::Document| d.get_str("asset").ok()).unwrap_or("N/A");
            let operation = id.and_then(|d: &mongodb::bson::Document| d.get_str("operation_type").ok()).unwrap_or("N/A");
            
            let count = doc.get_i32("snapshot_count").unwrap_or(0) as usize;
            
            // Extract dates
            let min_date = doc.get("min_date")
                .and_then(|d| {
                    if let Bson::DateTime(dt) = d {
                        Some(dt.to_chrono().format("%Y-%m-%d").to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "N/A".to_string());
                
            let max_date = doc.get("max_date")
                .and_then(|d| {
                    if let Bson::DateTime(dt) = d {
                        Some(dt.to_chrono().format("%Y-%m-%d").to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "N/A".to_string());
            
            println!("{:<6} {:<12} {:<10} {:<8} {:<12} {:<10} {:<12} {:<12}", 
                rank, protocol, chain, asset, operation, count, min_date, max_date);
            
            total_snapshots += count;
            vaults_with_history += 1;
            *protocol_stats.entry(protocol.to_string()).or_insert(0) += count;
            
            rank += 1;
    }
    
    println!("\n{}", "=".repeat(110));
    println!("\n📈 SUMMARY STATISTICS");
    println!("====================");
    println!("Total Snapshots: {}", total_snapshots);
    println!("Vaults with History: {}", vaults_with_history);
    println!("Average snapshots per vault: {:.1}", total_snapshots as f64 / vaults_with_history as f64);
    
    println!("\n🏆 Top Protocols by Historical Data:");
    let mut protocol_vec: Vec<_> = protocol_stats.iter().collect();
    protocol_vec.sort_by(|a, b| b.1.cmp(a.1));
    
    for (i, (protocol, count)) in protocol_vec.iter().enumerate().take(10) {
        println!("  {}. {:<15} {:>6} snapshots", i + 1, protocol, count);
    }
    
    // Now check vaults without history
    println!("\n\n❌ CHECKING VAULTS WITHOUT HISTORICAL DATA");
    println!("==========================================");
    
    let pipeline_zero = vec![
        doc! {
            "$group": {
                "_id": {
                    "vault_id": "$vault_id",
                    "protocol": "$protocol",
                    "chain": "$chain",
                    "asset": "$asset"
                },
                "count": { "$sum": 1 }
            }
        },
        doc! {
            "$match": { "count": 1 }
        },
        doc! {
            "$sort": { "_id.protocol": 1, "_id.chain": 1 }
        }
    ];
    
    let mut cursor_zero = collection.aggregate(pipeline_zero).await?;
    let mut count_single = 0;
    
    println!("\nVaults with only 1 snapshot (no historical data):");
    println!("{:<15} {:<10} {:<10}", "Protocol", "Chain", "Asset");
    println!("{}", "-".repeat(40));
    
    while let Some(result) = cursor_zero.try_next().await? {
        use mongodb::bson::Bson;
        let doc: mongodb::bson::Document = result;
        let id = doc.get_document("_id").ok();
        let protocol = id.and_then(|d: &mongodb::bson::Document| d.get_str("protocol").ok()).unwrap_or("N/A");
        let chain = id.and_then(|d: &mongodb::bson::Document| d.get_str("chain").ok()).unwrap_or("N/A");
        let asset = id.and_then(|d: &mongodb::bson::Document| d.get_str("asset").ok()).unwrap_or("N/A");
            
        println!("{:<15} {:<10} {:<10}", protocol, chain, asset);
        count_single += 1;
    }
    
    println!("\nTotal vaults with only current snapshot: {}", count_single);
    
    Ok(())
}
