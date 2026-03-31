use crate::models::{Chain, PoolRate, Protocol, ProtocolRate};
use anyhow::Result;
use async_trait::async_trait;

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
pub mod aerodrome;
pub mod aura;
pub mod balancer;
pub mod benqi;
pub mod camelot;
pub mod compound;
pub mod convex;
pub mod curve;
pub mod defillama_pools;
pub mod ethena;
pub mod etherfi;
pub mod euler;
pub mod fluid;
pub mod fraxeth;
pub mod gmx;
pub mod jito;
pub mod jupiter;
pub mod justlend;
pub mod kamino;
pub mod lido;
pub mod marinade;
pub mod maverick;
pub mod meteora;
pub mod morpho;
pub mod orca;
pub mod pancakeswap;
pub mod pendle;
pub mod radiant;
pub mod raydium;
pub mod rocketpool;
pub mod silo;
pub mod sky;
pub mod sparklend;
pub mod stargate;
pub mod sushiswap;
pub mod traderjoe;
pub mod uniswap_v3;
pub mod uniswap_v4;
pub mod velodrome;
pub mod venus;
pub mod yearn;

pub use aave::AaveIndexer;
pub use aerodrome::AerodromeIndexer;
pub use aura::AuraIndexer;
pub use balancer::BalancerIndexer;
pub use benqi::BenqiIndexer;
pub use camelot::CamelotIndexer;
pub use compound::CompoundIndexer;
pub use convex::ConvexIndexer;
pub use curve::CurveIndexer;
pub use ethena::EthenaIndexer;
pub use etherfi::EtherFiIndexer;
pub use euler::EulerIndexer;
pub use fluid::FluidIndexer;
pub use fraxeth::FraxEthIndexer;
pub use gmx::GmxIndexer;
pub use jito::JitoIndexer;
pub use jupiter::JupiterIndexer;
pub use justlend::JustLendIndexer;
pub use kamino::KaminoIndexer;
pub use lido::LidoIndexer;
pub use marinade::MarinadeIndexer;
pub use maverick::MaverickIndexer;
pub use meteora::MeteoraIndexer;
pub use morpho::MorphoIndexer;
pub use orca::OrcaIndexer;
pub use pancakeswap::PancakeSwapIndexer;
pub use pendle::PendleIndexer;
pub use radiant::RadiantIndexer;
pub use raydium::RaydiumIndexer;
pub use rocketpool::RocketPoolIndexer;
pub use silo::SiloIndexer;
pub use sky::SkyIndexer;
pub use sparklend::SparkLendIndexer;
pub use stargate::StargateIndexer;
pub use sushiswap::SushiSwapIndexer;
pub use traderjoe::TraderJoeIndexer;
pub use uniswap_v3::UniswapV3Indexer;
pub use uniswap_v4::UniswapV4Indexer;
pub use velodrome::VelodromeIndexer;
pub use venus::VenusIndexer;
pub use yearn::YearnIndexer;

#[cfg(test)]
mod aave_test;

#[cfg(test)]
mod kamino_test;

#[cfg(test)]
mod morpho_test;

#[cfg(test)]
mod fluid_test;
