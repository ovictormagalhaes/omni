use anyhow::Result;
use chrono::{DateTime, Utc};
use mongodb::{bson::doc, Collection, Database};
use serde::{Deserialize, Serialize};

use crate::models::{Chain, Protocol, WorkerExecutionRecord};

/// Aggregated health metrics for a protocol+chain pair,
/// computed from the last N worker execution records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolHealthMetrics {
    pub protocol: Protocol,
    pub chain: Chain,

    /// Percentage of recent runs where this indexer succeeded (0.0–100.0)
    #[serde(rename = "uptimePercent")]
    pub uptime_percent: f64,

    /// Average execution time in ms over recent runs
    #[serde(rename = "avgLatencyMs")]
    pub avg_latency_ms: f64,

    /// Number of recent runs considered
    #[serde(rename = "sampleCount")]
    pub sample_count: usize,

    /// Average vaults/pools found per run
    #[serde(rename = "avgItemsFound")]
    pub avg_items_found: f64,

    /// Last time this indexer was seen succeeding
    #[serde(rename = "lastSuccessAt", skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<DateTime<Utc>>,

    /// Last error message (if most recent run failed)
    #[serde(rename = "lastError", skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,

    /// Computed at
    #[serde(rename = "computedAt")]
    pub computed_at: DateTime<Utc>,
}

pub struct ProtocolHealthService {
    executions: Collection<WorkerExecutionRecord>,
}

impl ProtocolHealthService {
    pub async fn new(mongodb_url: &str, database: &str) -> Result<Self> {
        let client = mongodb::Client::with_uri_str(mongodb_url).await?;
        let db: Database = client.database(database);
        Ok(Self {
            executions: db.collection("worker_executions"),
        })
    }

    /// Compute health metrics from the last `window` execution records.
    pub async fn compute_health(&self, window: usize) -> Result<Vec<ProtocolHealthMetrics>> {
        use futures::TryStreamExt;

        let options = mongodb::options::FindOptions::builder()
            .sort(doc! { "executedAt": -1 })
            .limit(window as i64)
            .build();

        let cursor = self.executions.find(doc! {}).with_options(options).await?;
        let records: Vec<WorkerExecutionRecord> = cursor.try_collect().await?;

        if records.is_empty() {
            return Ok(vec![]);
        }

        // Aggregate per (protocol, chain)
        use std::collections::HashMap;
        struct Acc {
            successes: u32,
            total: u32,
            total_latency_ms: i64,
            total_items: usize,
            last_success_at: Option<DateTime<Utc>>,
            last_error: Option<String>,
        }

        let mut map: HashMap<(Protocol, Chain), Acc> = HashMap::new();

        for record in &records {
            for ps in &record.protocol_breakdown {
                let key = (ps.protocol.clone(), ps.chain.clone());
                let acc = map.entry(key).or_insert_with(|| Acc {
                    successes: 0,
                    total: 0,
                    total_latency_ms: 0,
                    total_items: 0,
                    last_success_at: None,
                    last_error: None,
                });

                acc.total += 1;
                acc.total_latency_ms += ps.execution_time_ms;
                acc.total_items += ps.vaults_found;

                if ps.error.is_none() {
                    acc.successes += 1;
                    if acc.last_success_at.is_none() {
                        acc.last_success_at = Some(record.executed_at);
                    }
                } else if acc.last_error.is_none() {
                    acc.last_error = ps.error.clone();
                }
            }

            // Also count pool breakdown
            for ps in &record.pool_breakdown {
                let key = (ps.protocol.clone(), ps.chain.clone());
                let acc = map.entry(key).or_insert_with(|| Acc {
                    successes: 0,
                    total: 0,
                    total_latency_ms: 0,
                    total_items: 0,
                    last_success_at: None,
                    last_error: None,
                });

                acc.total += 1;
                acc.total_latency_ms += ps.execution_time_ms;
                acc.total_items += ps.pools_found;

                if ps.error.is_none() {
                    acc.successes += 1;
                    if acc.last_success_at.is_none() {
                        acc.last_success_at = Some(record.executed_at);
                    }
                } else if acc.last_error.is_none() {
                    acc.last_error = ps.error.clone();
                }
            }
        }

        let now = Utc::now();
        let metrics: Vec<ProtocolHealthMetrics> = map
            .into_iter()
            .map(|((protocol, chain), acc)| ProtocolHealthMetrics {
                protocol,
                chain,
                uptime_percent: if acc.total > 0 {
                    (acc.successes as f64 / acc.total as f64) * 100.0
                } else {
                    0.0
                },
                avg_latency_ms: if acc.total > 0 {
                    acc.total_latency_ms as f64 / acc.total as f64
                } else {
                    0.0
                },
                sample_count: acc.total as usize,
                avg_items_found: if acc.total > 0 {
                    acc.total_items as f64 / acc.total as f64
                } else {
                    0.0
                },
                last_success_at: acc.last_success_at,
                last_error: acc.last_error,
                computed_at: now,
            })
            .collect();

        Ok(metrics)
    }
}
