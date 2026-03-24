// Library exports for testing and potential reuse

pub mod models;
pub mod indexers;
pub mod services;
pub mod routes;
pub mod config;

pub use config::Config;
pub use services::{HistoricalDataService, RealtimeService};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub historical_service: HistoricalDataService,
    pub realtime_service: RealtimeService,
}
