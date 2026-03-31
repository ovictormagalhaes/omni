// Library exports for testing and potential reuse

pub mod config;
pub mod indexers;
pub mod models;
pub mod routes;
pub mod services;

pub use config::Config;
pub use services::{
    HistoricalDataService, PoolHistoricalService, PoolRealtimeService, RealtimeService,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub historical_service: HistoricalDataService,
    pub realtime_service: RealtimeService,
    pub pool_historical_service: PoolHistoricalService,
    pub pool_realtime_service: PoolRealtimeService,
}
