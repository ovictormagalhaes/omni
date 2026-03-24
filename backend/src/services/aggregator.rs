use anyhow::Result;

use crate::{
    config::Config,
    indexers::{AaveIndexer, KaminoIndexer, MorphoIndexer, FluidIndexer, SparkLendIndexer, JustLendIndexer, EulerIndexer, JupiterIndexer, LidoIndexer, MarinadeIndexer, JitoIndexer, RocketPoolIndexer},
    models::*,
};

pub struct RateAggregator {
    aave_indexer: AaveIndexer,
    kamino_indexer: KaminoIndexer,
    morpho_indexer: MorphoIndexer,
    fluid_indexer: FluidIndexer,
    sparklend_indexer: SparkLendIndexer,
    justlend_indexer: JustLendIndexer,
    euler_indexer: EulerIndexer,
    jupiter_indexer: JupiterIndexer,
    lido_indexer: LidoIndexer,
    marinade_indexer: MarinadeIndexer,
    jito_indexer: JitoIndexer,
    rocketpool_indexer: RocketPoolIndexer,
}

impl RateAggregator {
    pub fn new(config: Config) -> Self {
        let aave_indexer = AaveIndexer::new(
            config.aave_subgraph_arbitrum.clone(),
            config.aave_subgraph_base.clone(),
        );

        let kamino_indexer = KaminoIndexer::new(config.kamino_api_url.clone());
        let morpho_indexer = MorphoIndexer::new(config.morpho_api_url.clone());
        let fluid_indexer = FluidIndexer::new(config.fluid_api_url.clone());
        let sparklend_indexer = SparkLendIndexer::new();
        let justlend_indexer = JustLendIndexer::new(config.trongrid_api_key.clone());
        let euler_indexer = EulerIndexer::new();
        let jupiter_indexer = JupiterIndexer::new();
        let lido_indexer = LidoIndexer::new();
        let marinade_indexer = MarinadeIndexer::new();
        let jito_indexer = JitoIndexer::new();
        let rocketpool_indexer = RocketPoolIndexer::new();

        Self {
            aave_indexer,
            kamino_indexer,
            morpho_indexer,
            fluid_indexer,
            sparklend_indexer,
            justlend_indexer,
            euler_indexer,
            jupiter_indexer,
            lido_indexer,
            marinade_indexer,
            jito_indexer,
            rocketpool_indexer,
        }
    }

    pub async fn get_rates(&self, query: &RateQuery) -> Result<Vec<RateResult>> {
        let target_chains = query.parse_chains().unwrap_or_else(|| Chain::all());
        let target_protocols = query.parse_protocols().unwrap_or_else(|| Protocol::all());
        
        tracing::debug!("Target chains: {:?}, Target protocols: {:?}", target_chains, target_protocols);

        // Fetch rates from all indexers in parallel
        let mut tasks = Vec::new();

        // Aave tasks
        if target_protocols.contains(&Protocol::Aave) {
            for chain in &target_chains {
                if *chain != Chain::Solana {
                    let indexer = self.aave_indexer.clone();
                    let chain_clone = chain.clone();
                    tasks.push(tokio::spawn(async move {
                        indexer.fetch_rates(&chain_clone).await
                    }));
                }
            }
        }

        // Kamino task
        if target_protocols.contains(&Protocol::Kamino) && target_chains.contains(&Chain::Solana) {
            let indexer = self.kamino_indexer.clone();
            tasks.push(tokio::spawn(async move {
                indexer.fetch_rates().await
            }));
        }

        // Morpho task
        if target_protocols.contains(&Protocol::Morpho) {
            let indexer = self.morpho_indexer.clone();
            tasks.push(tokio::spawn(async move {
                indexer.fetch_rates().await
            }));
        }

        // Fluid task
        if target_protocols.contains(&Protocol::Fluid) && target_chains.contains(&Chain::Ethereum) {
            let indexer = self.fluid_indexer.clone();
            tasks.push(tokio::spawn(async move {
                indexer.fetch_rates().await
            }));
        }

         // SparkLend tasks
        if target_protocols.contains(&Protocol::SparkLend) {
            for chain in &[Chain::Ethereum] {
                if target_chains.contains(chain) {
                    let indexer = self.sparklend_indexer.clone();
                    let chain_clone = chain.clone();
                    tasks.push(tokio::spawn(async move {
                        indexer.fetch_rates(&chain_clone).await
                    }));
                }
            }
        }

        // JustLend task (Tron only)
        if target_protocols.contains(&Protocol::JustLend) && target_chains.contains(&Chain::Tron) {
            let indexer = self.justlend_indexer.clone();
            tasks.push(tokio::spawn(async move {
                indexer.fetch_rates(&Chain::Tron).await
            }));
        }

        // Euler task (Ethereum only)
        if target_protocols.contains(&Protocol::Euler) && target_chains.contains(&Chain::Ethereum) {
            let indexer = self.euler_indexer.clone();
            tasks.push(tokio::spawn(async move {
                indexer.fetch_rates(&Chain::Ethereum).await
            }));
        }

        // Jupiter task (Solana only)
        if target_protocols.contains(&Protocol::Jupiter) && target_chains.contains(&Chain::Solana) {
            let indexer = self.jupiter_indexer.clone();
            tasks.push(tokio::spawn(async move {
                indexer.fetch_rates(&Chain::Solana).await
            }));
        }

        // Lido tasks (Ethereum + Solana)
        if target_protocols.contains(&Protocol::Lido) {
            for chain in &target_chains {
                if *chain == Chain::Ethereum || *chain == Chain::Solana {
                    let indexer = self.lido_indexer.clone();
                    let chain_clone = chain.clone();
                    tasks.push(tokio::spawn(async move {
                        indexer.fetch_rates(&chain_clone).await
                    }));
                }
            }
        }

        // Marinade task (Solana only)
        if target_protocols.contains(&Protocol::Marinade) && target_chains.contains(&Chain::Solana) {
            let indexer = self.marinade_indexer.clone();
            tasks.push(tokio::spawn(async move {
                indexer.fetch_rates(&Chain::Solana).await
            }));
        }

        // Jito task (Solana only)
        if target_protocols.contains(&Protocol::Jito) && target_chains.contains(&Chain::Solana) {
            let indexer = self.jito_indexer.clone();
            tasks.push(tokio::spawn(async move {
                indexer.fetch_rates(&Chain::Solana).await
            }));
        }

        // Rocket Pool task (Ethereum only)
        if target_protocols.contains(&Protocol::RocketPool) && target_chains.contains(&Chain::Ethereum) {
            let indexer = self.rocketpool_indexer.clone();
            tasks.push(tokio::spawn(async move {
                indexer.fetch_rates(&Chain::Ethereum).await
            }));
        }

        // Wait for all tasks
        let mut all_rates = Vec::new();
        tracing::debug!("Waiting for {} indexer tasks", tasks.len());
        for task in tasks {
            match task.await {
                Ok(Ok(rates)) => {
                    tracing::debug!("Indexer returned {} rates", rates.len());
                    all_rates.extend(rates);
                },
                Ok(Err(e)) => tracing::error!("Indexer error: {:?}", e),
                Err(e) => tracing::error!("Task join error: {:?}", e),
            }
        }
        
        tracing::debug!("Total rates collected: {}", all_rates.len());

        // Filter by asset, action, chain, operation_type, asset_category, and minimum liquidity
        let target_operation_types = query.parse_operation_types();
        let target_asset_categories = query.parse_asset_categories();
        
        let filtered_rates: Vec<_> = all_rates
            .into_iter()
            .filter(|r| {
                // Filter by asset (comma-separated list of symbols)
                if let Some(ref target_assets) = query.parse_assets() {
                    if !target_assets.contains(&r.asset.symbol().to_uppercase()) {
                        return false;
                    }
                }
                
                // Filter by action
                if let Some(ref action) = query.action {
                    if &r.action != action {
                        return false;
                    }
                }
                
                // Filter by operation_type (lending, vault)
                if let Some(ref operation_types) = target_operation_types {
                    if !operation_types.contains(&r.operation_type) {
                        return false;
                    }
                }

                // Filter by asset_category (usd-based, btc-based, eth-based)
                if let Some(ref asset_categories) = target_asset_categories {
                    let asset_cats = r.asset.category();
                    if !asset_cats.iter().any(|cat| asset_categories.contains(cat)) {
                        return false;
                    }
                }
                
                // Filter by chain (if specified)
                if !target_chains.contains(&r.chain) {
                    return false;
                }
                
                // Filter inactive vaults
                if !r.active {
                    return false;
                }
                
                // Filter by minimum liquidity (default: 100k USD, ignore 0 liquidity)
                if r.available_liquidity == 0 || r.available_liquidity < query.min_liquidity {
                    return false;
                }
                
                true
            })
            .collect();
        
        tracing::debug!("Filtered rates: {}", filtered_rates.len());

        // Convert to RateResult
        let results: Vec<RateResult> = filtered_rates
            .into_iter()
            .map(|rate| {
                // Use the appropriate rate based on action
                let apy = match rate.action {
                    Action::Supply => rate.supply_apy,
                    Action::Borrow => rate.borrow_apr,
                };

                let url = match rate.protocol {
                    Protocol::Aave => self.aave_indexer.get_protocol_url(&rate.chain, rate.underlying_asset.as_deref()),
                    Protocol::Kamino => self.kamino_indexer.get_protocol_url(),
                    Protocol::Morpho => self.morpho_indexer.get_protocol_url(&rate.chain, rate.vault_id.as_deref()),
                    Protocol::Fluid => self.fluid_indexer.get_protocol_url(),
                    Protocol::SparkLend => self.sparklend_indexer.get_protocol_url(&rate.chain, rate.underlying_asset.as_deref()),
                    Protocol::JustLend => self.justlend_indexer.get_protocol_url(),
                    Protocol::Euler => self.euler_indexer.get_protocol_url(rate.vault_id.as_deref()),
                    Protocol::Jupiter => self.jupiter_indexer.get_protocol_url(),
                    Protocol::Lido => self.lido_indexer.get_protocol_url(),
                    Protocol::Marinade => self.marinade_indexer.get_protocol_url(),
                    Protocol::Jito => self.jito_indexer.get_protocol_url(),
                    Protocol::RocketPool => self.rocketpool_indexer.get_protocol_url(),
                };

                RateResult {
                    protocol: rate.protocol,
                    chain: rate.chain,
                    asset: rate.asset.clone(),
                    action: rate.action,
                    asset_category: rate.asset.category(),
                    apy,
                    rewards: rate.rewards,
                    net_apy: apy + rate.rewards,
                    performance_fee: rate.performance_fee,
                    active: rate.active,
                    collateral_enabled: rate.collateral_enabled,
                    collateral_ltv: rate.collateral_ltv,
                    liquidity: rate.available_liquidity,
                    total_liquidity: rate.total_liquidity,
                    utilization_rate: rate.utilization_rate.round() as u32,
                    operation_type: rate.operation_type,
                    vault_id: rate.vault_id.clone(),
                    vault_name: rate.vault_name,
                    url,
                    last_update: rate.timestamp,
                    apy_metrics: None, // No historical APY data from real-time aggregator
                }
            })
            .collect();

        Ok(results)
    }
}

// Make indexers cloneable for async tasks
impl Clone for AaveIndexer {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
        }
    }
}

impl Clone for KaminoIndexer {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            api_url: self.api_url.clone(),
        }
    }
}

impl Clone for MorphoIndexer {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            api_url: self.api_url.clone(),
        }
    }
}

impl Clone for FluidIndexer {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            api_url: self.api_url.clone(),
        }
    }
}
