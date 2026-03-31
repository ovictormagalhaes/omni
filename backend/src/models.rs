use bson;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Custom serializer for f64 with 5 decimal places
mod round_f64_5 {
    use serde::Serializer;

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
    #[serde(rename = "liquiditypool")]
    LiquidityPool,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum Protocol {
    #[serde(rename = "aave-v3")]
    Aave,
    #[serde(rename = "kamino")]
    Kamino,
    #[serde(rename = "morpho")]
    Morpho,
    #[serde(rename = "fluid")]
    Fluid,
    #[serde(rename = "sparklend")]
    SparkLend,
    #[serde(rename = "justlend")]
    JustLend,
    #[serde(rename = "euler")]
    Euler,
    #[serde(rename = "jupiter")]
    Jupiter,
    #[serde(rename = "lido")]
    Lido,
    #[serde(rename = "marinade")]
    Marinade,
    #[serde(rename = "jito")]
    Jito,
    #[serde(rename = "rocketpool")]
    RocketPool,
    #[serde(rename = "uniswap-v3")]
    Uniswap,
    #[serde(rename = "uniswap-v4")]
    UniswapV4,
    #[serde(rename = "raydium")]
    Raydium,
    #[serde(rename = "compound-v3")]
    Compound,
    #[serde(rename = "venus")]
    Venus,
    #[serde(rename = "pendle")]
    Pendle,
    #[serde(rename = "ethena")]
    Ethena,
    #[serde(rename = "etherfi")]
    EtherFi,
    #[serde(rename = "curve")]
    Curve,
    #[serde(rename = "pancakeswap")]
    PancakeSwap,
    #[serde(rename = "aerodrome")]
    Aerodrome,
    #[serde(rename = "velodrome")]
    Velodrome,
    #[serde(rename = "orca")]
    Orca,
    #[serde(rename = "meteora")]
    Meteora,
    #[serde(rename = "benqi")]
    Benqi,
    #[serde(rename = "radiant")]
    Radiant,
    #[serde(rename = "sushiswap")]
    SushiSwap,
    #[serde(rename = "camelot")]
    Camelot,
    #[serde(rename = "traderjoe")]
    TraderJoe,
    #[serde(rename = "sky")]
    Sky,
    #[serde(rename = "silo")]
    Silo,
    #[serde(rename = "fraxeth")]
    FraxEth,
    #[serde(rename = "balancer")]
    Balancer,
    #[serde(rename = "maverick")]
    Maverick,
    #[serde(rename = "aura")]
    Aura,
    #[serde(rename = "convex")]
    Convex,
    #[serde(rename = "yearn")]
    Yearn,
    #[serde(rename = "stargate")]
    Stargate,
    #[serde(rename = "gmx")]
    Gmx,
}

impl<'de> Deserialize<'de> for Protocol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let lower = s.to_lowercase();
        match lower.as_str() {
            "aave" | "aave-v3" => Ok(Protocol::Aave),
            "kamino" => Ok(Protocol::Kamino),
            "morpho" => Ok(Protocol::Morpho),
            "fluid" => Ok(Protocol::Fluid),
            "sparklend" => Ok(Protocol::SparkLend),
            "justlend" => Ok(Protocol::JustLend),
            "euler" => Ok(Protocol::Euler),
            "jupiter" => Ok(Protocol::Jupiter),
            "lido" => Ok(Protocol::Lido),
            "marinade" => Ok(Protocol::Marinade),
            "jito" => Ok(Protocol::Jito),
            "rocketpool" => Ok(Protocol::RocketPool),
            "uniswap" | "uniswap-v3" | "uniswapv3" => Ok(Protocol::Uniswap),
            "uniswap-v4" | "uniswapv4" => Ok(Protocol::UniswapV4),
            "raydium" => Ok(Protocol::Raydium),
            "compound" | "compound-v3" | "compoundv3" => Ok(Protocol::Compound),
            "venus" => Ok(Protocol::Venus),
            "pendle" => Ok(Protocol::Pendle),
            "ethena" => Ok(Protocol::Ethena),
            "etherfi" => Ok(Protocol::EtherFi),
            "curve" => Ok(Protocol::Curve),
            "pancakeswap" => Ok(Protocol::PancakeSwap),
            "aerodrome" => Ok(Protocol::Aerodrome),
            "velodrome" => Ok(Protocol::Velodrome),
            "orca" => Ok(Protocol::Orca),
            "meteora" => Ok(Protocol::Meteora),
            "benqi" => Ok(Protocol::Benqi),
            "radiant" => Ok(Protocol::Radiant),
            "sushiswap" => Ok(Protocol::SushiSwap),
            "camelot" => Ok(Protocol::Camelot),
            "traderjoe" => Ok(Protocol::TraderJoe),
            "sky" => Ok(Protocol::Sky),
            "silo" => Ok(Protocol::Silo),
            "fraxeth" => Ok(Protocol::FraxEth),
            "balancer" => Ok(Protocol::Balancer),
            "maverick" => Ok(Protocol::Maverick),
            "aura" => Ok(Protocol::Aura),
            "convex" => Ok(Protocol::Convex),
            "yearn" => Ok(Protocol::Yearn),
            "stargate" => Ok(Protocol::Stargate),
            "gmx" => Ok(Protocol::Gmx),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &[
                    "aave-v3",
                    "kamino",
                    "morpho",
                    "fluid",
                    "sparklend",
                    "justlend",
                    "euler",
                    "jupiter",
                    "lido",
                    "marinade",
                    "jito",
                    "rocketpool",
                    "uniswap-v3",
                    "uniswap-v4",
                    "raydium",
                    "compound-v3",
                    "venus",
                    "pendle",
                    "ethena",
                    "etherfi",
                    "curve",
                    "pancakeswap",
                    "aerodrome",
                    "velodrome",
                    "orca",
                    "meteora",
                    "benqi",
                    "radiant",
                    "sushiswap",
                    "camelot",
                    "traderjoe",
                    "sky",
                    "silo",
                    "fraxeth",
                    "balancer",
                    "maverick",
                    "aura",
                    "convex",
                    "yearn",
                    "stargate",
                    "gmx",
                ],
            )),
        }
    }
}

// Implement Display for Protocol to support formatting
impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Aave => write!(f, "aave-v3"),
            Protocol::Kamino => write!(f, "kamino"),
            Protocol::Morpho => write!(f, "morpho"),
            Protocol::Fluid => write!(f, "fluid"),
            Protocol::SparkLend => write!(f, "sparklend"),
            Protocol::JustLend => write!(f, "justlend"),
            Protocol::Euler => write!(f, "euler"),
            Protocol::Jupiter => write!(f, "jupiter"),
            Protocol::Lido => write!(f, "lido"),
            Protocol::Marinade => write!(f, "marinade"),
            Protocol::Jito => write!(f, "jito"),
            Protocol::RocketPool => write!(f, "rocketpool"),
            Protocol::Uniswap => write!(f, "uniswap-v3"),
            Protocol::UniswapV4 => write!(f, "uniswap-v4"),
            Protocol::Raydium => write!(f, "raydium"),
            Protocol::Compound => write!(f, "compound-v3"),
            Protocol::Venus => write!(f, "venus"),
            Protocol::Pendle => write!(f, "pendle"),
            Protocol::Ethena => write!(f, "ethena"),
            Protocol::EtherFi => write!(f, "etherfi"),
            Protocol::Curve => write!(f, "curve"),
            Protocol::PancakeSwap => write!(f, "pancakeswap"),
            Protocol::Aerodrome => write!(f, "aerodrome"),
            Protocol::Velodrome => write!(f, "velodrome"),
            Protocol::Orca => write!(f, "orca"),
            Protocol::Meteora => write!(f, "meteora"),
            Protocol::Benqi => write!(f, "benqi"),
            Protocol::Radiant => write!(f, "radiant"),
            Protocol::SushiSwap => write!(f, "sushiswap"),
            Protocol::Camelot => write!(f, "camelot"),
            Protocol::TraderJoe => write!(f, "traderjoe"),
            Protocol::Sky => write!(f, "sky"),
            Protocol::Silo => write!(f, "silo"),
            Protocol::FraxEth => write!(f, "fraxeth"),
            Protocol::Balancer => write!(f, "balancer"),
            Protocol::Maverick => write!(f, "maverick"),
            Protocol::Aura => write!(f, "aura"),
            Protocol::Convex => write!(f, "convex"),
            Protocol::Yearn => write!(f, "yearn"),
            Protocol::Stargate => write!(f, "stargate"),
            Protocol::Gmx => write!(f, "gmx"),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
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

impl<'de> Deserialize<'de> for Chain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let lower = s.to_lowercase();
        match lower.as_str() {
            "ethereum" => Ok(Chain::Ethereum),
            "solana" => Ok(Chain::Solana),
            "bsc" => Ok(Chain::BSC),
            "bitcoin" => Ok(Chain::Bitcoin),
            "tron" => Ok(Chain::Tron),
            "base" => Ok(Chain::Base),
            "arbitrum" => Ok(Chain::Arbitrum),
            "polygon" => Ok(Chain::Polygon),
            "optimism" => Ok(Chain::Optimism),
            "avalanche" => Ok(Chain::Avalanche),
            "sui" => Ok(Chain::Sui),
            "hyperliquid" => Ok(Chain::Hyperliquid),
            "scroll" => Ok(Chain::Scroll),
            "mantle" => Ok(Chain::Mantle),
            "linea" => Ok(Chain::Linea),
            "blast" => Ok(Chain::Blast),
            "fantom" => Ok(Chain::Fantom),
            "zksync" => Ok(Chain::ZkSync),
            "aptos" => Ok(Chain::Aptos),
            "celo" => Ok(Chain::Celo),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &[
                    "ethereum",
                    "solana",
                    "bsc",
                    "bitcoin",
                    "tron",
                    "base",
                    "arbitrum",
                    "polygon",
                    "optimism",
                    "avalanche",
                    "sui",
                    "hyperliquid",
                    "scroll",
                    "mantle",
                    "linea",
                    "blast",
                    "fantom",
                    "zksync",
                    "aptos",
                    "celo",
                ],
            )),
        }
    }
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
    USDE, // Ethena USD
    #[serde(rename = "SUSDE")]
    SUSDE, // Staked Ethena USD
    PYUSD, // PayPal USD
    FRAX, // Frax
    LUSD, // Liquity USD
    GHO,  // Aave GHO
    #[serde(rename = "CRVUSD")]
    CRVUSD, // Curve USD
    #[serde(rename = "USDD")]
    USDD, // Tron USDD

    // EUR Stablecoins
    EURC, // Circle EUR
    EURS, // STASIS EUR
    EURT, // Tether EUR

    // ETH and LSTs
    WETH,
    #[serde(rename = "ETH")]
    ETH,
    #[serde(rename = "STETH")]
    STETH, // Lido stETH
    WSTETH, // Lido Wrapped Staked ETH
    RETH,   // Rocket Pool ETH
    CBETH,  // Coinbase ETH
    #[serde(rename = "SETH2")]
    SETH2, // StakeWise ETH2
    #[serde(rename = "SFRXETH")]
    SFRXETH, // Staked Frax ETH

    // BTC
    WBTC,
    CBBTC, // Coinbase Wrapped BTC
    TBTC,  // Threshold BTC
    SBTC,  // Synth BTC

    // SOL and LSTs
    SOL,
    #[serde(rename = "STSOL")]
    STSOL, // Lido stSOL
    #[serde(rename = "MSOL")]
    MSOL, // Marinade mSOL
    #[serde(rename = "JITOSOL")]
    JITOSOL, // Jito JitoSOL
    #[serde(rename = "JUPSOL")]
    JUPSOL, // Jupiter JupSOL

    // Other
    TRX,  // Tron
    LINK, // Chainlink
    AAVE, // Aave token
    UNI,  // Uniswap
    CRV,  // Curve
    BAL,  // Balancer
    COMP, // Compound
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
            Asset::Unknown(symbol) => Self::infer_category_from_symbol(symbol),
        }
    }

    /// Heuristic fallback: infer asset category from symbol name patterns.
    /// Covers tokens like USDC.e, wstETH, cbBTC, etc. that aren't in KnownAsset.
    fn infer_category_from_symbol(symbol: &str) -> Vec<AssetCategory> {
        let s = symbol.to_uppercase();

        // USD / Stablecoin patterns
        if s.contains("USD")
            || s.contains("DAI")
            || s.contains("FRAX")
            || s.contains("LUSD")
            || s.contains("GHO")
            || s.contains("TUSD")
            || s.contains("BUSD")
            || s.contains("GUSD")
            || s.contains("DOLA")
            || s.contains("MIM")
            || s.contains("USDX")
            || s.contains("SUSD")
            || s.contains("CUSD")
            || s.contains("MUSD")
            || s.contains("EUSD")
            || s.contains("USDB")
            || s == "FDUSD"
            || s == "USDP"
        {
            // Top stables get both tags, others just Stablecoin
            if s.contains("USDC") || s.contains("USDT") {
                return vec![AssetCategory::UsdCorrelated, AssetCategory::Stablecoin];
            }
            return vec![AssetCategory::Stablecoin];
        }

        // BTC patterns
        if s.contains("BTC")
            || s.contains("SBTC")
            || s.contains("RENBTC")
            || s.contains("TBTC")
            || s == "BBTC"
            || s.contains("PBTC")
        {
            return vec![AssetCategory::BtcCorrelated];
        }

        // ETH patterns
        if s.contains("ETH")
            || s.contains("STETH")
            || s.contains("SETH")
            || s.contains("RETH")
            || s.contains("ANKRETH")
            || s.contains("SWETH")
            || s.contains("OETH")
            || s.contains("METH")
            || s.contains("EETH")
            || s.contains("WEETH")
            || s.contains("EZETH")
        {
            return vec![AssetCategory::EthCorrelated];
        }

        // SOL patterns
        if s.contains("SOL")
            || s.contains("MSOL")
            || s.contains("JITOSOL")
            || s.contains("JUPSOL")
            || s.contains("BSOL")
            || s.contains("HSOL")
            || s.contains("VSOL")
            || s.contains("STSOL")
            || s.contains("DSOL")
            || s.contains("INF") && s.contains("SOL")
        {
            return vec![AssetCategory::SolCorrelated];
        }

        vec![]
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
            "WSOL" | "SOL" => Some(KnownAsset::SOL),
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
                tracing::warn!(
                    "[{}] Unknown asset detected: '{}' - adding as Unknown",
                    protocol,
                    symbol
                );
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
            DAI | USDE | SUSDE | PYUSD | FRAX | LUSD | GHO | CRVUSD | USDD | EURC | EURS | EURT => {
                vec![AssetCategory::Stablecoin]
            }

            // BTC Correlated
            WBTC | CBBTC | TBTC | SBTC => vec![AssetCategory::BtcCorrelated],

            // ETH Correlated
            ETH | WETH | STETH | WSTETH | RETH | CBETH | SETH2 | SFRXETH => {
                vec![AssetCategory::EthCorrelated]
            }

            // SOL Correlated
            SOL | STSOL | MSOL | JITOSOL | JUPSOL => vec![AssetCategory::SolCorrelated],

            // Other tokens don't belong to any filter category
            TRX | LINK | AAVE | UNI | CRV | BAL | COMP => vec![],
        }
    }
}

impl Asset {
    /// Return canonical "family" name for cross-chain pair normalization.
    /// WBTC, CBBTC, tBTC → "BTC"; USDC, USDT → "USD"; etc.
    pub fn canonical_name(&self) -> String {
        match self {
            Asset::Known(k) => match k {
                KnownAsset::WBTC | KnownAsset::CBBTC | KnownAsset::TBTC | KnownAsset::SBTC => {
                    "BTC".to_string()
                }
                KnownAsset::ETH | KnownAsset::WETH => "ETH".to_string(),
                KnownAsset::STETH
                | KnownAsset::WSTETH
                | KnownAsset::RETH
                | KnownAsset::CBETH
                | KnownAsset::SETH2
                | KnownAsset::SFRXETH => "LST-ETH".to_string(),
                KnownAsset::USDC | KnownAsset::USDT => "USD".to_string(),
                KnownAsset::DAI
                | KnownAsset::USDE
                | KnownAsset::SUSDE
                | KnownAsset::PYUSD
                | KnownAsset::FRAX
                | KnownAsset::LUSD
                | KnownAsset::GHO
                | KnownAsset::CRVUSD
                | KnownAsset::USDD => "STABLE".to_string(),
                KnownAsset::EURC | KnownAsset::EURS | KnownAsset::EURT => "EUR".to_string(),
                KnownAsset::SOL => "SOL".to_string(),
                KnownAsset::MSOL | KnownAsset::JITOSOL | KnownAsset::JUPSOL | KnownAsset::STSOL => {
                    "LST-SOL".to_string()
                }
                other => format!("{:?}", other).to_uppercase(),
            },
            Asset::Unknown(s) => s.to_uppercase(),
        }
    }
}

/// Generate a normalized pair string for cross-chain comparison.
/// Always in alphabetical canonical order: "CBBTC/USDC" → "BTC/USD", "USDC/WETH" → "ETH/USD"
pub fn normalize_pair(token0: &Asset, token1: &Asset) -> String {
    let mut names = [token0.canonical_name(), token1.canonical_name()];
    names.sort();
    format!("{}/{}", names[0], names[1])
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
    pub protocols: Option<String>,
    #[serde(default)]
    pub operation_types: Option<String>,
    #[serde(default)]
    pub asset_categories: Option<String>,
    /// Search by token/asset symbol (substring match)
    #[serde(default)]
    pub token: Option<String>,
    /// Minimum liquidity in USD (default: 1000000)
    #[serde(default = "default_min_liquidity")]
    pub min_liquidity: u64,
    /// Page number (1-indexed, default: 1)
    #[serde(default = "default_page")]
    pub page: u64,
    /// Results per page (default: 20)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_min_liquidity() -> u64 {
    1_000_000
}

fn default_page() -> u64 {
    1
}

fn default_page_size() -> u64 {
    20
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
    pub page: u64,
    #[serde(rename = "pageSize")]
    pub page_size: u64,
    #[serde(rename = "totalCount")]
    pub total_count: usize,
    #[serde(rename = "totalPages")]
    pub total_pages: u64,
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
    pub vault_name: Option<String>,       // Human-readable vault name
    pub underlying_asset: Option<String>, // Token contract address
    pub timestamp: DateTime<Utc>,
}

// ============================================================================
// LIQUIDITY POOL MODELS (for DEX pool comparison)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PoolType {
    Standard,
    #[serde(rename = "concentrated")]
    ConcentratedLiquidity,
}

impl std::fmt::Display for PoolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolType::Standard => write!(f, "Standard"),
            PoolType::ConcentratedLiquidity => write!(f, "Concentrated"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FeeTier {
    #[serde(rename = "0.01%")]
    Bps1,
    #[serde(rename = "0.05%")]
    Bps5,
    #[serde(rename = "0.30%")]
    Bps30,
    #[serde(rename = "1.00%")]
    Bps100,
    Custom(u32),
}

impl FeeTier {
    pub fn from_bps(bps: u32) -> Self {
        match bps {
            1 => FeeTier::Bps1,
            5 => FeeTier::Bps5,
            30 => FeeTier::Bps30,
            100 => FeeTier::Bps100,
            other => FeeTier::Custom(other),
        }
    }

    /// Convert Uniswap V3 fee tier (100, 500, 3000, 10000) to FeeTier
    pub fn from_uniswap_fee(fee: u32) -> Self {
        match fee {
            100 => FeeTier::Bps1,
            500 => FeeTier::Bps5,
            3000 => FeeTier::Bps30,
            10000 => FeeTier::Bps100,
            other => FeeTier::Custom(other / 100),
        }
    }

    pub fn to_bps(&self) -> u32 {
        match self {
            FeeTier::Bps1 => 1,
            FeeTier::Bps5 => 5,
            FeeTier::Bps30 => 30,
            FeeTier::Bps100 => 100,
            FeeTier::Custom(bps) => *bps,
        }
    }

    pub fn display(&self) -> String {
        let bps = self.to_bps();
        if bps < 100 {
            format!("0.{:02}%", bps)
        } else {
            format!("{}.{:02}%", bps / 100, bps % 100)
        }
    }
}

/// Internal pool rate format (output from pool indexers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolRate {
    pub protocol: Protocol,
    pub chain: Chain,
    pub token0: Asset,
    pub token1: Asset,
    pub pool_type: PoolType,
    pub fee_tier: FeeTier,
    pub fee_rate_bps: u32,
    pub tvl_usd: f64,
    pub volume_24h_usd: f64,
    pub volume_7d_usd: f64,
    pub fees_24h_usd: f64,
    pub fees_7d_usd: f64,
    pub fee_apr_24h: f64,
    pub fee_apr_7d: f64,
    pub rewards_apr: f64,
    pub pool_address: String,
    pub pool_id: Option<String>,
    pub active: bool,
    pub timestamp: DateTime<Utc>,
}

/// API response model for a single pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolResult {
    pub protocol: Protocol,
    pub chain: Chain,
    pub token0: String,
    pub token1: String,
    pub pair: String,
    #[serde(rename = "normalizedPair")]
    pub normalized_pair: String,
    #[serde(rename = "token0Categories")]
    pub token0_categories: Vec<AssetCategory>,
    #[serde(rename = "token1Categories")]
    pub token1_categories: Vec<AssetCategory>,
    #[serde(rename = "poolType")]
    pub pool_type: PoolType,
    #[serde(rename = "feeTier")]
    pub fee_tier: String,
    #[serde(rename = "feeRateBps")]
    pub fee_rate_bps: u32,
    #[serde(rename = "tvlUsd")]
    pub tvl_usd: f64,
    #[serde(rename = "volume24h")]
    pub volume_24h_usd: f64,
    #[serde(rename = "volume7d")]
    pub volume_7d_usd: f64,
    /// Turnover Ratio 24h = Volume 24h / TVL (how many times the pool turns over daily)
    #[serde(rename = "turnoverRatio24h", serialize_with = "round_f64_5::serialize")]
    pub turnover_ratio_24h: f64,
    /// Turnover Ratio 7d (daily avg) = (Volume 7d / 7) / TVL
    #[serde(rename = "turnoverRatio7d", serialize_with = "round_f64_5::serialize")]
    pub turnover_ratio_7d: f64,
    /// Fee APR 24h = Turnover Ratio 24h × Fee Rate × 365 × 100
    #[serde(rename = "fees24h")]
    pub fees_24h_usd: f64,
    #[serde(rename = "fees7d")]
    pub fees_7d_usd: f64,
    #[serde(rename = "feeApr24h", serialize_with = "round_f64_5::serialize")]
    pub fee_apr_24h: f64,
    /// Fee APR 7d = Turnover Ratio 7d × Fee Rate × 365 × 100
    #[serde(rename = "feeApr7d", serialize_with = "round_f64_5::serialize")]
    pub fee_apr_7d: f64,
    #[serde(rename = "rewardsApr", serialize_with = "round_f64_5::serialize")]
    pub rewards_apr: f64,
    #[serde(rename = "totalApr", serialize_with = "round_f64_5::serialize")]
    pub total_apr: f64,
    #[serde(rename = "poolAddress")]
    pub pool_address: String,
    pub url: String,
    #[serde(rename = "lastUpdate")]
    pub last_update: DateTime<Utc>,
    #[serde(rename = "poolVaultId")]
    pub pool_vault_id: String,
}

/// Query parameters for pool search
#[derive(Debug, Deserialize)]
pub struct PoolQuery {
    /// Filter by asset category for token side 0 (e.g. "btc-correlated")
    #[serde(default)]
    pub asset_categories_0: Option<String>,
    /// Filter by asset category for token side 1 (e.g. "usd-correlated")
    #[serde(default)]
    pub asset_categories_1: Option<String>,
    /// Search by token symbol (substring match on token0)
    #[serde(default)]
    pub token_a: Option<String>,
    /// Search by token symbol (substring match on token1)
    #[serde(default)]
    pub token_b: Option<String>,
    /// Search by token symbol (substring match on either side of pair) — legacy
    #[serde(default)]
    pub token: Option<String>,
    /// Search by exact pair like "ETH/USDC"
    #[serde(default)]
    pub pair: Option<String>,
    #[serde(default)]
    pub chains: Option<String>,
    #[serde(default)]
    pub protocols: Option<String>,
    /// Filter by pool type: "concentrated" or "standard"
    #[serde(default)]
    pub pool_type: Option<String>,
    /// Minimum TVL in USD (default: 10000)
    #[serde(default = "default_pool_min_tvl")]
    pub min_tvl: u64,
    /// Minimum 24h volume in USD (default: 0 = no filter)
    #[serde(default)]
    pub min_volume: u64,
    /// Filter by normalized pair (e.g., "BTC/ETH", "SOL/USD")
    #[serde(default)]
    pub normalized_pair: Option<String>,
    /// Page number (1-indexed, default: 1)
    #[serde(default = "default_page")]
    pub page: u64,
    /// Results per page (default: 20)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_pool_min_tvl() -> u64 {
    100_000
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl PoolQuery {
    fn parse_categories(raw: &Option<String>) -> Option<Vec<AssetCategory>> {
        raw.as_ref().map(|s| {
            s.split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .filter_map(|item| serde_json::from_str(&format!("\"{}\"", item)).ok())
                .collect()
        })
    }

    pub fn parse_asset_categories_0(&self) -> Option<Vec<AssetCategory>> {
        Self::parse_categories(&self.asset_categories_0)
    }

    pub fn parse_asset_categories_1(&self) -> Option<Vec<AssetCategory>> {
        Self::parse_categories(&self.asset_categories_1)
    }

    pub fn parse_chains(&self) -> Option<Vec<Chain>> {
        self.chains.as_ref().map(|s| {
            s.split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .filter_map(|item| serde_json::from_str(&format!("\"{}\"", item)).ok())
                .collect()
        })
    }

    pub fn parse_protocols(&self) -> Option<Vec<Protocol>> {
        self.protocols.as_ref().map(|s| {
            s.split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .filter_map(|item| serde_json::from_str(&format!("\"{}\"", item)).ok())
                .collect()
        })
    }

    pub fn parse_pool_type(&self) -> Option<PoolType> {
        self.pool_type
            .as_ref()
            .and_then(|s| serde_json::from_str(&format!("\"{}\"", s.trim())).ok())
    }
}

/// API response for pool search
#[derive(Debug, Serialize)]
pub struct PoolResponse {
    pub success: bool,
    pub timestamp: DateTime<Utc>,
    pub results: Vec<PoolResult>,
    pub count: usize,
    pub page: u64,
    #[serde(rename = "pageSize")]
    pub page_size: u64,
    #[serde(rename = "totalCount")]
    pub total_count: usize,
    #[serde(rename = "totalPages")]
    pub total_pages: u64,
}

/// Convert PoolRate to PoolResult
impl PoolRate {
    pub fn to_result(&self, url: String) -> PoolResult {
        let pool_vault_id = PoolSnapshot::generate_pool_vault_id(
            &self.protocol,
            &self.chain,
            &self.token0.symbol(),
            &self.token1.symbol(),
            self.fee_rate_bps,
            &self.pool_address,
        );
        let pair = format!("{}/{}", self.token0.symbol(), self.token1.symbol());
        let normalized_pair = normalize_pair(&self.token0, &self.token1);
        // Turnover Ratio = Volume / TVL (daily)
        let turnover_ratio_24h = if self.tvl_usd > 0.0 {
            self.volume_24h_usd / self.tvl_usd
        } else {
            0.0
        };
        let turnover_ratio_7d = if self.tvl_usd > 0.0 && self.volume_7d_usd > 0.0 {
            (self.volume_7d_usd / 7.0) / self.tvl_usd
        } else {
            0.0
        };

        // Use fee APR from indexer (may use DeFiLlama apyBase or manual calculation)
        let fee_apr_24h = self.fee_apr_24h;
        let fee_apr_7d = self.fee_apr_7d;

        PoolResult {
            protocol: self.protocol.clone(),
            chain: self.chain.clone(),
            token0: self.token0.symbol(),
            token1: self.token1.symbol(),
            pair,
            normalized_pair,
            token0_categories: self.token0.category(),
            token1_categories: self.token1.category(),
            pool_type: self.pool_type.clone(),
            fee_tier: self.fee_tier.display(),
            fee_rate_bps: self.fee_rate_bps,
            tvl_usd: self.tvl_usd,
            volume_24h_usd: self.volume_24h_usd,
            volume_7d_usd: self.volume_7d_usd,
            turnover_ratio_24h,
            turnover_ratio_7d,
            fees_24h_usd: self.fees_24h_usd,
            fees_7d_usd: self.fees_7d_usd,
            fee_apr_24h,
            fee_apr_7d,
            rewards_apr: self.rewards_apr,
            total_apr: fee_apr_24h + self.rewards_apr,
            pool_address: self.pool_address.clone(),
            url,
            last_update: self.timestamp,
            pool_vault_id,
        }
    }
}

/// Daily snapshot of a liquidity pool (for MongoDB storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSnapshot {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<bson::oid::ObjectId>,

    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub date: DateTime<Utc>,

    pub pool_vault_id: String,
    pub protocol: Protocol,
    pub chain: Chain,
    pub token0: String,
    pub token1: String,
    pub pair: String,
    pub normalized_pair: String,
    pub pool_type: PoolType,
    pub fee_rate_bps: u32,
    pub tvl_usd: f64,
    pub volume_24h_usd: f64,
    pub fees_24h_usd: f64,
    pub turnover_ratio_24h: f64,
    pub fee_apr_24h: f64,
    pub fee_apr_7d: f64,
    pub rewards_apr: f64,
    pub url: String,

    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub collected_at: DateTime<Utc>,
}

impl PoolSnapshot {
    pub fn from_pool_result(result: &PoolResult, date: DateTime<Utc>) -> Self {
        PoolSnapshot {
            id: None,
            date,
            pool_vault_id: result.pool_vault_id.clone(),
            protocol: result.protocol.clone(),
            chain: result.chain.clone(),
            token0: result.token0.clone(),
            token1: result.token1.clone(),
            pair: result.pair.clone(),
            normalized_pair: result.normalized_pair.clone(),
            pool_type: result.pool_type.clone(),
            fee_rate_bps: result.fee_rate_bps,
            tvl_usd: result.tvl_usd,
            volume_24h_usd: result.volume_24h_usd,
            fees_24h_usd: result.fees_24h_usd,
            turnover_ratio_24h: result.turnover_ratio_24h,
            fee_apr_24h: result.fee_apr_24h,
            fee_apr_7d: result.fee_apr_7d,
            rewards_apr: result.rewards_apr,
            url: result.url.clone(),
            collected_at: chrono::Utc::now(),
        }
    }

    /// Generate deterministic pool vault ID
    pub fn generate_pool_vault_id(
        protocol: &Protocol,
        chain: &Chain,
        token0: &str,
        token1: &str,
        fee_rate_bps: u32,
        pool_address: &str,
    ) -> String {
        use sha2::{Digest, Sha256};
        let key = format!(
            "pool|{}|{}|{}|{}|{}|{}",
            format!("{:?}", protocol).to_lowercase(),
            format!("{:?}", chain).to_lowercase(),
            token0.to_uppercase(),
            token1.to_uppercase(),
            fee_rate_bps,
            pool_address.to_lowercase(),
        );
        let hash = Sha256::digest(key.as_bytes());
        format!("{:x}", hash)[..16].to_string()
    }
}

/// Response for pool history endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolHistoryResponse {
    pub success: bool,
    pub pool_vault_id: String,
    pub pair: Option<String>,
    pub protocol: Option<Protocol>,
    pub chain: Option<Chain>,
    pub url: Option<String>,
    pub days: u32,
    pub points: Vec<PoolHistoryPoint>,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub avg_fee_apr: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub min_fee_apr: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub max_fee_apr: f64,
    pub avg_tvl: f64,
    pub data_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolHistoryPoint {
    pub date: DateTime<Utc>,
    pub tvl_usd: f64,
    pub volume_24h_usd: f64,
    pub fee_rate_bps: u32,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub turnover_ratio_24h: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub fee_apr_24h: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub fee_apr_7d: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub rewards_apr: f64,
}

/// Real-time consolidated pool data (1 doc per pool)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimePool {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<bson::oid::ObjectId>,
    pub pool_vault_id: String,
    pub protocol: Protocol,
    pub chain: Chain,
    pub token0: String,
    pub token1: String,
    pub pair: String,
    pub normalized_pair: String,
    pub pool_type: PoolType,
    pub fee_rate_bps: u32,
    pub url: String,
    pub current: CurrentPoolData,
    pub fee_apr_metrics: PoolAprMetrics,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: DateTime<Utc>,
    pub snapshot_count: i32,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub first_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentPoolData {
    pub tvl_usd: f64,
    pub volume_24h_usd: f64,
    pub volume_7d_usd: f64,
    pub fees_24h_usd: f64,
    #[serde(default)]
    pub fees_7d_usd: f64,
    pub turnover_ratio_24h: f64,
    pub turnover_ratio_7d: f64,
    pub fee_apr_24h: f64,
    pub fee_apr_7d: f64,
    pub rewards_apr: f64,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub collected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolAprMetrics {
    pub instant: f64,
    pub apr_7d: f64,
    pub apr_30d: f64,
    pub volatility: f64,
    pub days_with_data: i32,
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
            Protocol::Uniswap,
            Protocol::UniswapV4,
            Protocol::Raydium,
            Protocol::Compound,
            Protocol::Venus,
            Protocol::Pendle,
            Protocol::Ethena,
            Protocol::EtherFi,
            Protocol::Curve,
            Protocol::PancakeSwap,
            Protocol::Aerodrome,
            Protocol::Velodrome,
            Protocol::Orca,
            Protocol::Meteora,
            Protocol::Benqi,
            Protocol::Radiant,
            Protocol::SushiSwap,
            Protocol::Camelot,
            Protocol::TraderJoe,
            Protocol::Sky,
            Protocol::Silo,
            Protocol::FraxEth,
            Protocol::Balancer,
            Protocol::Maverick,
            Protocol::Aura,
            Protocol::Convex,
            Protocol::Yearn,
            Protocol::Stargate,
            Protocol::Gmx,
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
        use sha2::{Digest, Sha256};
        let action_str = action
            .map(|a| format!("{:?}", a).to_lowercase())
            .unwrap_or_default();
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

// ============================================================================
// SCORE MODELS (for pool/lending position scoring)
// ============================================================================

/// Request for pool score analysis
#[derive(Debug, Deserialize)]
pub struct PoolScoreRequest {
    /// First token symbol (e.g., "CBBTC")
    pub token0: String,
    /// Second token symbol (e.g., "WETH")
    pub token1: String,
    /// Protocol where the user's pool is (e.g., "uniswap")
    pub protocol: Option<Protocol>,
    /// Chain where the user's pool is (e.g., "base")
    pub chain: Option<Chain>,
    /// Fee tier in basis points (e.g., 5 = 0.05%, 30 = 0.30%, 100 = 1%)
    pub fee_tier: Option<u32>,
    /// Minimum TVL filter in USD (default: 10000)
    #[serde(default = "default_pool_min_tvl")]
    pub min_tvl: u64,
}

/// Request for lending score analysis
#[derive(Debug, Deserialize)]
pub struct LendingScoreRequest {
    pub supplies: Vec<ScoreAsset>,
    pub borrows: Vec<ScoreAsset>,
    pub protocol: Option<Protocol>,
    pub chain: Option<Chain>,
    #[serde(default = "default_min_liquidity")]
    pub min_liquidity: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScoreAsset {
    pub token: String,
    #[serde(default)]
    pub value: f64,
}

/// A scored pool suggestion
#[derive(Debug, Clone, Serialize)]
pub struct PoolScoreSuggestion {
    pub rank: usize,
    pub protocol: Protocol,
    pub chain: Chain,
    pub token0: String,
    pub token1: String,
    pub pair: String,
    #[serde(rename = "normalizedPair")]
    pub normalized_pair: String,
    #[serde(rename = "feeTier")]
    pub fee_tier: String,
    #[serde(rename = "feeRateBps")]
    pub fee_rate_bps: u32,
    #[serde(rename = "tvlUsd")]
    pub tvl_usd: f64,
    #[serde(rename = "volume24h")]
    pub volume_24h_usd: f64,
    #[serde(rename = "turnoverRatio24h", serialize_with = "round_f64_5::serialize")]
    pub turnover_ratio_24h: f64,
    #[serde(rename = "feeApr24h", serialize_with = "round_f64_5::serialize")]
    pub fee_apr_24h: f64,
    #[serde(rename = "feeApr7d", serialize_with = "round_f64_5::serialize")]
    pub fee_apr_7d: f64,
    #[serde(rename = "totalApr", serialize_with = "round_f64_5::serialize")]
    pub total_apr: f64,
    #[serde(rename = "poolType")]
    pub pool_type: PoolType,
    pub url: String,
    #[serde(rename = "poolVaultId")]
    pub pool_vault_id: String,
}

/// Response for pool score endpoint
#[derive(Debug, Serialize)]
pub struct PoolScoreResponse {
    pub success: bool,
    pub timestamp: DateTime<Utc>,
    /// The user's current pool info (null if not found in our data)
    #[serde(rename = "current")]
    pub your_pool: Option<PoolScoreSuggestion>,
    /// Rank of the user's pool among comparable pools (null if not found)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<usize>,
    /// Total number of comparable pools found
    #[serde(rename = "totalComparable")]
    pub total_comparable: usize,
    /// Normalized pair used for comparison (e.g., "BTC/ETH")
    #[serde(rename = "normalizedPair")]
    pub normalized_pair: String,
    /// Token categories detected
    #[serde(rename = "token0Category")]
    pub token0_category: String,
    #[serde(rename = "token1Category")]
    pub token1_category: String,
    /// Top 3 suggestions sorted by total APR
    pub suggestions: Vec<PoolScoreSuggestion>,
}

/// A single asset rate within a lending suggestion
#[derive(Debug, Clone, Serialize)]
pub struct LendingAssetRate {
    pub asset: String,
    #[serde(rename = "assetCategory")]
    pub asset_category: String,
    pub action: Action,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub apy: f64,
    #[serde(serialize_with = "round_f64_5::serialize")]
    pub rewards: f64,
    #[serde(rename = "netApy", serialize_with = "round_f64_5::serialize")]
    pub net_apy: f64,
    /// Effective APY = netApy weighted by position size (netApy * valueUsd / totalValue)
    #[serde(
        rename = "effectiveApy",
        serialize_with = "round_f64_5::serialize",
        skip_serializing_if = "is_zero"
    )]
    pub effective_apy: f64,
    pub liquidity: u64,
    #[serde(rename = "valueUsd", skip_serializing_if = "is_zero")]
    pub value_usd: f64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
}

/// A scored lending suggestion (protocol + chain combination)
#[derive(Debug, Clone, Serialize)]
pub struct LendingScoreSuggestion {
    pub rank: usize,
    pub protocol: Protocol,
    pub chain: Chain,
    #[serde(rename = "supplyRates")]
    pub supply_rates: Vec<LendingAssetRate>,
    #[serde(rename = "borrowRates")]
    pub borrow_rates: Vec<LendingAssetRate>,
    #[serde(rename = "combinedNetApy", serialize_with = "round_f64_5::serialize")]
    pub combined_net_apy: f64,
    #[serde(rename = "assetsMatched")]
    pub assets_matched: usize,
    #[serde(rename = "assetsTotal")]
    pub assets_total: usize,
}

/// Response for lending score endpoint
#[derive(Debug, Serialize)]
pub struct LendingScoreResponse {
    pub success: bool,
    pub timestamp: DateTime<Utc>,
    /// The user's current position score (null if protocol/chain not specified or not found)
    #[serde(rename = "yourPosition", skip_serializing_if = "Option::is_none")]
    pub your_position: Option<LendingScoreSuggestion>,
    /// Rank of user's position (null if not found)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<usize>,
    /// Total comparable protocol+chain combinations
    #[serde(rename = "totalComparable")]
    pub total_comparable: usize,
    /// Asset categories detected
    #[serde(rename = "assetCategories")]
    pub asset_categories: std::collections::HashMap<String, String>,
    /// Top 3 suggestions
    pub suggestions: Vec<LendingScoreSuggestion>,
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

    /// Breakdown by protocol+chain (lending/vault)
    #[serde(rename = "protocolBreakdown")]
    pub protocol_breakdown: Vec<ProtocolStats>,

    /// Breakdown by protocol+chain (LP pools)
    #[serde(rename = "poolBreakdown")]
    pub pool_breakdown: Vec<PoolCollectionStats>,

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

/// Per-protocol+chain stats for lending/vault collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStats {
    pub protocol: Protocol,
    pub chain: Chain,

    /// Vaults returned by the indexer
    #[serde(rename = "vaultsFound")]
    pub vaults_found: usize,

    /// Vaults actually saved (after event-change filtering)
    #[serde(rename = "vaultsSaved")]
    pub vaults_saved: usize,

    /// Execution time for this indexer task (ms)
    #[serde(rename = "executionTimeMs")]
    pub execution_time_ms: i64,

    /// Error if this indexer task failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Per-protocol+chain stats for LP pool collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCollectionStats {
    pub protocol: Protocol,
    pub chain: Chain,

    /// Pools returned by the indexer
    #[serde(rename = "poolsFound")]
    pub pools_found: usize,

    /// Pools actually saved
    #[serde(rename = "poolsSaved")]
    pub pools_saved: usize,

    /// Execution time for this indexer task (ms)
    #[serde(rename = "executionTimeMs")]
    pub execution_time_ms: i64,

    /// Error if this indexer task failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Protocol serialization / deserialization
    // ========================================================================

    #[test]
    fn test_protocol_serialize_rename() {
        assert_eq!(
            serde_json::to_string(&Protocol::Aave).unwrap(),
            "\"aave-v3\""
        );
        assert_eq!(
            serde_json::to_string(&Protocol::Uniswap).unwrap(),
            "\"uniswap-v3\""
        );
        assert_eq!(
            serde_json::to_string(&Protocol::UniswapV4).unwrap(),
            "\"uniswap-v4\""
        );
        assert_eq!(
            serde_json::to_string(&Protocol::Compound).unwrap(),
            "\"compound-v3\""
        );
        assert_eq!(
            serde_json::to_string(&Protocol::Morpho).unwrap(),
            "\"morpho\""
        );
    }

    #[test]
    fn test_protocol_deserialize_aliases() {
        // "aave" and "aave-v3" should both map to Aave
        let p: Protocol = serde_json::from_str("\"aave\"").unwrap();
        assert_eq!(p, Protocol::Aave);
        let p: Protocol = serde_json::from_str("\"aave-v3\"").unwrap();
        assert_eq!(p, Protocol::Aave);

        // Uniswap aliases
        let p: Protocol = serde_json::from_str("\"uniswap\"").unwrap();
        assert_eq!(p, Protocol::Uniswap);
        let p: Protocol = serde_json::from_str("\"uniswap-v3\"").unwrap();
        assert_eq!(p, Protocol::Uniswap);
        let p: Protocol = serde_json::from_str("\"uniswapv3\"").unwrap();
        assert_eq!(p, Protocol::Uniswap);

        // Compound aliases
        let p: Protocol = serde_json::from_str("\"compound\"").unwrap();
        assert_eq!(p, Protocol::Compound);
        let p: Protocol = serde_json::from_str("\"compound-v3\"").unwrap();
        assert_eq!(p, Protocol::Compound);
    }

    #[test]
    fn test_protocol_deserialize_case_insensitive() {
        let p: Protocol = serde_json::from_str("\"AAVE\"").unwrap();
        assert_eq!(p, Protocol::Aave);
        let p: Protocol = serde_json::from_str("\"Morpho\"").unwrap();
        assert_eq!(p, Protocol::Morpho);
    }

    #[test]
    fn test_protocol_deserialize_unknown_variant() {
        let result: Result<Protocol, _> = serde_json::from_str("\"not_a_protocol\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_protocol_display_roundtrip() {
        for protocol in Protocol::all() {
            let display = format!("{}", protocol);
            let deserialized: Protocol = serde_json::from_str(&format!("\"{}\"", display)).unwrap();
            assert_eq!(
                protocol, deserialized,
                "Roundtrip failed for {:?}",
                protocol
            );
        }
    }

    // ========================================================================
    // Chain serialization / deserialization
    // ========================================================================

    #[test]
    fn test_chain_deserialize() {
        let c: Chain = serde_json::from_str("\"ethereum\"").unwrap();
        assert_eq!(c, Chain::Ethereum);
        let c: Chain = serde_json::from_str("\"bsc\"").unwrap();
        assert_eq!(c, Chain::BSC);
        let c: Chain = serde_json::from_str("\"zksync\"").unwrap();
        assert_eq!(c, Chain::ZkSync);
    }

    #[test]
    fn test_chain_serialize() {
        assert_eq!(
            serde_json::to_string(&Chain::Ethereum).unwrap(),
            "\"ethereum\""
        );
        assert_eq!(serde_json::to_string(&Chain::BSC).unwrap(), "\"bsc\"");
        assert_eq!(serde_json::to_string(&Chain::ZkSync).unwrap(), "\"zksync\"");
    }

    #[test]
    fn test_chain_all_count() {
        assert_eq!(Chain::all().len(), 20);
    }

    #[test]
    fn test_protocol_all_count() {
        assert_eq!(Protocol::all().len(), 41);
    }

    // ========================================================================
    // Asset categorization
    // ========================================================================

    #[test]
    fn test_asset_from_symbol_known() {
        let asset = Asset::from_symbol("USDC", "test");
        assert_eq!(asset, Asset::Known(KnownAsset::USDC));
        assert_eq!(asset.symbol(), "USDC");
    }

    #[test]
    fn test_asset_from_symbol_normalizes_eth_variants() {
        // WETH and ETH both map to KnownAsset::ETH
        let weth = Asset::from_symbol("WETH", "test");
        let eth = Asset::from_symbol("ETH", "test");
        assert_eq!(weth, eth);
        assert_eq!(weth, Asset::Known(KnownAsset::ETH));
    }

    #[test]
    fn test_asset_from_symbol_normalizes_sol_variants() {
        let wsol = Asset::from_symbol("WSOL", "test");
        let sol = Asset::from_symbol("SOL", "test");
        assert_eq!(wsol, sol);
        assert_eq!(wsol, Asset::Known(KnownAsset::SOL));
    }

    #[test]
    fn test_asset_from_symbol_unknown() {
        let asset = Asset::from_symbol("PEPE", "test");
        assert_eq!(asset, Asset::Unknown("PEPE".to_string()));
        assert_eq!(asset.symbol(), "PEPE");
    }

    #[test]
    fn test_asset_category_usd_correlated() {
        let usdc = Asset::from_symbol("USDC", "test");
        let cats = usdc.category();
        assert!(cats.contains(&AssetCategory::UsdCorrelated));
        assert!(cats.contains(&AssetCategory::Stablecoin));
    }

    #[test]
    fn test_asset_category_stablecoin_only() {
        // DAI should be Stablecoin but not UsdCorrelated
        let dai = Asset::from_symbol("DAI", "test");
        let cats = dai.category();
        assert!(cats.contains(&AssetCategory::Stablecoin));
        assert!(!cats.contains(&AssetCategory::UsdCorrelated));
    }

    #[test]
    fn test_asset_category_eth_correlated() {
        for sym in &["ETH", "WETH", "STETH", "WSTETH", "RETH", "CBETH"] {
            let asset = Asset::from_symbol(sym, "test");
            let cats = asset.category();
            assert!(
                cats.contains(&AssetCategory::EthCorrelated),
                "{} should be EthCorrelated",
                sym
            );
        }
    }

    #[test]
    fn test_asset_category_btc_correlated() {
        for sym in &["BTC", "WBTC", "CBBTC", "TBTC"] {
            let asset = Asset::from_symbol(sym, "test");
            let cats = asset.category();
            assert!(
                cats.contains(&AssetCategory::BtcCorrelated),
                "{} should be BtcCorrelated",
                sym
            );
        }
    }

    #[test]
    fn test_asset_category_sol_correlated() {
        for sym in &["SOL", "MSOL", "JITOSOL", "JUPSOL"] {
            let asset = Asset::from_symbol(sym, "test");
            let cats = asset.category();
            assert!(
                cats.contains(&AssetCategory::SolCorrelated),
                "{} should be SolCorrelated",
                sym
            );
        }
    }

    #[test]
    fn test_asset_infer_category_unknown_stablecoin() {
        // Unknown token with "USD" pattern should infer Stablecoin
        let asset = Asset::Unknown("TUSD".to_string());
        let cats = asset.category();
        assert!(cats.contains(&AssetCategory::Stablecoin));
    }

    #[test]
    fn test_asset_infer_category_unknown_eth() {
        let asset = Asset::Unknown("WEETH".to_string());
        let cats = asset.category();
        assert!(cats.contains(&AssetCategory::EthCorrelated));
    }

    #[test]
    fn test_asset_other_tokens_no_category() {
        let link = Asset::from_symbol("LINK", "test");
        assert!(link.category().is_empty());
    }

    // ========================================================================
    // canonical_name and normalize_pair
    // ========================================================================

    #[test]
    fn test_canonical_name_btc_family() {
        for sym in &["WBTC", "CBBTC", "TBTC"] {
            let asset = Asset::from_symbol(sym, "test");
            assert_eq!(
                asset.canonical_name(),
                "BTC",
                "{} canonical should be BTC",
                sym
            );
        }
    }

    #[test]
    fn test_canonical_name_eth_family() {
        let eth = Asset::from_symbol("ETH", "test");
        assert_eq!(eth.canonical_name(), "ETH");
        let wsteth = Asset::from_symbol("WSTETH", "test");
        assert_eq!(wsteth.canonical_name(), "LST-ETH");
    }

    #[test]
    fn test_canonical_name_usd_vs_stable() {
        let usdc = Asset::from_symbol("USDC", "test");
        assert_eq!(usdc.canonical_name(), "USD");
        let dai = Asset::from_symbol("DAI", "test");
        assert_eq!(dai.canonical_name(), "STABLE");
    }

    #[test]
    fn test_normalize_pair_alphabetical_order() {
        let eth = Asset::from_symbol("ETH", "test");
        let usdc = Asset::from_symbol("USDC", "test");
        // ETH vs USD → alphabetical: "ETH/USD"
        assert_eq!(normalize_pair(&eth, &usdc), "ETH/USD");
        // Reversed input should produce same output
        assert_eq!(normalize_pair(&usdc, &eth), "ETH/USD");
    }

    #[test]
    fn test_normalize_pair_cross_chain_equivalence() {
        let wbtc = Asset::from_symbol("WBTC", "test");
        let cbbtc = Asset::from_symbol("CBBTC", "test");
        let usdc = Asset::from_symbol("USDC", "test");
        let usdt = Asset::from_symbol("USDT", "test");
        // WBTC/USDC and CBBTC/USDT should produce the same normalized pair
        assert_eq!(normalize_pair(&wbtc, &usdc), normalize_pair(&cbbtc, &usdt));
        assert_eq!(normalize_pair(&wbtc, &usdc), "BTC/USD");
    }

    // ========================================================================
    // FeeTier
    // ========================================================================

    #[test]
    fn test_fee_tier_from_bps() {
        assert_eq!(FeeTier::from_bps(1), FeeTier::Bps1);
        assert_eq!(FeeTier::from_bps(5), FeeTier::Bps5);
        assert_eq!(FeeTier::from_bps(30), FeeTier::Bps30);
        assert_eq!(FeeTier::from_bps(100), FeeTier::Bps100);
        assert_eq!(FeeTier::from_bps(50), FeeTier::Custom(50));
    }

    #[test]
    fn test_fee_tier_from_uniswap_fee() {
        assert_eq!(FeeTier::from_uniswap_fee(100), FeeTier::Bps1);
        assert_eq!(FeeTier::from_uniswap_fee(500), FeeTier::Bps5);
        assert_eq!(FeeTier::from_uniswap_fee(3000), FeeTier::Bps30);
        assert_eq!(FeeTier::from_uniswap_fee(10000), FeeTier::Bps100);
    }

    #[test]
    fn test_fee_tier_to_bps_roundtrip() {
        for bps in [1, 5, 30, 100] {
            assert_eq!(FeeTier::from_bps(bps).to_bps(), bps);
        }
    }

    #[test]
    fn test_fee_tier_display() {
        assert_eq!(FeeTier::Bps1.display(), "0.01%");
        assert_eq!(FeeTier::Bps5.display(), "0.05%");
        assert_eq!(FeeTier::Bps30.display(), "0.30%");
        assert_eq!(FeeTier::Bps100.display(), "1.00%");
    }

    // ========================================================================
    // RateQuery parsing
    // ========================================================================

    #[test]
    fn test_rate_query_parse_chains() {
        let query = RateQuery {
            action: None,
            assets: None,
            chains: Some("ethereum,base,arbitrum".to_string()),
            protocols: None,
            operation_types: None,
            asset_categories: None,
            token: None,
            min_liquidity: 0,
            page: 1,
            page_size: 20,
        };
        let chains = query.parse_chains().unwrap();
        assert_eq!(chains.len(), 3);
        assert!(chains.contains(&Chain::Ethereum));
        assert!(chains.contains(&Chain::Base));
        assert!(chains.contains(&Chain::Arbitrum));
    }

    #[test]
    fn test_rate_query_parse_protocols() {
        let query = RateQuery {
            action: None,
            assets: None,
            chains: None,
            protocols: Some("aave,morpho,compound".to_string()),
            operation_types: None,
            asset_categories: None,
            token: None,
            min_liquidity: 0,
            page: 1,
            page_size: 20,
        };
        let protos = query.parse_protocols().unwrap();
        assert_eq!(protos.len(), 3);
        assert!(protos.contains(&Protocol::Aave));
        assert!(protos.contains(&Protocol::Morpho));
        assert!(protos.contains(&Protocol::Compound));
    }

    #[test]
    fn test_rate_query_parse_assets() {
        let query = RateQuery {
            action: None,
            assets: Some("usdc, eth, wbtc".to_string()),
            chains: None,
            protocols: None,
            operation_types: None,
            asset_categories: None,
            token: None,
            min_liquidity: 0,
            page: 1,
            page_size: 20,
        };
        let assets = query.parse_assets().unwrap();
        assert_eq!(assets, vec!["USDC", "ETH", "WBTC"]);
    }

    #[test]
    fn test_rate_query_parse_operation_types() {
        let query = RateQuery {
            action: None,
            assets: None,
            chains: None,
            protocols: None,
            operation_types: Some("lending,vault".to_string()),
            asset_categories: None,
            token: None,
            min_liquidity: 0,
            page: 1,
            page_size: 20,
        };
        let ops = query.parse_operation_types().unwrap();
        assert_eq!(ops.len(), 2);
        assert!(ops.contains(&OperationType::Lending));
        assert!(ops.contains(&OperationType::Vault));
    }

    #[test]
    fn test_rate_query_parse_none_returns_none() {
        let query = RateQuery {
            action: None,
            assets: None,
            chains: None,
            protocols: None,
            operation_types: None,
            asset_categories: None,
            token: None,
            min_liquidity: 0,
            page: 1,
            page_size: 20,
        };
        assert!(query.parse_chains().is_none());
        assert!(query.parse_protocols().is_none());
        assert!(query.parse_assets().is_none());
    }

    // ========================================================================
    // Vault ID stability
    // ========================================================================

    #[test]
    fn test_vault_id_deterministic() {
        let id1 = RateSnapshot::generate_vault_id(
            &Protocol::Aave,
            &Chain::Ethereum,
            "USDC",
            "https://app.aave.com",
            OperationType::Lending,
            Some(&Action::Supply),
        );
        let id2 = RateSnapshot::generate_vault_id(
            &Protocol::Aave,
            &Chain::Ethereum,
            "USDC",
            "https://app.aave.com",
            OperationType::Lending,
            Some(&Action::Supply),
        );
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_vault_id_different_for_supply_vs_borrow() {
        let supply_id = RateSnapshot::generate_vault_id(
            &Protocol::Aave,
            &Chain::Ethereum,
            "USDC",
            "https://app.aave.com",
            OperationType::Lending,
            Some(&Action::Supply),
        );
        let borrow_id = RateSnapshot::generate_vault_id(
            &Protocol::Aave,
            &Chain::Ethereum,
            "USDC",
            "https://app.aave.com",
            OperationType::Lending,
            Some(&Action::Borrow),
        );
        assert_ne!(supply_id, borrow_id);
    }

    #[test]
    fn test_vault_id_different_for_different_chains() {
        let eth_id = RateSnapshot::generate_vault_id(
            &Protocol::Aave,
            &Chain::Ethereum,
            "USDC",
            "https://app.aave.com",
            OperationType::Lending,
            Some(&Action::Supply),
        );
        let arb_id = RateSnapshot::generate_vault_id(
            &Protocol::Aave,
            &Chain::Arbitrum,
            "USDC",
            "https://app.aave.com",
            OperationType::Lending,
            Some(&Action::Supply),
        );
        assert_ne!(eth_id, arb_id);
    }

    #[test]
    fn test_pool_vault_id_deterministic() {
        let id1 = PoolSnapshot::generate_pool_vault_id(
            &Protocol::Uniswap,
            &Chain::Ethereum,
            "ETH",
            "USDC",
            30,
            "0xabc123",
        );
        let id2 = PoolSnapshot::generate_pool_vault_id(
            &Protocol::Uniswap,
            &Chain::Ethereum,
            "ETH",
            "USDC",
            30,
            "0xabc123",
        );
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_pool_vault_id_different_for_different_fees() {
        let id_30 = PoolSnapshot::generate_pool_vault_id(
            &Protocol::Uniswap,
            &Chain::Ethereum,
            "ETH",
            "USDC",
            30,
            "0xabc",
        );
        let id_5 = PoolSnapshot::generate_pool_vault_id(
            &Protocol::Uniswap,
            &Chain::Ethereum,
            "ETH",
            "USDC",
            5,
            "0xabc",
        );
        assert_ne!(id_30, id_5);
    }

    // ========================================================================
    // PoolRate.to_result
    // ========================================================================

    #[test]
    fn test_pool_rate_to_result_turnover_ratio() {
        let rate = PoolRate {
            protocol: Protocol::Uniswap,
            chain: Chain::Ethereum,
            token0: Asset::from_symbol("ETH", "test"),
            token1: Asset::from_symbol("USDC", "test"),
            pool_type: PoolType::ConcentratedLiquidity,
            fee_tier: FeeTier::Bps30,
            fee_rate_bps: 30,
            tvl_usd: 1_000_000.0,
            volume_24h_usd: 500_000.0,
            volume_7d_usd: 3_500_000.0,
            fees_24h_usd: 150.0,
            fees_7d_usd: 1050.0,
            fee_apr_24h: 5.475,
            fee_apr_7d: 5.475,
            rewards_apr: 0.0,
            pool_address: "0xabc".to_string(),
            pool_id: None,
            active: true,
            timestamp: Utc::now(),
        };

        let result = rate.to_result("https://example.com".to_string());
        assert_eq!(result.pair, "ETH/USDC");
        assert_eq!(result.normalized_pair, "ETH/USD");
        // turnover = 500k / 1M = 0.5
        assert!((result.turnover_ratio_24h - 0.5).abs() < 0.001);
        // 7d turnover = (3.5M / 7) / 1M = 0.5
        assert!((result.turnover_ratio_7d - 0.5).abs() < 0.001);
        assert_eq!(result.total_apr, 5.475); // fee_apr + rewards
    }

    #[test]
    fn test_pool_rate_to_result_zero_tvl() {
        let rate = PoolRate {
            protocol: Protocol::Uniswap,
            chain: Chain::Ethereum,
            token0: Asset::from_symbol("ETH", "test"),
            token1: Asset::from_symbol("USDC", "test"),
            pool_type: PoolType::Standard,
            fee_tier: FeeTier::Bps30,
            fee_rate_bps: 30,
            tvl_usd: 0.0,
            volume_24h_usd: 100.0,
            volume_7d_usd: 700.0,
            fees_24h_usd: 0.0,
            fees_7d_usd: 0.0,
            fee_apr_24h: 0.0,
            fee_apr_7d: 0.0,
            rewards_apr: 0.0,
            pool_address: "0x0".to_string(),
            pool_id: None,
            active: true,
            timestamp: Utc::now(),
        };

        let result = rate.to_result("https://example.com".to_string());
        assert_eq!(result.turnover_ratio_24h, 0.0);
        assert_eq!(result.turnover_ratio_7d, 0.0);
    }

    // ========================================================================
    // Action / OperationType serialization
    // ========================================================================

    #[test]
    fn test_action_serialization() {
        assert_eq!(
            serde_json::to_string(&Action::Supply).unwrap(),
            "\"supply\""
        );
        assert_eq!(
            serde_json::to_string(&Action::Borrow).unwrap(),
            "\"borrow\""
        );
        let a: Action = serde_json::from_str("\"supply\"").unwrap();
        assert_eq!(a, Action::Supply);
    }

    #[test]
    fn test_operation_type_serialization() {
        assert_eq!(
            serde_json::to_string(&OperationType::Lending).unwrap(),
            "\"lending\""
        );
        assert_eq!(
            serde_json::to_string(&OperationType::LiquidityPool).unwrap(),
            "\"liquiditypool\""
        );
        let op: OperationType = serde_json::from_str("\"vault\"").unwrap();
        assert_eq!(op, OperationType::Vault);
    }

    // ========================================================================
    // round_f64_5 serializer
    // ========================================================================

    #[test]
    fn test_round_f64_5_serializer() {
        #[derive(Serialize)]
        struct T {
            #[serde(serialize_with = "round_f64_5::serialize")]
            v: f64,
        }
        let t = T { v: 1.23456789 };
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, r#"{"v":1.23457}"#);
    }

    // ========================================================================
    // PoolQuery parsing
    // ========================================================================

    #[test]
    fn test_pool_query_parse_pool_type() {
        let query = PoolQuery {
            asset_categories_0: None,
            asset_categories_1: None,
            token_a: None,
            token_b: None,
            token: None,
            pair: None,
            chains: None,
            protocols: None,
            pool_type: Some("concentrated".to_string()),
            min_tvl: 0,
            min_volume: 0,
            normalized_pair: None,
            page: 1,
            page_size: 20,
        };
        assert_eq!(
            query.parse_pool_type(),
            Some(PoolType::ConcentratedLiquidity)
        );
    }
}
