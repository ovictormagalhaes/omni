use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize)]
struct GraphQLQuery {
    query: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<ReserveDataResponse>,
}

#[derive(Debug, Deserialize)]
struct ReserveDataResponse {
    #[serde(rename = "reserveParamsHistoryItems")]
    reserve_params_history_items: Vec<HistoricalPoint>,
}

#[derive(Debug, Deserialize)]
struct HistoricalPoint {
    timestamp: String,
    #[serde(rename = "liquidityRate")]
    liquidity_rate: String,
    #[serde(rename = "availableLiquidity")]
    available_liquidity: String,
    #[serde(rename = "totalLiquidity")]
    total_liquidity: String,
    #[serde(rename = "utilizationRate")]
    utilization_rate: String,
}

#[tokio::test]
async fn test_aave_base_usdc_has_historical_data() -> Result<()> {
    // USDC address on Base
    let usdc_address = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
    
    // Get API key from env
    let api_key = env::var("THE_GRAPH_API_KEY")
        .unwrap_or_else(|_| "13124d1f498df89160c42e0b10d17b8f".to_string());
    
    // Aave Base subgraph
    let subgraph_id = "GQFbb95cE6d8mV989mL5figjaGaKCQB3xqYrr1bRyXqF";
    let url = format!("https://gateway-arbitrum.network.thegraph.com/api/{}/subgraphs/id/{}", 
        api_key, subgraph_id);
    
    // Calculate timestamps for last 30 days
    let now = chrono::Utc::now().timestamp();
    let thirty_days_ago = now - (30 * 24 * 60 * 60);
    
    // GraphQL query to fetch historical data
    let query = format!(r#"
        {{
            reserveParamsHistoryItems(
                first: 1000,
                orderBy: timestamp,
                orderDirection: desc,
                where: {{
                    reserve: "{}",
                    timestamp_gte: {}
                }}
            ) {{
                timestamp
                liquidityRate
                availableLiquidity
                totalLiquidity
                utilizationRate
            }}
        }}
    "#, usdc_address.to_lowercase(), thirty_days_ago);
    
    println!("\n🔍 Testing Aave Base USDC Historical Data");
    println!("================================================");
    println!("USDC Address: {}", usdc_address);
    println!("Subgraph URL: {}", url);
    println!("Query period: Last 30 days");
    println!("");
    
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&GraphQLQuery { query })
        .send()
        .await?;
    
    let status = response.status();
    println!("Response Status: {}", status);
    
    let response_text = response.text().await?;
    println!("Response (first 500 chars): {}", &response_text[..response_text.len().min(500)]);
    
    let response_data: GraphQLResponse = serde_json::from_str(&response_text)?;
    
    if let Some(data) = response_data.data {
        let points = data.reserve_params_history_items;
        println!("\n✅ SUCCESS: Found {} historical data points", points.len());
        
        // Assert we have more than 1 point
        assert!(points.len() > 1, "Expected more than 1 historical point, got {}", points.len());
        
        // Show first 5 points
        println!("\nFirst 5 data points:");
        for (i, point) in points.iter().take(5).enumerate() {
            let timestamp = point.timestamp.parse::<i64>().unwrap_or(0);
            let date = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Invalid".to_string());
            
            println!("  {}. {}", i + 1, date);
            println!("     Liquidity Rate: {}", point.liquidity_rate);
            println!("     Available Liquidity: {}", point.available_liquidity);
            println!("     Utilization: {}%", point.utilization_rate);
        }
        
        // Group by day to see unique days
        use std::collections::HashSet;
        let mut unique_days = HashSet::new();
        for point in &points {
            if let Ok(timestamp) = point.timestamp.parse::<i64>() {
                if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0) {
                    let day = dt.format("%Y-%m-%d").to_string();
                    unique_days.insert(day);
                }
            }
        }
        
        println!("\n📊 Statistics:");
        println!("  Total data points: {}", points.len());
        println!("  Unique days: {}", unique_days.len());
        println!("  Points per day (avg): {:.1}", points.len() as f64 / unique_days.len() as f64);
        
        // This proves the subgraph HAS historical data
        assert!(unique_days.len() >= 20, 
            "Expected at least 20 unique days of data, got {}", unique_days.len());
        
        println!("\n✅ CONFIRMED: Aave Base USDC has {} days of historical data in The Graph", 
            unique_days.len());
        println!("✅ CONFIRMED: Total of {} data points need deduplication", points.len());
        
    } else {
        panic!("No data returned from The Graph API");
    }
    
    Ok(())
}
