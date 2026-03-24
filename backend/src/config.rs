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
                .unwrap_or_else(|_| "Ff2GgEFBKbhPEBBtq4bTCTbicBWmPuMqmpCTfBMvHBiS".to_string()),
            aave_subgraph_base: std::env::var("AAVE_SUBGRAPH_BASE")
                .unwrap_or_else(|_| "GQFbb95cE6d8mV989mL5figjaGaKCQB3xqYrr1bRyXqF".to_string()),
            kamino_api_url: std::env::var("KAMINO_API_URL")
                .unwrap_or_else(|_| "https://api.kamino.finance".to_string()),
            morpho_api_url: std::env::var("MORPHO_API_URL")
                .unwrap_or_else(|_| "https://api.morpho.org/graphql".to_string()),
            fluid_api_url: std::env::var("FLUID_API_URL")
                .unwrap_or_else(|_| "https://api.fluid.instadapp.io".to_string()),
            trongrid_api_key: std::env::var("TRONGRID_API_KEY").ok(),
            the_graph_api_key: std::env::var("THE_GRAPH_API_KEY").ok(),
        })
    }
}
