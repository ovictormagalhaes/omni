use serde::{Deserialize, Serialize, Serializer};
use chrono::{DateTime, Utc};
use bson;

// Custom serializer for f64 with 5 decimal places
mod round_f64_5 {
    use serde::{Serializer};
    
    pub fn serialize<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let rounded = (value * 100000.0).round() / 100000.0;
        serializer.serialize_f64(rounded)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Supply,
    Borrow,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Lending,
    Vault,
    Staking,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Aave,
    Kamino,
    Morpho,
    Fluid,
    SparkLend,
    JustLend,
    Euler,
    Jupiter,
    Lido,
    Marinade,
    Jito,
    RocketPool,
}

// Implement Display for Protocol to support formatting
impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    Ethereum,
    Solana,
    #[serde(rename = "bsc")]
    BSC,
    Bitcoin,
    Tron,
    Base,
    Arbitrum,
    Polygon,
    Optimism,
    Avalanche,
    Sui,
    Hyperliquid,
    Scroll,
    Mantle,
    Linea,
    Blast,
    Fantom,
    #[serde(rename = "zksync")]
    ZkSync,
    Aptos,
    Celo,
}

// Implement Display for Chain to support formatting
impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Asset {
    Known(KnownAsset),
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum KnownAsset {
    // USD Stablecoins
    USDC,
    USDT,
    DAI,
    USDE,   // Ethena USD
    #[serde(rename = "SUSDE")]
    SUSDE,  // Staked Ethena USD
    PYUSD,  // PayPal USD
    FRAX,   // Frax
    LUSD,   // Liquity USD
    GHO,    // Aave GHO
    #[serde(rename = "CRVUSD")]
    CRVUSD, // Curve USD
    #[serde(rename = "USDD")]
    USDD,   // Tron USDD
    
    // EUR Stablecoins
    EURC,   // Circle EUR
    EURS,   // STASIS EUR
    EURT,   // Tether EUR
    
    // ETH and LSTs
    WETH,
    #[serde(rename = "ETH")]
    ETH,
    #[serde(rename = "STETH")]
    STETH,  // Lido stETH
    WSTETH, // Lido Wrapped Staked ETH
    RETH,   // Rocket Pool ETH
    CBETH,  // Coinbase ETH
    #[serde(rename = "SETH2")]
    SETH2,  // StakeWise ETH2
    #[serde(rename = "SFRXETH")]
    SFRXETH, // Staked Frax ETH
    
    // BTC
    WBTC,
    CBBTC,  // Coinbase Wrapped BTC
    TBTC,   // Threshold BTC
    SBTC,   // Synth BTC
    
    // SOL and LSTs
    SOL,
    #[serde(rename = "STSOL")]
    STSOL,   // Lido stSOL
    #[serde(rename = "MSOL")]
    MSOL,    // Marinade mSOL
    #[serde(rename = "JITOSOL")]
    JITOSOL, // Jito JitoSOL
    #[serde(rename = "JUPSOL")]
    JUPSOL,  // Jupiter JupSOL
    
    // Other
    TRX,    // Tron
    LINK,   // Chainlink
    AAVE,   // Aave token
    UNI,    // Uniswap
    CRV,    // Curve
    BAL,    // Balancer
    COMP,   // Compound
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AssetCategory {
    #[serde(rename = "usd-correlated")]
    UsdCorrelated,
    #[serde(rename = "stablecoin")]
    Stablecoin,
    #[serde(rename = "btc-correlated")]
    BtcCorrelated,
    #[serde(rename = "eth-correlated")]
    EthCorrelated,
    #[serde(rename = "sol-correlated")]
    SolCorrelated,
    #[serde(rename = "other")]
    Other,
}

impl Asset {
    pub fn symbol(&self) -> String {
        match self {
            Asset::Known(known) => format!("{:?}", known).to_uppercase(),
            Asset::Unknown(symbol) => symbol.clone(),
        }
    }
    
    pub fn category(&self) -> Vec<AssetCategory> {
        match self {
            Asset::Known(known) => known.category(),
            Asset::Unknown(_) => vec![], // Tokens desconhecidos não têm categoria
        }
    }
    
    /// Helper function to normalize asset strings across indexers
    pub fn from_symbol(symbol: &str, protocol: &str) -> Asset {
        let known = match symbol.to_uppercase().as_str() {
            "USDC" => Some(KnownAsset::USDC),
            "USDT" => Some(KnownAsset::USDT),
            "DAI" => Some(KnownAsset::DAI),
            "USDE" => Some(KnownAsset::USDE),
            "SUSDE" => Some(KnownAsset::SUSDE),
            "PYUSD" => Some(KnownAsset::PYUSD),
            "FRAX" => Some(KnownAsset::FRAX),
            "LUSD" => Some(KnownAsset::LUSD),
            "GHO" => Some(KnownAsset::GHO),
            "CRVUSD" => Some(KnownAsset::CRVUSD),
            "EURC" => Some(KnownAsset::EURC),
            "EURS" => Some(KnownAsset::EURS),
            "EURT" => Some(KnownAsset::EURT),
            "WETH" | "ETH" => Some(KnownAsset::ETH),
            "STETH" => Some(KnownAsset::STETH),
            "WSTETH" => Some(KnownAsset::WSTETH),
            "RETH" => Some(KnownAsset::RETH),
            "CBETH" => Some(KnownAsset::CBETH),
            "SETH2" => Some(KnownAsset::SETH2),
            "SFRXETH" => Some(KnownAsset::SFRXETH),
            "WBTC" | "BTC" => Some(KnownAsset::WBTC),
            "CBBTC" => Some(KnownAsset::CBBTC),
            "TBTC" => Some(KnownAsset::TBTC),
            "SBTC" => Some(KnownAsset::SBTC),
            "SOL" => Some(KnownAsset::SOL),
            "STSOL" => Some(KnownAsset::STSOL),
            "MSOL" => Some(KnownAsset::MSOL),
            "JITOSOL" => Some(KnownAsset::JITOSOL),
            "JUPSOL" => Some(KnownAsset::JUPSOL),
            "TRX" => Some(KnownAsset::TRX),
            "USDD" => Some(KnownAsset::USDD),
            "LINK" => Some(KnownAsset::LINK),
            "AAVE" => Some(KnownAsset::AAVE),
            "UNI" => Some(KnownAsset::UNI),
            "CRV" => Some(KnownAsset::CRV),
            "BAL" => Some(KnownAsset::BAL),
            "COMP" => Some(KnownAsset::COMP),
            _ => None,
        };
        
        match known {
            Some(k) => Asset::Known(k),
            None => {
                tracing::warn!("[{}] Unknown asset detected: '{}' - adding as Unknown", protocol, symbol);
                Asset::Unknown(symbol.to_uppercase())
            }
        }
    }
}

impl KnownAsset {
    pub fn category(&self) -> Vec<AssetCategory> {
        use KnownAsset::*;
        match self {
            // USD Correlated: apenas USDC e USDT
            USDC | USDT => vec![AssetCategory::UsdCorrelated, AssetCategory::Stablecoin],
            
            // Stablecoin: todas as outras stablecoins fiat-pegged
            DAI | USDE | SUSDE | PYUSD | FRAX | LUSD | GHO | CRVUSD | USDD |
            EURC | EURS | EURT => vec![AssetCategory::Stablecoin],
            
            // BTC Correlated
            WBTC | CBBTC | TBTC | SBTC => vec![AssetCategory::BtcCorrelated],
            
            // ETH Correlated
            ETH | WETH | STETH | WSTETH | RETH | CBETH | SETH2 | SFRXETH => 
                vec![AssetCategory::EthCorrelated],
            
            // SOL Correlated
            SOL | STSOL | MSOL | JITOSOL | JUPSOL => vec![AssetCategory::SolCorrelated],
            
            // Outros tokens não entram em categorias de filtro
            TRX | LINK | AAVE | UNI | CRV | BAL | COMP => vec![],
        }
    }
}

// Implement Display for Asset to support formatting
impl std::fmt::Display for Asset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

#[derive(Debug, Deserialize)]
pub struct RateQuery {
    #[serde(default)]
    pub action: Option<Action>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub chains: Option<String>,
    #[serde(default)]
    pub protocols: Option<String>,    #[serde(default)]
    pub operation_types: Option<String>,    #[serde(default)]
    pub asset_categories: Option<String>,
    /// Minimum liquidity in USD (default: 1000000)
    #[serde(default = "default_min_liquidity")]
    pub min_liquidity: u64,
}

fn default_min_liquidity() -> u64 {
    1_000_000
}

impl RateQuery {
    /// Parse chains from comma-separated string
    pub fn parse_chains(&self) -> Option<Vec<Chain>> {
        self.chains.as_ref().map(|s| {
            s.split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .filter_map(|item| serde_json::from_str(&format!("\"{}\"", item)).ok())
                .collect()
        })
    }

    /// Parse protocols from comma-separated string
    pub fn parse_protocols(&self) -> Option<Vec<Protocol>> {
        self.protocols.as_ref().map(|s| {
            s.split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .filter_map(|item| serde_json::from_str(&format!("\"{}\"", item)).ok())
                .collect()
        })
    }

    /// Parse operation_types from comma-separated string (lending, vault)
    pub fn parse_operation_types(&self) -> Option<Vec<OperationType>> {
        self.operation_types.as_ref().map(|s| {
            s.split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .filter_map(|item| serde_json::from_str(&format!("\"{}\"", item)).ok())
                .collect()
        })
    }

    /// Parse assets from comma-separated list of symbols (e.g., "USDC,USDT,ETH")
    pub fn parse_assets(&self) -> Option<Vec<String>> {
        self.assets.as_ref().map(|s| {
            s.split(',')
                .map(|item| item.trim().to_uppercase())
                .filter(|item| !item.is_empty())
                .collect()
        })
    }

    /// Parse asset_categories from comma-separated string (usd-based, btc-based, eth-based)
    pub fn parse_asset_categories(&self) -> Option<Vec<AssetCategory>> {
        self.asset_categories.as_ref().map(|s| {
            s.split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .filter_map(|item| serde_json::from_str(&format!("\"{}\"", item)).ok())
                .collect()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateResult {
    pub protocol: Protocol,
    pub chain: Chain,
    pub asset: Asset,
    pub action: Action,
    #[serde(rename = "assetCategory")]
    pub asset_category: Vec<AssetCategory>,
    pub apy: f64,
    /// Additional rewards APY from protocol tokens (e.g., AAVE, MORPHO, etc.)
    pub rewards: f64,
    /// Net APY = base APY + rewards
    #[serde(rename = "netApy")]
    pub net_apy: f64,
    /// Performance fee charged by the vault/protocol (as decimal, e.g., 0.1 = 10%)
    #[serde(rename = "performanceFee", skip_serializing_if = "Option::is_none")]
    pub performance_fee: Option<f64>,
    /// Whether the vault/protocol is currently active (not paused, deprecated, or closed)
    pub active: bool,
    /// Whether the asset can be used as collateral for borrowing
    #[serde(rename = "collateralEnabled")]
    pub collateral_enabled: bool,
    /// Maximum loan-to-value ratio for collateral (0.0 to 1.0, e.g., 0.75 = 75%)
    #[serde(rename = "collateralLtv")]
    pub collateral_ltv: f64,
    pub liquidity: u64,
    #[serde(rename = "totalLiquidity")]
    pub total_liquidity: u64,
    #[serde(rename = "utilizationRate")]
    pub utilization_rate: u32,
    #[serde(rename = "operationType")]
    pub operation_type: OperationType,
    pub url: String,
    #[serde(rename = "vaultId", skip_serializing_if = "Option::is_none")]
    pub vault_id: Option<String>,
    #[serde(rename = "vaultName", skip_serializing_if = "Option::is_none")]
    pub vault_name: Option<String>,
    #[serde(rename = "lastUpdate")]
    pub last_update: DateTime<Utc>,
    /// APY metrics for different time periods
    #[serde(rename = "apyMetrics", skip_serializing_if = "Option::is_none")]
    pub apy_metrics: Option<ApyMetrics>,
}

#[derive(Debug, Serialize)]
pub struct RateResponse {
    pub success: bool,
    pub timestamp: DateTime<Utc>,
    pub query: QueryInfo,
    pub results: Vec<RateResult>,
    pub count: usize,
    #[serde(rename = "totalLiquidity")]
    pub total_liquidity: u64,
}

#[derive(Debug, Serialize)]
pub struct QueryInfo {
    pub action: Action,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Vec<String>>,
    pub chains: Vec<Chain>,
    pub protocols: Vec<Protocol>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

// Internal protocol rate format (before aggregation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRate {
    pub protocol: Protocol,
    pub chain: Chain,
    pub asset: Asset,
    pub action: Action,
    pub supply_apy: f64,
    pub borrow_apr: f64,
    /// Additional rewards APY from protocol tokens
    pub rewards: f64,
    /// Performance fee charged by the vault/protocol (as decimal, e.g., 0.1 = 10%)
    pub performance_fee: Option<f64>,
    /// Whether the vault/protocol is currently active (not paused, deprecated, or closed)
    pub active: bool,
    /// Whether the asset can be used as collateral for borrowing
    pub collateral_enabled: bool,
    /// Maximum loan-to-value ratio for collateral (0.0 to 1.0, e.g., 0.75 = 75%)
    pub collateral_ltv: f64,
    pub available_liquidity: u64,
    pub total_liquidity: u64,
    pub utilization_rate: f64,
    pub ltv: f64,
    pub operation_type: OperationType,
    pub vault_id: Option<String>,
    pub vault_name: Option<String>, // Human-readable vault name
    pub underlying_asset: Option<String>,  // Token contract address
    pub timestamp: DateTime<Utc>,
}

impl Chain {
    pub fn all() -> Vec<Chain> {
        vec![
            Chain::Ethereum,
            Chain::Solana,
            Chain::BSC,
            Chain::Bitcoin,
            Chain::Tron,
            Chain::Base,
            Chain::Arbitrum,
            Chain::Polygon,
            Chain::Optimism,
            Chain::Avalanche,
            Chain::Sui,
            Chain::Hyperliquid,
            Chain::Scroll,
            Chain::Mantle,
            Chain::Linea,
            Chain::Blast,
            Chain::Fantom,
            Chain::ZkSync,
            Chain::Aptos,
            Chain::Celo,
        ]
    }
}

impl Protocol {
    pub fn all() -> Vec<Protocol> {
        vec![
            Protocol::Aave, 
            Protocol::Kamino, 
            Protocol::Morpho, 
            Protocol::Fluid, 
            Protocol::SparkLend, 
            Protocol::JustLend, 
            Protocol::Euler, 
            Protocol::Jupiter,
            Protocol::Lido,
            Protocol::Marinade,
            Protocol::Jito,
            Protocol::RocketPool,
        ]
    }
}

// Default action for legacy snapshots without action field
fn default_action() -> Action {
    Action::Supply
}

// ============================================================================
// HISTORICAL DATA MODELS (for MongoDB storage and backtesting)
// ============================================================================

/// Daily snapshot of rates for a specific vault (protocol + chain + pool)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateSnapshot {
    /// MongoDB document ID
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<bson::oid::ObjectId>,
    
    /// Date of snapshot (UTC, start of day) — stored as BSON Date for range queries
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub date: DateTime<Utc>,
    
    /// Unique vault identifier (hash of protocol+chain+asset+url)
    pub vault_id: String,
    
    /// Protocol identifier
    pub protocol: Protocol,
    
    /// Blockchain
    pub chain: Chain,
    
    /// Asset identifier
    pub asset: String,
    
    /// Human-readable vault name (e.g., "Aave USDC Arbitrum", "Kamino USDC Main")
    pub vault_name: Option<String>,
    
    /// Deep link URL to protocol
    pub url: String,
    
    /// Operation type (lending, vault, staking)
    pub operation_type: OperationType,
    
    /// Action (Supply or Borrow) - defaults to Supply for legacy snapshots
    #[serde(default = "default_action")]
    pub action: Action,
    
    /// Net APY (includes base + rewards)
    pub net_apy: f64,
    
    /// Base APY without rewards
    pub base_apy: f64,
    
    /// Rewards APY from protocol tokens
    pub rewards_apy: f64,
    
    /// Total liquidity available (USD)
    pub liquidity_usd: u64,
    
    /// Total Value Locked (USD)
    pub tvl_usd: u64,
    
    /// Utilization rate (0-100)
    pub utilization_rate: u32,
    
    /// Additional metadata
    pub metadata: Option<bson::Document>,
    
    /// When this snapshot was collected
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub collected_at: DateTime<Utc>,
}

impl RateSnapshot {
    /// Create snapshot from RateResult
    pub fn from_rate_result(rate: &RateResult, date: DateTime<Utc>) -> Self {
        let vault_id = Self::generate_vault_id(
            &rate.protocol,
            &rate.chain,
            &rate.asset.to_string(),
            &rate.url,
            rate.operation_type,
            Some(&rate.action),
        );
        
        RateSnapshot {
            id: None,
            date,
            vault_id,
            protocol: rate.protocol.clone(),
            chain: rate.chain.clone(),
            asset: rate.asset.to_string(),
            vault_name: rate.vault_name.clone(),
            url: rate.url.clone(),
            operation_type: rate.operation_type,
            action: rate.action.clone(),
            net_apy: rate.net_apy,
            base_apy: rate.apy,
            rewards_apy: rate.rewards,
            liquidity_usd: rate.liquidity,
            tvl_usd: rate.total_liquidity,
            utilization_rate: rate.utilization_rate,
            metadata: None,
            collected_at: chrono::Utc::now(),
        }
    }
    
    /// Generate deterministic, stable vault ID from components.
    /// Inputs: protocol + chain + asset + url + operation_type.
    /// Uses SHA-256 (first 16 hex chars = 64 bits) so the ID is:
    ///  - Stable across Rust versions (unlike DefaultHasher)
    ///  - Unique per (protocol, chain, asset, url, operation_type)
    ///    i.e. supply and borrow of the same pool get DIFFERENT vault_ids
    pub fn generate_vault_id(
        protocol: &Protocol,
        chain: &Chain,
        asset: &str,
        url: &str,
        operation_type: OperationType,
        action: Option<&Action>,
    ) -> String {
        use sha2::{Sha256, Digest};
        let action_str = action.map(|a| format!("{:?}", a).to_lowercase()).unwrap_or_default();
        let key = format!(
            "{}|{}|{}|{}|{:?}|{}",
            format!("{:?}", protocol).to_lowercase(),
            format!("{:?}", chain).to_lowercase(),
            asset.to_uppercase(),
            url,
            operation_type,
            action_str,
        );
        let hash = Sha256::digest(key.as_bytes());
        // First 16 hex chars (8 bytes) — compact but still astronomically collision-resistant
        // for our scale (~thousands of vaults).
        format!("{:x}", hash)[..16].to_string()
    }
}

/// Query parameters for historical data retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalQuery {
    /// Start date (inclusive)
    pub start_date: DateTime<Utc>,
    
    /// End date (inclusive)
    pub end_date: DateTime<Utc>,
    
    /// Filter by protocol
    pub protocol: Option<Protocol>,
    
    /// Filter by chain
    pub chain: Option<Chain>,
    
    /// Filter by asset
    pub asset: Option<String>,
    
    /// Rate type (supply or borrow)
    pub action: Option<Action>,
}

/// Aggregated statistics for backtesting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestStats {
    /// Asset analyzed
    pub asset: String,
    
    /// Time period
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    
    /// Average APY across period
    pub avg_apy: f64,
    
    /// Minimum APY observed
    pub min_apy: f64,
    
    /// Maximum APY observed
    pub max_apy: f64,
    
    /// Standard deviation (volatility)
    pub std_deviation: f64,
    
    /// Best protocol during period
    pub best_protocol: Protocol,
    pub best_protocol_avg_apy: f64,
    
    /// Hypothetical earnings on $1M investment
    pub earnings_on_1m: f64,
    
    /// Number of data points
    pub sample_size: usize,
}

// ============================================================================
// VAULT HISTORY MODELS (for APY chart / detail view)
// ============================================================================

/// A single data-point in a vault's APY time-series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultHistoryPoint {
    pub date: DateTime<Utc>,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub net_apy: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub base_apy: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub rewards_apy: f64,
    pub liquidity_usd: u64,
    pub utilization_rate: u32,
}

/// Response model for vault history endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultHistoryResponse {
    pub success: bool,
    pub vault_id: String,
    pub vault_name: Option<String>,
    pub protocol: Option<Protocol>,
    pub chain: Option<Chain>,
    pub asset: Option<String>,
    pub operation_type: Option<OperationType>,
    pub url: Option<String>,
    pub days: u32,
    pub points: Vec<VaultHistoryPoint>,
    /// Summary stats over the returned window
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub avg_apy: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub min_apy: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub max_apy: f64,
    /// false when no snapshots were found (data not yet collected for this vault)
    pub data_available: bool,
}

/// Worker execution record for monitoring and auditing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerExecutionRecord {
    /// MongoDB document ID
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<bson::oid::ObjectId>,
    
    /// When the worker was executed
    #[serde(rename = "executedAt")]
    pub executed_at: DateTime<Utc>,
    
    /// Target collection date (which day's data was collected)
    #[serde(rename = "collectionDate")]
    pub collection_date: String, // "2026-02-18"
    
    /// Execution status
    pub status: ExecutionStatus,
    
    /// Error details if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
    
    /// Execution statistics
    pub stats: ExecutionStats,
    
    /// Duration in seconds
    #[serde(rename = "durationSeconds")]
    pub duration_seconds: i64,
    
    /// Breakdown by protocol
    #[serde(rename = "protocolBreakdown")]
    pub protocol_breakdown: Vec<ProtocolStats>,
    
    /// System info
    #[serde(rename = "systemInfo")]
    pub system_info: SystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Success,
    PartialSuccess, // Some protocols failed but others succeeded
    Failed,
    Skipped, // Already collected today
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    pub message: String,
    #[serde(rename = "failedProtocols")]
    pub failed_protocols: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Total vaults processed
    #[serde(rename = "vaultsProcessed")]
    pub vaults_processed: usize,
    
    /// Today's snapshots inserted
    #[serde(rename = "snapshotsInserted")]
    pub snapshots_inserted: usize,
    
    /// Snapshots updated (if re-running)
    #[serde(rename = "snapshotsUpdated")]
    pub snapshots_updated: usize,
    
    /// New vaults discovered (first time seeing them)
    #[serde(rename = "newVaultsDiscovered")]
    pub new_vaults_discovered: usize,
    
    /// Backfill snapshots created
    #[serde(rename = "backfillSnapshotsCreated")]
    pub backfill_snapshots_created: usize,
    
    /// Vaults with real historical data fetched
    #[serde(rename = "vaultsWithRealHistory")]
    pub vaults_with_real_history: usize,
    
    /// Vaults skipped (no historical data source available)
    #[serde(rename = "vaultsSkippedNoHistory")]
    pub vaults_skipped_no_history: usize,
    
    /// Total data points in database after execution
    #[serde(rename = "totalSnapshotsInDb")]
    pub total_snapshots_in_db: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStats {
    pub protocol: Protocol,
    
    /// Vaults found for this protocol
    #[serde(rename = "vaultsFound")]
    pub vaults_found: usize,
    
    /// Snapshots inserted for this protocol
    #[serde(rename = "snapshotsInserted")]
    pub snapshots_inserted: usize,
    
    /// Did we get real historical data?
    #[serde(rename = "historicalDataSource")]
    pub historical_data_source: HistoricalDataSource,
    
    /// Execution time for this protocol (ms)
    #[serde(rename = "executionTimeMs")]
    pub execution_time_ms: i64,
    
    /// Error if protocol failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HistoricalDataSource {
    TheGraph,       // Real data from TheGraph
    ProtocolApi,    // Real data from protocol's API
    NotApplicable,  // No historical data source available (vault skipped)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Worker version/commit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    
    /// Hostname/pod name (for k8s)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    
    /// Environment (dev, staging, production)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
}
/// Real-time consolidated rate (1 doc per vault)
/// Updated by worker after each collection cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeRate {
    /// MongoDB _id
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<bson::oid::ObjectId>,
    
    /// Unique vault identifier
    pub vault_id: String,
    
    /// Protocol
    pub protocol: Protocol,
    
    ///Chain
    pub chain: Chain,
    
    /// Asset symbol
    pub asset: String,
    
    /// Asset category
    pub asset_category: AssetCategory,
    
    /// Vault name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault_name: Option<String>,
    
    /// Deep link URL
    pub url: String,
    
    /// Operation type
    pub operation_type: OperationType,
    
    /// Action (Supply or Borrow)
    pub action: Action,
    
    /// Current (latest) snapshot data
    pub current: CurrentRateData,
    
    /// APY metrics (7D, 30D, 90D averages)
    pub apy_metrics: ApyMetrics,
    
    /// Last update timestamp
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: DateTime<Utc>,
    
    /// Number of historical snapshots available
    pub snapshot_count: i32,
    
    /// First time this vault was seen
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub first_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentRateData {
    /// Base APY (without rewards)
    pub base_apy: f64,
    
    /// Rewards APY
    pub rewards_apy: f64,
    
    /// Net APY (base + rewards)
    pub net_apy: f64,
    
    /// Available liquidity (USD)
    pub liquidity_usd: u64,
    
    /// Total liquidity/TVL (USD)
    pub tvl_usd: u64,
    
    /// Utilization rate (0-100)
    pub utilization_rate: u32,
    
    /// When this rate was collected
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub collected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApyMetrics {
    /// Current instant APY
    pub instant: f64,
    
    /// 7-day average APY
    pub apy_7d: f64,
    
    /// 30-day average APY
    pub apy_30d: f64,
    
    /// 60-day average APY
    pub apy_60d: f64,
    
    /// 90-day average APY
    pub apy_90d: f64,
    
    /// APY volatility (standard deviation)
    pub volatility: f64,
    
    /// Number of days with data
    pub days_with_data: i32,
}
