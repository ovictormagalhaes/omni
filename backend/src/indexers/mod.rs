use anyhow::Result;
use async_trait::async_trait;
use crate::models::{Chain, Protocol, ProtocolRate, PoolRate};

// ============================================================================
// Indexer traits — implement these for each protocol
// ============================================================================

/// Trait for lending/vault/staking protocol indexers
#[async_trait]
pub trait RateIndexer: Send + Sync {
    /// Which protocol this indexer serves
    fn protocol(&self) -> Protocol;

    /// Chains this indexer supports
    fn supported_chains(&self) -> Vec<Chain>;

    /// Fetch rates for a given chain
    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>>;

    /// Build a deep-link URL for a specific rate entry
    fn rate_url(&self, rate: &ProtocolRate) -> String;
}

/// Trait for DEX/LP pool indexers
#[async_trait]
pub trait PoolIndexer: Send + Sync {
    /// Which protocol this indexer serves
    fn protocol(&self) -> Protocol;

    /// Chains this indexer supports
    fn supported_chains(&self) -> Vec<Chain>;

    /// Fetch pools for a given chain
    async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>>;

    /// Build a deep-link URL for a specific pool entry
    fn pool_url(&self, pool: &PoolRate) -> String;
}

// ============================================================================
// Protocol modules
// ============================================================================

pub mod aave;
pub mod kamino;
pub mod morpho;
pub mod fluid;
pub mod sparklend;
pub mod justlend;
pub mod euler;
pub mod jupiter;
pub mod lido;
pub mod marinade;
pub mod jito;
pub mod rocketpool;
pub mod raydium;
pub mod uniswap_v3;
pub mod uniswap_v4;
pub mod compound;
pub mod venus;
pub mod pendle;
pub mod ethena;
pub mod etherfi;
pub mod curve;
pub mod defillama_pools;
pub mod pancakeswap;
pub mod aerodrome;
pub mod velodrome;
pub mod meteora;
pub mod orca;
pub mod benqi;
pub mod radiant;
pub mod sushiswap;
pub mod camelot;
pub mod traderjoe;
pub mod sky;
pub mod silo;
pub mod fraxeth;
pub mod balancer;
pub mod maverick;
pub mod aura;
pub mod convex;
pub mod yearn;
pub mod stargate;
pub mod gmx;

pub use aave::AaveIndexer;
pub use kamino::KaminoIndexer;
pub use morpho::MorphoIndexer;
pub use fluid::FluidIndexer;
pub use sparklend::SparkLendIndexer;
pub use justlend::JustLendIndexer;
pub use euler::EulerIndexer;
pub use jupiter::JupiterIndexer;
pub use lido::LidoIndexer;
pub use marinade::MarinadeIndexer;
pub use jito::JitoIndexer;
pub use rocketpool::RocketPoolIndexer;
pub use raydium::RaydiumIndexer;
pub use uniswap_v3::UniswapV3Indexer;
pub use uniswap_v4::UniswapV4Indexer;
pub use compound::CompoundIndexer;
pub use venus::VenusIndexer;
pub use pendle::PendleIndexer;
pub use ethena::EthenaIndexer;
pub use etherfi::EtherFiIndexer;
pub use curve::CurveIndexer;
pub use pancakeswap::PancakeSwapIndexer;
pub use aerodrome::AerodromeIndexer;
pub use velodrome::VelodromeIndexer;
pub use meteora::MeteoraIndexer;
pub use orca::OrcaIndexer;
pub use benqi::BenqiIndexer;
pub use radiant::RadiantIndexer;
pub use sushiswap::SushiSwapIndexer;
pub use camelot::CamelotIndexer;
pub use traderjoe::TraderJoeIndexer;
pub use sky::SkyIndexer;
pub use silo::SiloIndexer;
pub use fraxeth::FraxEthIndexer;
pub use balancer::BalancerIndexer;
pub use maverick::MaverickIndexer;
pub use aura::AuraIndexer;
pub use convex::ConvexIndexer;
pub use yearn::YearnIndexer;
pub use stargate::StargateIndexer;
pub use gmx::GmxIndexer;

#[cfg(test)]
mod aave_test;

#[cfg(test)]
mod kamino_test;

#[cfg(test)]
mod morpho_test;

#[cfg(test)]
mod fluid_test;
