use anyhow::Result;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub redis_url: String,
    pub redis_cache_ttl: u64,
    pub mongodb_url: String,
    pub mongodb_database: String,
    pub backfill_days: i64,
    pub aave_subgraph_arbitrum: String,
    pub aave_subgraph_base: String,
    pub kamino_api_url: String,
    pub morpho_api_url: String,
    pub fluid_api_url: String,
    pub trongrid_api_key: Option<String>,
    pub the_graph_api_key: Option<String>,
    pub raydium_api_url: String,
    pub cors_origins: Vec<String>,
    // Performance tuning
    pub max_concurrent_indexers: usize,
    pub backfill_concurrency: usize,
    pub indexer_timeout_secs: u64,
    pub cb_failure_threshold: u32,
    pub cb_cooldown_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Config {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            redis_cache_ttl: std::env::var("REDIS_CACHE_TTL")
                .unwrap_or_else(|_| "300".to_string())
                .parse()?,
            mongodb_url: std::env::var("MONGODB_URL")
                .unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string()),
            mongodb_database: std::env::var("MONGODB_DATABASE")
                .unwrap_or_else(|_| "omni".to_string()),
            backfill_days: std::env::var("BACKFILL_DAYS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()?,
            aave_subgraph_arbitrum: std::env::var("AAVE_SUBGRAPH_ARBITRUM")
                .unwrap_or_else(|_| "https://api.thegraph.com/subgraphs/name/aave/protocol-v3-arbitrum".to_string()),
            aave_subgraph_base: std::env::var("AAVE_SUBGRAPH_BASE")
                .unwrap_or_else(|_| "https://api.thegraph.com/subgraphs/name/aave/protocol-v3-base".to_string()),
            kamino_api_url: std::env::var("KAMINO_API_URL")
                .unwrap_or_else(|_| "https://api.kamino.finance".to_string()),
            morpho_api_url: std::env::var("MORPHO_API_URL")
                .unwrap_or_else(|_| "https://api.morpho.org/graphql".to_string()),
            fluid_api_url: std::env::var("FLUID_API_URL")
                .unwrap_or_else(|_| "https://api.fluid.instadapp.io".to_string()),
            trongrid_api_key: std::env::var("TRONGRID_API_KEY").ok(),
            the_graph_api_key: std::env::var("THE_GRAPH_API_KEY").ok(),
            raydium_api_url: std::env::var("RAYDIUM_API_URL")
                .unwrap_or_else(|_| "https://api-v3.raydium.io".to_string()),
            cors_origins: std::env::var("CORS_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:5173".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            // Performance tuning
            max_concurrent_indexers: std::env::var("MAX_CONCURRENT_INDEXERS")
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,
            backfill_concurrency: std::env::var("BACKFILL_CONCURRENCY")
                .unwrap_or_else(|_| "25".to_string())
                .parse()?,
            indexer_timeout_secs: std::env::var("INDEXER_TIMEOUT_SECS")
                .unwrap_or_else(|_| "45".to_string())
                .parse()?,
            cb_failure_threshold: std::env::var("CB_FAILURE_THRESHOLD")
                .unwrap_or_else(|_| "3".to_string())
                .parse()?,
            cb_cooldown_secs: std::env::var("CB_COOLDOWN_SECS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()?,
        })
    }
}
