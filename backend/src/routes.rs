use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use std::sync::Arc;

use crate::{
    models::*,
    services::aggregator::RateAggregator,
    AppState,
};
use crate::models::VaultHistoryResponse;

pub async fn get_rates(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RateQuery>,
) -> Result<Json<RateResponse>, AppError> {
    tracing::info!("Received rate query - action: {:?}, assets: {:?}, chains: {:?}, protocols: {:?}", 
        query.action, query.assets, query.chains, query.protocols);
    tracing::debug!("Received rate query: {:?}", query);

    // Fetch directly from protocols (no cache)
    tracing::info!("📡 Fetching rates directly from protocols");
    let aggregator = RateAggregator::new(state.config.clone());
    let mut results = aggregator.get_rates(&query).await?;

    // Sort based on action (supply APY descending by default)
    results.sort_by(|a, b| match &query.action {
        Some(Action::Supply) | None => b.apy.partial_cmp(&a.apy).unwrap(), // Descending for supply
        Some(Action::Borrow) => a.apy.partial_cmp(&b.apy).unwrap(), // Ascending for borrow
    });

    // Results are returned already sorted by APY according to action; no explicit rank field.

    let count = results.len();
    
    // Calculate total liquidity across all results
    let total_liquidity: u64 = results.iter().map(|r| r.liquidity).sum();

    let response = RateResponse {
        success: true,
        timestamp: Utc::now(),
        query: QueryInfo {
            action: query.action.clone().unwrap_or(Action::Supply),
            assets: query.parse_assets(),
            chains: query.parse_chains().unwrap_or_else(|| Chain::all()),
            protocols: query.parse_protocols().unwrap_or_else(|| Protocol::all()),
        },
        results,
        count,
        total_liquidity,
    };

    Ok(Json(response))
}

// ============================================================================
// ASSETS ENDPOINT
// ============================================================================

#[derive(serde::Serialize)]
pub struct AssetInfo {
    pub symbol: String,
    pub categories: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct AssetsResponse {
    pub success: bool,
    pub assets: Vec<AssetInfo>,
    pub count: usize,
}

/// GET /api/v1/assets
/// Returns distinct assets available in the live protocol data, grouped by category.
pub async fn get_assets(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AssetsResponse>, AppError> {
    let aggregator = RateAggregator::new(state.config.clone());
    let query = crate::models::RateQuery {
        action: None,
        assets: None,
        chains: None,
        protocols: None,
        operation_types: None,
        asset_categories: None,
        min_liquidity: 0,
    };
    let results = aggregator.get_rates(&query).await.unwrap_or_default();

    let mut seen = std::collections::HashSet::new();
    let mut assets: Vec<AssetInfo> = results
        .into_iter()
        .filter_map(|r| {
            let symbol = r.asset.symbol().to_uppercase();
            if seen.insert(symbol.clone()) {
                let cats = r.asset.category();
                let categories = if cats.is_empty() {
                    vec!["other".to_string()]
                } else {
                    cats.iter().map(|c| format!("{:?}", c).to_lowercase().replace('_', "-")).collect()
                };
                Some(AssetInfo { symbol, categories })
            } else {
                None
            }
        })
        .collect();

    assets.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    let count = assets.len();

    Ok(Json(AssetsResponse { success: true, assets, count }))
}

// Error handling
pub enum AppError {
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Internal(err) => {
                tracing::error!("Internal error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
        };

        let body = Json(ErrorResponse {
            success: false,
            error: ErrorDetail {
                code: "INTERNAL_ERROR".to_string(),
                message: error_message,
            },
        });

        (status, body).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self::Internal(err.into())
    }
}

// ============================================================================
// HISTORICAL DATA ENDPOINTS
// ============================================================================

/// Backtest to analyze historical performance
/// GET /api/v1/historical/backtest?start_date=2026-01-01T00:00:00Z&end_date=2026-02-14T00:00:00Z&asset=USDC&action=supply
/// 
/// Query parameters:
/// - start_date (required): ISO 8601 datetime
/// - end_date (required): ISO 8601 datetime  
/// - asset (required): Asset symbol (USDC, ETH, etc)
/// - action (optional): supply or borrow (default: supply)
/// - protocol (optional): Filter by specific protocol
/// - chain (optional): Filter by specific chain
pub async fn backtest(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BacktestQuery>,
) -> Result<Json<BacktestResponse>, AppError> {
    tracing::info!("Backtest request: {:?}", params);
    
    // Parse dates
    use chrono::DateTime;
    let start_date = DateTime::parse_from_rfc3339(&params.start_date)
        .map_err(|e| anyhow::anyhow!("Invalid start_date: {}", e))?
        .with_timezone(&Utc);
        
    let end_date = DateTime::parse_from_rfc3339(&params.end_date)
        .map_err(|e| anyhow::anyhow!("Invalid end_date: {}", e))?
        .with_timezone(&Utc);
    
    // Build historical query
    let historical_query = HistoricalQuery {
        start_date,
        end_date,
        protocol: params.protocol,
        chain: params.chain,
        asset: Some(params.asset.clone()),
        action: params.action.clone(),
    };
    
    // Run backtest
    let stats = state.historical_service
        .backtest(historical_query)
        .await?;
    
    Ok(Json(BacktestResponse {
        success: true,
        timestamp: Utc::now(),
        stats,
    }))
}

// ============================================================================
// HISTORICAL DATA RESPONSE MODELS
// ============================================================================

#[derive(serde::Serialize)]
pub struct CollectSnapshotResponse {
    pub success: bool,
    pub timestamp: chrono::DateTime<Utc>,
    pub rates_collected: usize,
    pub rates_saved: usize,
}

#[derive(serde::Deserialize, Debug)]
pub struct BacktestQuery {
    pub start_date: String, // ISO 8601
    pub end_date: String,   // ISO 8601
    pub asset: String,
    pub action: Option<Action>,
    pub protocol: Option<Protocol>,
    pub chain: Option<Chain>,
}

#[derive(serde::Serialize)]
pub struct BacktestResponse {
    pub success: bool,
    pub timestamp: chrono::DateTime<Utc>,
    pub stats: BacktestStats,
}

// ============================================================================
// VAULT HISTORY ENDPOINT
// ============================================================================

/// GET /api/v1/rates/history
/// Returns daily APY time-series for a specific vault (used by the detail drawer/chart).
/// Always returns 90 days of data. Frontend filters locally for 30/60/90 day views.
///
/// Query parameters (use vault_id OR protocol+chain+asset):
/// - vault_id   (optional) – unique vault identifier
/// - protocol   (optional) – e.g. "aave"
/// - chain      (optional) – e.g. "arbitrum"
/// - asset      (optional) – e.g. "USDC"
#[derive(serde::Deserialize, Debug)]
pub struct VaultHistoryQuery {
    pub vault_id: Option<String>,
    pub protocol: Option<Protocol>,
    pub chain: Option<Chain>,
    pub asset: Option<String>,
}

pub async fn vault_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<VaultHistoryQuery>,
) -> Result<Json<VaultHistoryResponse>, AppError> {
    let data = state.historical_service
        .get_vault_history(
            params.vault_id.as_deref(),
            params.protocol.as_ref(),
            params.chain.as_ref(),
            params.asset.as_deref(),
        )
        .await?;

    Ok(Json(data))
}