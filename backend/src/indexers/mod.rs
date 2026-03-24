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

#[cfg(test)]
mod aave_test;

#[cfg(test)]
mod kamino_test;

#[cfg(test)]
mod morpho_test;

#[cfg(test)]
mod fluid_test;
