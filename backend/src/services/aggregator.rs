use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

use crate::{
    config::Config,
    indexers::{
        AaveIndexer, AerodromeIndexer, AuraIndexer, BalancerIndexer, CamelotIndexer, CurveIndexer,
        EthenaIndexer, EulerIndexer, FluidIndexer, FraxEthIndexer, GmxIndexer, JitoIndexer,
        JupiterIndexer, JustLendIndexer, KaminoIndexer, LidoIndexer, MarinadeIndexer,
        MaverickIndexer, MeteoraIndexer, MorphoIndexer, OrcaIndexer, PancakeSwapIndexer,
        PendleIndexer, PoolIndexer, RateIndexer, RaydiumIndexer, RocketPoolIndexer, SiloIndexer,
        SkyIndexer, SparkLendIndexer, SushiSwapIndexer, TraderJoeIndexer, UniswapV3Indexer,
        UniswapV4Indexer, VelodromeIndexer, VenusIndexer, YearnIndexer,
    },
    models::*,
    services::circuit_breaker::CircuitBreaker,
};

// ============================================================================
// Public types used by collection_worker
// ============================================================================

/// Metadata captured per indexer task (timing + errors)
#[derive(Debug, Clone)]
pub struct IndexerTaskMeta {
    pub protocol: Protocol,
    pub chain: Chain,
    pub items_found: usize,
    pub duration_ms: i64,
    pub error: Option<String>,
}

/// Result of get_rates: rates + per-task metadata
pub struct RatesCollectionOutput {
    pub rates: Vec<RateResult>,
    pub task_meta: Vec<IndexerTaskMeta>,
}

/// Result of get_pools: pools + per-task metadata
pub struct PoolsCollectionOutput {
    pub pools: Vec<PoolResult>,
    pub task_meta: Vec<IndexerTaskMeta>,
}

// ============================================================================
// Registry-based aggregator
// ============================================================================

pub struct RateAggregator {
    rate_indexers: Vec<Arc<dyn RateIndexer>>,
    pool_indexers: Vec<Arc<dyn PoolIndexer>>,
    semaphore: Arc<Semaphore>,
    indexer_timeout: Duration,
    pub circuit_breaker: CircuitBreaker,
}

impl RateAggregator {
    pub fn new(config: Config) -> Self {
        // ---- Rate indexers ----
        // NOTE: DeFiLlama-dependent indexers are temporarily disabled.
        // They will be re-enabled once each has a direct data source.
        // Disabled rate indexers: Compound, EtherFi, Benqi, Radiant, Convex, Stargate
        let rate_indexers: Vec<Arc<dyn RateIndexer>> = vec![
            Arc::new(AaveIndexer::new(
                config.aave_subgraph_arbitrum.clone(),
                config.aave_subgraph_base.clone(),
            )),
            Arc::new(KaminoIndexer::new(config.kamino_api_url.clone())),
            Arc::new(MorphoIndexer::new(config.morpho_api_url.clone())),
            Arc::new(FluidIndexer::new(config.fluid_api_url.clone())),
            Arc::new(SparkLendIndexer::new()),
            Arc::new(JustLendIndexer::new(config.trongrid_api_key.clone())),
            Arc::new(EulerIndexer::new()),
            Arc::new(JupiterIndexer::new()),
            Arc::new(LidoIndexer::new()),
            Arc::new(MarinadeIndexer::new()),
            Arc::new(JitoIndexer::new()),
            Arc::new(RocketPoolIndexer::new()),
            // Arc::new(CompoundIndexer::new().with_cache(defillama_cache.clone())),
            Arc::new(VenusIndexer::new()),
            Arc::new(PendleIndexer::new()),
            Arc::new(EthenaIndexer::new()),
            // Arc::new(EtherFiIndexer::new().with_cache(defillama_cache.clone())),
            // Arc::new(BenqiIndexer::new().with_cache(defillama_cache.clone())),
            // Arc::new(RadiantIndexer::new().with_cache(defillama_cache.clone())),
            Arc::new(SkyIndexer::new()),
            Arc::new(SiloIndexer::new()),
            Arc::new(FraxEthIndexer::new()),
            Arc::new(AuraIndexer::new()),
            // Arc::new(ConvexIndexer::new().with_cache(defillama_cache.clone())),
            Arc::new(YearnIndexer::new()),
            // Arc::new(StargateIndexer::new().with_cache(defillama_cache.clone())),
            Arc::new(GmxIndexer::new()),
        ];

        // ---- Pool indexers ----
        // All pool indexers migrated to direct sources (no more DeFiLlama)
        let pool_indexers: Vec<Arc<dyn PoolIndexer>> = vec![
            Arc::new(RaydiumIndexer::new(config.raydium_api_url.clone())),
            Arc::new(UniswapV3Indexer::new(config.the_graph_api_key.clone())),
            Arc::new(UniswapV4Indexer::new()),
            Arc::new(CurveIndexer::new()),
            Arc::new(PancakeSwapIndexer::new(config.the_graph_api_key.clone())),
            Arc::new(AerodromeIndexer::new(config.the_graph_api_key.clone())),
            Arc::new(VelodromeIndexer::new(config.the_graph_api_key.clone())),
            Arc::new(OrcaIndexer::new()),
            Arc::new(MeteoraIndexer::new()),
            Arc::new(SushiSwapIndexer::new(config.the_graph_api_key.clone())),
            Arc::new(CamelotIndexer::new(config.the_graph_api_key.clone())),
            Arc::new(TraderJoeIndexer::new(config.the_graph_api_key.clone())),
            Arc::new(BalancerIndexer::new()),
            Arc::new(MaverickIndexer::new()),
        ];

        Self {
            rate_indexers,
            pool_indexers,
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_indexers)),
            indexer_timeout: Duration::from_secs(config.indexer_timeout_secs),
            circuit_breaker: CircuitBreaker::new(
                config.cb_failure_threshold,
                config.cb_cooldown_secs,
            ),
        }
    }

    // ========================================================================
    // Rate collection (lending/vault/staking)
    // ========================================================================

    pub async fn get_rates(&self, query: &RateQuery) -> Result<Vec<RateResult>> {
        let output = self.get_rates_with_meta(query).await?;
        Ok(output.rates)
    }

    pub async fn get_rates_with_meta(&self, query: &RateQuery) -> Result<RatesCollectionOutput> {
        let target_chains = query.parse_chains().unwrap_or_else(|| Chain::all());
        let target_protocols = query.parse_protocols().unwrap_or_else(|| Protocol::all());

        tracing::debug!(
            "Target chains: {:?}, Target protocols: {:?}",
            target_chains,
            target_protocols
        );

        // Spawn one task per (indexer, chain) pair
        type TaskResult = (Protocol, Chain, Result<Vec<ProtocolRate>>, u128);
        let mut tasks: Vec<tokio::task::JoinHandle<TaskResult>> = Vec::new();

        for indexer in &self.rate_indexers {
            let protocol = indexer.protocol();
            if !target_protocols.contains(&protocol) {
                continue;
            }

            for chain in indexer.supported_chains() {
                if !target_chains.contains(&chain) {
                    continue;
                }

                // Circuit breaker check
                if self.circuit_breaker.should_skip(&protocol, &chain).await {
                    tracing::debug!("⚡ Skipping {:?}/{:?} (circuit open)", protocol, chain);
                    continue;
                }

                let idx = Arc::clone(indexer);
                let sem = Arc::clone(&self.semaphore);
                let c = chain.clone();
                let timeout = self.indexer_timeout;

                tasks.push(tokio::spawn(async move {
                    let _permit = sem.acquire().await.expect("semaphore closed");
                    let start = Instant::now();
                    let result = match tokio::time::timeout(timeout, idx.fetch_rates(&c)).await {
                        Ok(r) => r,
                        Err(_) => Err(anyhow::anyhow!(
                            "indexer timeout after {}s",
                            timeout.as_secs()
                        )),
                    };
                    let elapsed = start.elapsed().as_millis();
                    (idx.protocol(), c, result, elapsed)
                }));
            }
        }

        // Collect results + metadata
        let mut all_rates = Vec::new();
        let mut task_meta = Vec::new();
        tracing::debug!("Waiting for {} rate indexer tasks", tasks.len());

        for task in tasks {
            match task.await {
                Ok((protocol, chain, Ok(rates), elapsed_ms)) => {
                    tracing::debug!(
                        "{:?}/{:?} returned {} rates in {}ms",
                        protocol,
                        chain,
                        rates.len(),
                        elapsed_ms
                    );
                    self.circuit_breaker.record_success(&protocol, &chain).await;
                    task_meta.push(IndexerTaskMeta {
                        protocol,
                        chain,
                        items_found: rates.len(),
                        duration_ms: elapsed_ms as i64,
                        error: None,
                    });
                    all_rates.extend(rates);
                }
                Ok((protocol, chain, Err(e), elapsed_ms)) => {
                    tracing::error!(
                        "{:?}/{:?} failed in {}ms: {:?}",
                        protocol,
                        chain,
                        elapsed_ms,
                        e
                    );
                    self.circuit_breaker.record_failure(&protocol, &chain).await;
                    task_meta.push(IndexerTaskMeta {
                        protocol,
                        chain,
                        items_found: 0,
                        duration_ms: elapsed_ms as i64,
                        error: Some(format!("{:?}", e)),
                    });
                }
                Err(e) => tracing::error!("Task join error: {:?}", e),
            }
        }

        tracing::debug!("Total rates collected: {}", all_rates.len());

        // Filter by query params
        let target_operation_types = query.parse_operation_types();
        let target_asset_categories = query.parse_asset_categories();

        let filtered_rates: Vec<_> = all_rates
            .into_iter()
            .filter(|r| {
                if let Some(ref target_assets) = query.parse_assets() {
                    if !target_assets.contains(&r.asset.symbol().to_uppercase()) {
                        return false;
                    }
                }
                if let Some(ref action) = query.action {
                    if &r.action != action {
                        return false;
                    }
                }
                if let Some(ref operation_types) = target_operation_types {
                    if !operation_types.contains(&r.operation_type) {
                        return false;
                    }
                }
                if let Some(ref asset_categories) = target_asset_categories {
                    let asset_cats = r.asset.category();
                    if !asset_cats.iter().any(|cat| asset_categories.contains(cat)) {
                        return false;
                    }
                }
                if !target_chains.contains(&r.chain) {
                    return false;
                }
                if !r.active {
                    return false;
                }
                if r.available_liquidity == 0 || r.available_liquidity < query.min_liquidity {
                    return false;
                }
                true
            })
            .collect();

        tracing::debug!("Filtered rates: {}", filtered_rates.len());

        // Build URL via trait and convert to RateResult
        let rate_indexer_map: std::collections::HashMap<Protocol, &Arc<dyn RateIndexer>> = self
            .rate_indexers
            .iter()
            .map(|i| (i.protocol(), i))
            .collect();

        let results: Vec<RateResult> = filtered_rates
            .into_iter()
            .map(|rate| {
                let apy = match rate.action {
                    Action::Supply => rate.supply_apy,
                    Action::Borrow => rate.borrow_apr,
                };

                let url = rate_indexer_map
                    .get(&rate.protocol)
                    .map(|idx| idx.rate_url(&rate))
                    .unwrap_or_default();

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
                    apy_metrics: None,
                }
            })
            .collect();

        Ok(RatesCollectionOutput {
            rates: results,
            task_meta,
        })
    }

    // ========================================================================
    // Pool collection (DEX/LP)
    // ========================================================================

    pub async fn get_pools(&self, query: &PoolQuery) -> Result<Vec<PoolResult>> {
        let output = self.get_pools_with_meta(query).await?;
        Ok(output.pools)
    }

    pub async fn get_pools_with_meta(&self, query: &PoolQuery) -> Result<PoolsCollectionOutput> {
        let target_chains = query.parse_chains().unwrap_or_else(|| Chain::all());
        let target_protocols = query.parse_protocols().unwrap_or_else(|| Protocol::all());

        type TaskResult = (Protocol, Chain, Result<Vec<PoolRate>>, u128);
        let mut tasks: Vec<tokio::task::JoinHandle<TaskResult>> = Vec::new();

        for indexer in &self.pool_indexers {
            let protocol = indexer.protocol();
            if !target_protocols.contains(&protocol) {
                continue;
            }

            for chain in indexer.supported_chains() {
                if !target_chains.contains(&chain) {
                    continue;
                }

                if self.circuit_breaker.should_skip(&protocol, &chain).await {
                    tracing::debug!("⚡ Skipping pool {:?}/{:?} (circuit open)", protocol, chain);
                    continue;
                }

                let idx = Arc::clone(indexer);
                let sem = Arc::clone(&self.semaphore);
                let c = chain.clone();
                let timeout = self.indexer_timeout;

                tasks.push(tokio::spawn(async move {
                    let _permit = sem.acquire().await.expect("semaphore closed");
                    let start = Instant::now();
                    let result = match tokio::time::timeout(timeout, idx.fetch_pools(&c)).await {
                        Ok(r) => r,
                        Err(_) => Err(anyhow::anyhow!(
                            "pool indexer timeout after {}s",
                            timeout.as_secs()
                        )),
                    };
                    let elapsed = start.elapsed().as_millis();
                    (idx.protocol(), c, result, elapsed)
                }));
            }
        }

        // Collect results
        tracing::debug!("Waiting for {} pool indexer tasks", tasks.len());
        let mut all_pool_rates = Vec::new();
        let mut pool_task_meta = Vec::new();

        for task in tasks {
            match task.await {
                Ok((protocol, chain, Ok(rates), elapsed_ms)) => {
                    tracing::debug!(
                        "{:?}/{:?} returned {} pools in {}ms",
                        protocol,
                        chain,
                        rates.len(),
                        elapsed_ms
                    );
                    self.circuit_breaker.record_success(&protocol, &chain).await;
                    pool_task_meta.push(IndexerTaskMeta {
                        protocol,
                        chain,
                        items_found: rates.len(),
                        duration_ms: elapsed_ms as i64,
                        error: None,
                    });
                    all_pool_rates.extend(rates);
                }
                Ok((protocol, chain, Err(e), elapsed_ms)) => {
                    tracing::error!(
                        "{:?}/{:?} pool fetch failed in {}ms: {:?}",
                        protocol,
                        chain,
                        elapsed_ms,
                        e
                    );
                    self.circuit_breaker.record_failure(&protocol, &chain).await;
                    pool_task_meta.push(IndexerTaskMeta {
                        protocol,
                        chain,
                        items_found: 0,
                        duration_ms: elapsed_ms as i64,
                        error: Some(format!("{:?}", e)),
                    });
                }
                Err(e) => tracing::error!("Pool task join error: {:?}", e),
            }
        }

        tracing::debug!("Total pool rates collected: {}", all_pool_rates.len());

        // Build URL via trait and convert to PoolResult
        let pool_indexer_map: std::collections::HashMap<Protocol, &Arc<dyn PoolIndexer>> = self
            .pool_indexers
            .iter()
            .map(|i| (i.protocol(), i))
            .collect();

        // Filter and convert
        let cats0 = query.parse_asset_categories_0();
        let cats1 = query.parse_asset_categories_1();
        let target_pool_type = query.parse_pool_type();
        let token_filter = query.token.as_ref().map(|t| t.to_uppercase());
        let pair_filter = query.pair.as_ref().map(|p| p.to_uppercase());

        let mut results: Vec<PoolResult> = all_pool_rates
            .into_iter()
            .map(|rate| {
                let url = pool_indexer_map
                    .get(&rate.protocol)
                    .map(|idx| idx.pool_url(&rate))
                    .unwrap_or_default();
                rate.to_result(url)
            })
            .filter(|r| {
                if (r.tvl_usd as u64) < query.min_tvl {
                    return false;
                }
                // Two-sided category filter (same logic as build_pool_filter)
                match (&cats0, &cats1) {
                    (Some(c0), Some(c1)) => {
                        let t0_in_c0 = r
                            .token0_categories
                            .iter()
                            .any(|c: &AssetCategory| c0.contains(c));
                        let t1_in_c1 = r
                            .token1_categories
                            .iter()
                            .any(|c: &AssetCategory| c1.contains(c));
                        let t0_in_c1 = r
                            .token0_categories
                            .iter()
                            .any(|c: &AssetCategory| c1.contains(c));
                        let t1_in_c0 = r
                            .token1_categories
                            .iter()
                            .any(|c: &AssetCategory| c0.contains(c));
                        if !((t0_in_c0 && t1_in_c1) || (t0_in_c1 && t1_in_c0)) {
                            return false;
                        }
                    }
                    (Some(cats), None) | (None, Some(cats)) => {
                        let has_match = r
                            .token0_categories
                            .iter()
                            .any(|c: &AssetCategory| cats.contains(c))
                            || r.token1_categories
                                .iter()
                                .any(|c: &AssetCategory| cats.contains(c));
                        if !has_match {
                            return false;
                        }
                    }
                    (None, None) => {}
                }
                if let Some(ref pt) = target_pool_type {
                    if &r.pool_type != pt {
                        return false;
                    }
                }
                if let Some(ref token) = token_filter {
                    if r.token0.to_uppercase() != *token && r.token1.to_uppercase() != *token {
                        return false;
                    }
                }
                if let Some(ref pair) = pair_filter {
                    let pair_upper = r.pair.to_uppercase();
                    let pair_reversed =
                        format!("{}/{}", r.token1.to_uppercase(), r.token0.to_uppercase());
                    if pair_upper != *pair && pair_reversed != *pair {
                        return false;
                    }
                }
                true
            })
            .collect();

        results.sort_by(|a, b| {
            b.fee_apr_24h
                .partial_cmp(&a.fee_apr_24h)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(PoolsCollectionOutput {
            pools: results,
            task_meta: pool_task_meta,
        })
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
