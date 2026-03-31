use axum::{
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use std::sync::Arc;

use crate::models::VaultHistoryResponse;
use crate::{models::*, AppState};

pub async fn get_rates(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RateQuery>,
) -> Result<Json<RateResponse>, AppError> {
    tracing::info!(
        "Received rate query - action: {:?}, assets: {:?}, chains: {:?}, protocols: {:?}",
        query.action,
        query.assets,
        query.chains,
        query.protocols
    );

    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);

    // Read from MongoDB with pagination — already sorted by net_apy
    let (results, total_count) = state.realtime_service.query_rates(&query).await?;

    let count = results.len();
    let total_liquidity: u64 = results.iter().map(|r| r.liquidity).sum();
    let total_pages = if total_count == 0 {
        0
    } else {
        total_count.div_ceil(page_size)
    };

    let response = RateResponse {
        success: true,
        timestamp: Utc::now(),
        query: QueryInfo {
            action: query.action.clone().unwrap_or(Action::Supply),
            assets: query.parse_assets(),
            chains: query.parse_chains().unwrap_or_else(Chain::all),
            protocols: query.parse_protocols().unwrap_or_else(Protocol::all),
        },
        results,
        count,
        total_liquidity,
        page,
        page_size,
        total_count: total_count as usize,
        total_pages,
    };

    Ok(Json(response))
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
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal error occurred. Please try again later.".to_string(),
                )
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
    let stats = state.historical_service.backtest(historical_query).await?;

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
    let data = state
        .historical_service
        .get_vault_history(
            params.vault_id.as_deref(),
            params.protocol.as_ref(),
            params.chain.as_ref(),
            params.asset.as_deref(),
        )
        .await?;

    Ok(Json(data))
}

// ============================================================================
// LIQUIDITY POOL ENDPOINTS
// ============================================================================

/// GET /api/v1/pools
/// Returns liquidity pools across DEXes, filterable by asset category, token, chain, protocol.
/// Default sort: TVL descending (highest liquidity first).
///
/// Query parameters:
/// - asset_categories_0 (optional): categories for one side, e.g. "btc-correlated"
/// - asset_categories_1 (optional): categories for other side, e.g. "usd-correlated"
/// - token_a (optional): search by token symbol (substring match on token0)
/// - token_b (optional): search by token symbol (substring match on token1)
/// - token (optional): search by token symbol, substring match on either side of pair
/// - pair (optional): exact pair like "ETH/USDC"
/// - chains (optional): "ethereum,solana,arbitrum"
/// - protocols (optional): "uniswap,raydium"
/// - pool_type (optional): "concentrated" or "standard"
/// - min_tvl (optional): minimum TVL in USD (default: 10000)
/// - min_volume (optional): minimum 24h volume in USD (default: 0)
/// - page (optional): page number, 1-indexed (default: 1)
/// - page_size (optional): results per page, max 100 (default: 20)
///   GET /api/v1/pools/history
///   Returns daily fee APR / TVL / volume time-series for a specific pool (90 days).
///
/// Query parameters:
/// - pool_vault_id (optional): unique pool identifier
/// - protocol (optional): e.g. "uniswap"
/// - chain (optional): e.g. "ethereum"
/// - pair (optional): e.g. "ETH/USDC"
#[derive(serde::Deserialize, Debug)]
pub struct PoolHistoryQuery {
    pub pool_vault_id: Option<String>,
    pub protocol: Option<Protocol>,
    pub chain: Option<Chain>,
    pub pair: Option<String>,
}

pub async fn pool_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PoolHistoryQuery>,
) -> Result<Json<PoolHistoryResponse>, AppError> {
    let data = state
        .pool_historical_service
        .get_pool_history(
            params.pool_vault_id.as_deref(),
            params.protocol.as_ref(),
            params.chain.as_ref(),
            params.pair.as_deref(),
        )
        .await?;

    Ok(Json(data))
}

// ============================================================================
// SCORE ENDPOINTS
// ============================================================================

/// POST /api/v1/score/pool
/// Analyze a pool position and find better alternatives across protocols/chains.
///
/// Request body (JSON):
/// {
///   "token0": "CBBTC",
///   "token1": "WETH",
///   "protocol": "uniswap",   // optional — to identify YOUR pool
///   "chain": "base",         // optional
///   "fee_tier": 5,            // optional — basis points (5 = 0.05%, 30 = 0.30%, 100 = 1%)
///   "min_tvl": 10000         // optional (default: 10000)
/// }
pub async fn score_pool(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Json<PoolScoreResponse>, AppError> {
    let req: PoolScoreRequest = serde_json::from_slice(&body)
        .map_err(|e| anyhow::anyhow!("Invalid request body: {}", e))?;

    tracing::info!(
        "Pool score request - token0: {}, token1: {}, protocol: {:?}, chain: {:?}, fee_tier: {:?}",
        req.token0,
        req.token1,
        req.protocol,
        req.chain,
        req.fee_tier
    );

    let response = crate::services::score::score_pool(&state.pool_realtime_service, &req).await?;

    Ok(Json(response))
}

/// POST /api/v1/score/lending
/// Analyze a lending position and find better alternatives across protocols/chains.
///
/// Request body (JSON):
/// {
///   "supply": ["CBBTC", "WETH"],
///   "borrow": ["USDC"],
///   "protocol": "aave",      // optional — to identify YOUR position
///   "chain": "base",         // optional
///   "min_liquidity": 1000000 // optional (default: 1000000)
/// }
pub async fn score_lending(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Json<LendingScoreResponse>, AppError> {
    let req: LendingScoreRequest = serde_json::from_slice(&body)
        .map_err(|e| anyhow::anyhow!("Invalid request body: {}", e))?;

    tracing::info!(
        "Lending score request - supplies: {:?}, borrows: {:?}, protocol: {:?}, chain: {:?}",
        req.supplies,
        req.borrows,
        req.protocol,
        req.chain
    );

    let response = crate::services::score::score_lending(&state.realtime_service, &req).await?;

    Ok(Json(response))
}

pub async fn get_pools(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PoolQuery>,
) -> Result<Json<PoolResponse>, AppError> {
    tracing::info!("Pool query - categories_0: {:?}, categories_1: {:?}, token_a: {:?}, token_b: {:?}, token: {:?}, pair: {:?}, chains: {:?}, protocols: {:?}",
        query.asset_categories_0, query.asset_categories_1, query.token_a, query.token_b, query.token, query.pair, query.chains, query.protocols);

    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);

    // Read from MongoDB with pagination — already sorted by fee_apr_24h desc
    let (results, total_count) = state.pool_realtime_service.query_pools(&query).await?;

    let count = results.len();
    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count + page_size) / page_size
    };

    Ok(Json(PoolResponse {
        success: true,
        timestamp: Utc::now(),
        results,
        count,
        page,
        page_size,
        total_count: total_count as usize,
        total_pages,
    }))
}
