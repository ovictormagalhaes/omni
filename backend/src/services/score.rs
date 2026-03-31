use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;

use crate::models::*;
use crate::services::pool_realtime::PoolRealtimeService;
use crate::services::realtime::RealtimeService;

pub async fn score_pool(
    pool_realtime: &PoolRealtimeService,
    req: &PoolScoreRequest,
) -> Result<PoolScoreResponse> {
    let token0_asset = Asset::from_symbol(&req.token0, "score");
    let token1_asset = Asset::from_symbol(&req.token1, "score");
    let token0_canonical = token0_asset.canonical_name();
    let token1_canonical = token1_asset.canonical_name();
    let target_normalized = normalize_pair(&token0_asset, &token1_asset);

    let query = PoolQuery {
        asset_categories_0: None,
        asset_categories_1: None,
        token_a: None,
        token_b: None,
        token: None,
        pair: None,
        chains: None,
        protocols: None,
        pool_type: None,
        min_tvl: req.min_tvl,
        min_volume: 0,
        normalized_pair: Some(target_normalized.clone()),
        page: 1,
        page_size: 100, // ignored by query_all_pools
    };

    let all_pools = pool_realtime.query_all_pools(&query).await?;

    let mut comparable: Vec<PoolResult> = all_pools;

    tracing::info!(
        "Pool score: found {} comparable pools for normalized_pair={}",
        comparable.len(),
        target_normalized
    );

    // Sort by fee_apr_7d (more reliable than total_apr which can be inflated by rewards on dead pools)
    comparable.sort_by(|a, b| {
        b.fee_apr_7d
            .partial_cmp(&a.fee_apr_7d)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let user_pool_index = if req.protocol.is_some() || req.chain.is_some() {
        let t0_sym = token0_asset.symbol();
        let t1_sym = token1_asset.symbol();
        let t0_cats = token0_asset.category();
        let t1_cats = token1_asset.category();

        // Log what we're looking for
        tracing::info!(
            "Pool score: searching user pool - protocol={:?}, chain={:?}, tokens=({}, {})",
            req.protocol,
            req.chain,
            t0_sym,
            t1_sym
        );

        // Log available pools for this protocol+chain
        for p in comparable.iter().filter(|p| {
            req.protocol.as_ref().is_some_and(|rp| p.protocol == *rp)
                && req.chain.as_ref().is_some_and(|rc| p.chain == *rc)
        }) {
            tracing::info!(
                "  candidate: {}/{} fee={} apr={:.2}",
                p.token0,
                p.token1,
                p.fee_tier,
                p.total_apr
            );
        }

        comparable.iter().position(|p| {
            let protocol_match = req.protocol.as_ref().is_none_or(|rp| p.protocol == *rp);
            let chain_match = req.chain.as_ref().is_none_or(|rc| p.chain == *rc);
            let fee_match = req.fee_tier.is_none_or(|bps| p.fee_rate_bps == bps);

            // Match tokens: exact symbol first, then category fallback (either order)
            let pt0 = p.token0.to_uppercase();
            let pt1 = p.token1.to_uppercase();
            let t0_up = t0_sym.to_uppercase();
            let t1_up = t1_sym.to_uppercase();

            let tokens_exact = (pt0 == t0_up || pt1 == t0_up) && (pt0 == t1_up || pt1 == t1_up);

            let tokens_by_category = if !tokens_exact {
                let p_t0_asset = Asset::from_symbol(&p.token0, "score");
                let p_t1_asset = Asset::from_symbol(&p.token1, "score");
                let p_t0_cats = p_t0_asset.category();
                let p_t1_cats = p_t1_asset.category();
                // Check both orderings: user(t0,t1) vs pool(pt0,pt1) or pool(pt1,pt0)
                (p_t0_cats.iter().any(|c| t0_cats.contains(c))
                    && p_t1_cats.iter().any(|c| t1_cats.contains(c)))
                    || (p_t0_cats.iter().any(|c| t1_cats.contains(c))
                        && p_t1_cats.iter().any(|c| t0_cats.contains(c)))
            } else {
                false
            };

            protocol_match && chain_match && fee_match && (tokens_exact || tokens_by_category)
        })
    } else {
        None
    };

    if user_pool_index.is_none() && (req.protocol.is_some() || req.chain.is_some()) {
        tracing::warn!(
            "Pool score: user pool not found for protocol={:?}, chain={:?}, tokens=({}, {})",
            req.protocol,
            req.chain,
            token0_asset.symbol(),
            token1_asset.symbol()
        );
    }

    let your_pool = user_pool_index.map(|idx| pool_to_suggestion(&comparable[idx], idx + 1));
    let score = your_pool.as_ref().map(|p| p.rank);

    let user_apr = user_pool_index
        .map(|idx| comparable[idx].fee_apr_7d)
        .unwrap_or(f64::NEG_INFINITY);

    // Filter suggestions: must be better than user's pool, must have actual volume
    let suggestions: Vec<PoolScoreSuggestion> = comparable
        .iter()
        .enumerate()
        .filter(|(idx, p)| {
            Some(*idx) != user_pool_index && p.fee_apr_7d > user_apr && p.volume_24h_usd > 0.0
        })
        .take(3)
        .map(|(i, p)| pool_to_suggestion(p, i + 1))
        .collect();

    Ok(PoolScoreResponse {
        success: true,
        timestamp: Utc::now(),
        your_pool,
        score,
        total_comparable: comparable.len(),
        normalized_pair: target_normalized,
        token0_category: token0_canonical,
        token1_category: token1_canonical,
        suggestions,
    })
}

fn pool_to_suggestion(p: &PoolResult, rank: usize) -> PoolScoreSuggestion {
    PoolScoreSuggestion {
        rank,
        protocol: p.protocol.clone(),
        chain: p.chain.clone(),
        token0: p.token0.clone(),
        token1: p.token1.clone(),
        pair: p.pair.clone(),
        normalized_pair: p.normalized_pair.clone(),
        fee_tier: p.fee_tier.clone(),
        fee_rate_bps: p.fee_rate_bps,
        tvl_usd: p.tvl_usd,
        volume_24h_usd: p.volume_24h_usd,
        turnover_ratio_24h: p.turnover_ratio_24h,
        fee_apr_24h: p.fee_apr_24h,
        fee_apr_7d: p.fee_apr_7d,
        total_apr: p.total_apr,
        pool_type: p.pool_type.clone(),
        url: p.url.clone(),
        pool_vault_id: p.pool_vault_id.clone(),
    }
}

pub async fn score_lending(
    realtime: &RealtimeService,
    req: &LendingScoreRequest,
) -> Result<LendingScoreResponse> {
    let mut asset_categories_map: HashMap<String, String> = HashMap::new();
    let all_tokens: Vec<&str> = req
        .supplies
        .iter()
        .map(|a| a.token.as_str())
        .chain(req.borrows.iter().map(|a| a.token.as_str()))
        .collect();
    for sym in &all_tokens {
        let asset = Asset::from_symbol(sym, "score");
        asset_categories_map.insert(sym.to_uppercase(), asset.canonical_name());
    }

    let supply_categories: Vec<AssetCategory> = req
        .supplies
        .iter()
        .flat_map(|a| Asset::from_symbol(&a.token, "score").category())
        .collect();

    let borrow_categories: Vec<AssetCategory> = req
        .borrows
        .iter()
        .flat_map(|a| Asset::from_symbol(&a.token, "score").category())
        .collect();

    let supply_query = RateQuery {
        action: Some(Action::Supply),
        assets: None,
        chains: None,
        protocols: None,
        operation_types: Some("lending".to_string()),
        asset_categories: if !supply_categories.is_empty() {
            Some(
                supply_categories
                    .iter()
                    .map(|c| {
                        serde_json::to_value(c)
                            .unwrap_or_default()
                            .as_str()
                            .unwrap_or("")
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            )
        } else {
            None
        },
        token: None,
        min_liquidity: req.min_liquidity,
        page: 1,
        page_size: 100,
    };

    let borrow_query = RateQuery {
        action: Some(Action::Borrow),
        assets: None,
        chains: None,
        protocols: None,
        operation_types: Some("lending".to_string()),
        asset_categories: if !borrow_categories.is_empty() {
            Some(
                borrow_categories
                    .iter()
                    .map(|c| {
                        serde_json::to_value(c)
                            .unwrap_or_default()
                            .as_str()
                            .unwrap_or("")
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            )
        } else {
            None
        },
        token: None,
        min_liquidity: req.min_liquidity,
        page: 1,
        page_size: 100,
    };

    let (supply_rates, borrow_rates) = tokio::join!(
        realtime.query_all_rates(&supply_query),
        realtime.query_all_rates(&borrow_query)
    );
    let supply_rates = supply_rates?;
    let borrow_rates = borrow_rates?;

    let has_values =
        req.supplies.iter().any(|a| a.value > 0.0) || req.borrows.iter().any(|a| a.value > 0.0);

    let mut groups: HashMap<(String, String), Vec<&RateResult>> = HashMap::new();
    for rate in supply_rates.iter().chain(borrow_rates.iter()) {
        let key = (
            serde_json::to_value(&rate.protocol)
                .unwrap_or_default()
                .as_str()
                .unwrap_or("")
                .to_string(),
            serde_json::to_value(&rate.chain)
                .unwrap_or_default()
                .as_str()
                .unwrap_or("")
                .to_string(),
        );
        groups.entry(key).or_default().push(rate);
    }

    let total_assets = req.supplies.len() + req.borrows.len();
    let mut suggestions: Vec<LendingScoreSuggestion> = Vec::new();

    let is_user_group = |proto_str: &str, chain_str: &str| -> bool {
        if let (Some(ref up), Some(ref uc)) = (&req.protocol, &req.chain) {
            let up_str = serde_json::to_value(up)
                .unwrap_or_default()
                .as_str()
                .unwrap_or("")
                .to_string();
            let uc_str = serde_json::to_value(uc)
                .unwrap_or_default()
                .as_str()
                .unwrap_or("")
                .to_string();
            proto_str == up_str && chain_str == uc_str
        } else {
            false
        }
    };

    for ((proto_str, chain_str), rates) in &groups {
        let protocol: Protocol = match serde_json::from_str(&format!("\"{}\"", proto_str)) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let chain: Chain = match serde_json::from_str(&format!("\"{}\"", chain_str)) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let is_user = is_user_group(proto_str, chain_str);

        let mut matched_supply: Vec<LendingAssetRate> = Vec::new();
        let mut matched_borrow: Vec<LendingAssetRate> = Vec::new();

        for sa in &req.supplies {
            let supply_asset = Asset::from_symbol(&sa.token, "score");
            let supply_cats = supply_asset.category();
            let supply_symbol = supply_asset.symbol();

            let best = if is_user {
                let exact = rates
                    .iter()
                    .filter(|r| r.action == Action::Supply)
                    .filter(|r| r.asset.symbol().to_uppercase() == supply_symbol.to_uppercase())
                    .max_by(|a, b| {
                        a.net_apy
                            .partial_cmp(&b.net_apy)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                exact.or_else(|| {
                    rates
                        .iter()
                        .filter(|r| r.action == Action::Supply)
                        .filter(|r| r.asset.category().iter().any(|c| supply_cats.contains(c)))
                        .max_by(|a, b| {
                            a.net_apy
                                .partial_cmp(&b.net_apy)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                })
            } else {
                rates
                    .iter()
                    .filter(|r| r.action == Action::Supply)
                    .filter(|r| r.asset.category().iter().any(|c| supply_cats.contains(c)))
                    .filter(|r| is_healthy_rate(r))
                    .max_by(|a, b| {
                        a.net_apy
                            .partial_cmp(&b.net_apy)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            };

            let cat_name = asset_categories_map
                .get(&sa.token.to_uppercase())
                .cloned()
                .unwrap_or_else(|| "OTHER".to_string());
            if let Some(rate) = best {
                matched_supply.push(LendingAssetRate {
                    asset: if is_user {
                        sa.token.to_uppercase()
                    } else {
                        rate.asset.symbol()
                    },
                    asset_category: cat_name,
                    action: Action::Supply,
                    apy: rate.apy,
                    rewards: rate.rewards,
                    net_apy: rate.net_apy,
                    effective_apy: 0.0,
                    liquidity: rate.liquidity,
                    value_usd: sa.value,
                    url: rate.url.clone(),
                });
            } else if is_user {
                matched_supply.push(LendingAssetRate {
                    asset: sa.token.to_uppercase(),
                    asset_category: cat_name,
                    action: Action::Supply,
                    apy: 0.0,
                    rewards: 0.0,
                    net_apy: 0.0,
                    effective_apy: 0.0,
                    liquidity: 0,
                    value_usd: sa.value,
                    url: String::new(),
                });
            }
        }

        for ba in &req.borrows {
            let borrow_asset = Asset::from_symbol(&ba.token, "score");
            let borrow_cats = borrow_asset.category();
            let borrow_symbol = borrow_asset.symbol();

            let best = if is_user {
                let exact = rates
                    .iter()
                    .filter(|r| r.action == Action::Borrow)
                    .filter(|r| r.asset.symbol().to_uppercase() == borrow_symbol.to_uppercase())
                    .min_by(|a, b| {
                        a.net_apy
                            .partial_cmp(&b.net_apy)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                exact.or_else(|| {
                    rates
                        .iter()
                        .filter(|r| r.action == Action::Borrow)
                        .filter(|r| r.asset.category().iter().any(|c| borrow_cats.contains(c)))
                        .min_by(|a, b| {
                            a.net_apy
                                .partial_cmp(&b.net_apy)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                })
            } else {
                rates
                    .iter()
                    .filter(|r| r.action == Action::Borrow)
                    .filter(|r| r.asset.category().iter().any(|c| borrow_cats.contains(c)))
                    .filter(|r| is_healthy_rate(r))
                    .min_by(|a, b| {
                        a.net_apy
                            .partial_cmp(&b.net_apy)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            };

            let cat_name = asset_categories_map
                .get(&ba.token.to_uppercase())
                .cloned()
                .unwrap_or_else(|| "OTHER".to_string());
            if let Some(rate) = best {
                matched_borrow.push(LendingAssetRate {
                    asset: if is_user {
                        ba.token.to_uppercase()
                    } else {
                        rate.asset.symbol()
                    },
                    asset_category: cat_name,
                    action: Action::Borrow,
                    apy: rate.apy,
                    rewards: rate.rewards,
                    net_apy: rate.net_apy,
                    effective_apy: 0.0,
                    liquidity: rate.liquidity,
                    value_usd: ba.value,
                    url: rate.url.clone(),
                });
            } else if is_user {
                matched_borrow.push(LendingAssetRate {
                    asset: ba.token.to_uppercase(),
                    asset_category: cat_name,
                    action: Action::Borrow,
                    apy: 0.0,
                    rewards: 0.0,
                    net_apy: 0.0,
                    effective_apy: 0.0,
                    liquidity: 0,
                    value_usd: ba.value,
                    url: String::new(),
                });
            }
        }

        let assets_matched = matched_supply.len() + matched_borrow.len();
        if assets_matched == 0 {
            continue;
        }

        let combined = compute_combined_net_apy(&matched_supply, &matched_borrow, has_values);

        // Compute effective_apy: weighted within each side (supply / borrow separately)
        if has_values {
            let total_supply_value: f64 = matched_supply.iter().map(|r| r.value_usd).sum();
            let total_borrow_value: f64 = matched_borrow.iter().map(|r| r.value_usd).sum();
            if total_supply_value > 0.0 {
                for r in &mut matched_supply {
                    r.effective_apy = r.net_apy * r.value_usd / total_supply_value;
                }
            }
            if total_borrow_value > 0.0 {
                for r in &mut matched_borrow {
                    r.effective_apy = r.net_apy * r.value_usd / total_borrow_value;
                }
            }
        }

        suggestions.push(LendingScoreSuggestion {
            rank: 0,
            protocol,
            chain,
            supply_rates: matched_supply,
            borrow_rates: matched_borrow,
            combined_net_apy: combined,
            assets_matched,
            assets_total: total_assets,
        });
    }

    let (user_proto_str, user_chain_str) =
        if let (Some(proto), Some(chain)) = (&req.protocol, &req.chain) {
            (
                serde_json::to_value(proto)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                serde_json::to_value(chain)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
            )
        } else {
            (String::new(), String::new())
        };

    let user_idx = find_user_idx(&suggestions, &user_proto_str, &user_chain_str);

    let user_matched_categories: Vec<String> = user_idx
        .map(|idx| {
            // Only include categories that have actual rates (exclude zero-rate placeholders)
            suggestions[idx]
                .supply_rates
                .iter()
                .filter(|r| r.liquidity > 0 || r.net_apy != 0.0)
                .map(|r| r.asset_category.clone())
                .chain(
                    suggestions[idx]
                        .borrow_rates
                        .iter()
                        .filter(|r| r.liquidity > 0 || r.net_apy != 0.0)
                        .map(|r| r.asset_category.clone()),
                )
                .collect()
        })
        .unwrap_or_default();

    if !user_matched_categories.is_empty() {
        suggestions.retain(|s| {
            let s_cats: Vec<String> = s
                .supply_rates
                .iter()
                .map(|r| r.asset_category.clone())
                .chain(s.borrow_rates.iter().map(|r| r.asset_category.clone()))
                .collect();
            user_matched_categories.iter().all(|c| s_cats.contains(c))
        });
    }

    suggestions.sort_by(|a, b| {
        b.assets_matched.cmp(&a.assets_matched).then_with(|| {
            b.combined_net_apy
                .partial_cmp(&a.combined_net_apy)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    for (i, s) in suggestions.iter_mut().enumerate() {
        s.rank = i + 1;
    }

    let user_idx = find_user_idx(&suggestions, &user_proto_str, &user_chain_str);
    let your_position = user_idx.map(|idx| suggestions[idx].clone());
    let score = your_position.as_ref().map(|p| p.rank);

    let total_comparable = suggestions.len();
    let user_combined = your_position
        .as_ref()
        .map(|p| p.combined_net_apy)
        .unwrap_or(f64::NEG_INFINITY);

    let top3: Vec<LendingScoreSuggestion> = suggestions
        .into_iter()
        .enumerate()
        .filter(|(idx, s)| Some(*idx) != user_idx && s.combined_net_apy > user_combined)
        .map(|(_, s)| s)
        .take(3)
        .collect();

    Ok(LendingScoreResponse {
        success: true,
        timestamp: Utc::now(),
        your_position,
        score,
        total_comparable,
        asset_categories: asset_categories_map,
        suggestions: top3,
    })
}

fn compute_combined_net_apy(
    supply: &[LendingAssetRate],
    borrow: &[LendingAssetRate],
    has_values: bool,
) -> f64 {
    if has_values {
        let total_supply_value: f64 = supply.iter().map(|r| r.value_usd).sum();
        let total_borrow_value: f64 = borrow.iter().map(|r| r.value_usd).sum();
        let total_value = total_supply_value + total_borrow_value;
        if total_value > 0.0 {
            let supply_weighted: f64 = supply.iter().map(|r| r.net_apy * r.value_usd).sum();
            let borrow_weighted: f64 = borrow.iter().map(|r| r.net_apy * r.value_usd).sum();
            (supply_weighted - borrow_weighted) / total_value
        } else {
            simple_combined(supply, borrow)
        }
    } else {
        simple_combined(supply, borrow)
    }
}

fn simple_combined(supply: &[LendingAssetRate], borrow: &[LendingAssetRate]) -> f64 {
    let s: f64 = supply.iter().map(|r| r.net_apy).sum();
    let b: f64 = borrow.iter().map(|r| r.net_apy).sum();
    s - b
}

/// Returns true if this rate represents a healthy, usable reserve.
/// Filters out reserves where:
/// - Not active (frozen, paused, or borrowing disabled)
/// - APY is 0% (nobody uses it or it's frozen)
/// - Utilization is 100% (fully borrowed, no available liquidity)
/// - Utilization is 0% (nobody borrows from it — dead market)
/// - Available liquidity is 0 (can't actually supply or borrow)
fn is_healthy_rate(rate: &RateResult) -> bool {
    if !rate.active {
        return false;
    }
    if rate.net_apy == 0.0 && rate.apy == 0.0 {
        return false;
    }
    if rate.utilization_rate >= 100 || rate.utilization_rate == 0 {
        return false;
    }
    if rate.liquidity == 0 {
        return false;
    }
    true
}

fn find_user_idx(
    suggestions: &[LendingScoreSuggestion],
    proto: &str,
    chain: &str,
) -> Option<usize> {
    if proto.is_empty() {
        return None;
    }
    suggestions.iter().position(|s| {
        let sp = serde_json::to_value(&s.protocol)
            .unwrap_or_default()
            .as_str()
            .unwrap_or("")
            .to_string();
        let sc = serde_json::to_value(&s.chain)
            .unwrap_or_default()
            .as_str()
            .unwrap_or("")
            .to_string();
        sp == proto && sc == chain
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rate(active: bool, net_apy: f64, apy: f64, util: u32, liquidity: u64) -> RateResult {
        RateResult {
            protocol: Protocol::Aave,
            chain: Chain::Ethereum,
            asset: Asset::from_symbol("USDC", "test"),
            action: Action::Supply,
            asset_category: vec![AssetCategory::UsdCorrelated],
            apy,
            rewards: 0.0,
            net_apy,
            performance_fee: None,
            active,
            collateral_enabled: true,
            collateral_ltv: 0.75,
            liquidity,
            total_liquidity: liquidity,
            utilization_rate: util,
            operation_type: OperationType::Lending,
            url: "https://example.com".to_string(),
            vault_id: None,
            vault_name: None,
            last_update: chrono::Utc::now(),
            apy_metrics: None,
        }
    }

    // ====================================================================
    // is_healthy_rate
    // ====================================================================

    #[test]
    fn test_healthy_rate_normal() {
        let rate = make_rate(true, 5.0, 5.0, 50, 1_000_000);
        assert!(is_healthy_rate(&rate));
    }

    #[test]
    fn test_unhealthy_rate_inactive() {
        let rate = make_rate(false, 5.0, 5.0, 50, 1_000_000);
        assert!(!is_healthy_rate(&rate));
    }

    #[test]
    fn test_unhealthy_rate_zero_apy() {
        let rate = make_rate(true, 0.0, 0.0, 50, 1_000_000);
        assert!(!is_healthy_rate(&rate));
    }

    #[test]
    fn test_unhealthy_rate_full_utilization() {
        let rate = make_rate(true, 5.0, 5.0, 100, 1_000_000);
        assert!(!is_healthy_rate(&rate));
    }

    #[test]
    fn test_unhealthy_rate_zero_utilization() {
        let rate = make_rate(true, 5.0, 5.0, 0, 1_000_000);
        assert!(!is_healthy_rate(&rate));
    }

    #[test]
    fn test_unhealthy_rate_zero_liquidity() {
        let rate = make_rate(true, 5.0, 5.0, 50, 0);
        assert!(!is_healthy_rate(&rate));
    }

    #[test]
    fn test_healthy_with_only_rewards() {
        // net_apy > 0 but base apy = 0 → still healthy if rewards exist
        let rate = make_rate(true, 3.0, 0.0, 50, 1_000_000);
        assert!(is_healthy_rate(&rate));
    }

    // ====================================================================
    // compute_combined_net_apy / simple_combined
    // ====================================================================

    fn make_lending_rate(action: Action, net_apy: f64, value: f64) -> LendingAssetRate {
        LendingAssetRate {
            asset: "USDC".to_string(),
            asset_category: "USD".to_string(),
            action,
            apy: net_apy,
            rewards: 0.0,
            net_apy,
            effective_apy: 0.0,
            liquidity: 1_000_000,
            value_usd: value,
            url: String::new(),
        }
    }

    #[test]
    fn test_simple_combined_supply_only() {
        let supply = vec![
            make_lending_rate(Action::Supply, 5.0, 0.0),
            make_lending_rate(Action::Supply, 3.0, 0.0),
        ];
        let borrow: Vec<LendingAssetRate> = vec![];
        // simple: sum(supply) - sum(borrow) = 8.0
        assert!((simple_combined(&supply, &borrow) - 8.0).abs() < 0.001);
    }

    #[test]
    fn test_simple_combined_supply_and_borrow() {
        let supply = vec![make_lending_rate(Action::Supply, 5.0, 0.0)];
        let borrow = vec![make_lending_rate(Action::Borrow, 3.0, 0.0)];
        assert!((simple_combined(&supply, &borrow) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_combined_with_values_weighted() {
        let supply = vec![
            make_lending_rate(Action::Supply, 10.0, 1000.0),
            make_lending_rate(Action::Supply, 2.0, 9000.0),
        ];
        let borrow = vec![make_lending_rate(Action::Borrow, 5.0, 5000.0)];
        // has_values = true
        // supply_weighted = 10*1000 + 2*9000 = 28000
        // borrow_weighted = 5*5000 = 25000
        // total_value = 10000 + 5000 = 15000
        // combined = (28000 - 25000) / 15000 = 0.2
        let result = compute_combined_net_apy(&supply, &borrow, true);
        assert!((result - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_compute_combined_without_values_uses_simple() {
        let supply = vec![make_lending_rate(Action::Supply, 5.0, 0.0)];
        let borrow = vec![make_lending_rate(Action::Borrow, 3.0, 0.0)];
        let result = compute_combined_net_apy(&supply, &borrow, false);
        assert!((result - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_combined_zero_values_fallback() {
        let supply = vec![make_lending_rate(Action::Supply, 5.0, 0.0)];
        let borrow = vec![make_lending_rate(Action::Borrow, 3.0, 0.0)];
        // has_values = true but all values are 0.0 → total_value = 0 → fallback to simple
        let result = compute_combined_net_apy(&supply, &borrow, true);
        assert!((result - 2.0).abs() < 0.001);
    }

    // ====================================================================
    // find_user_idx
    // ====================================================================

    #[test]
    fn test_find_user_idx_empty_proto() {
        let suggestions: Vec<LendingScoreSuggestion> = vec![];
        assert!(find_user_idx(&suggestions, "", "ethereum").is_none());
    }

    // ====================================================================
    // pool_to_suggestion
    // ====================================================================

    #[test]
    fn test_pool_to_suggestion_rank() {
        let pool = PoolResult {
            protocol: Protocol::Uniswap,
            chain: Chain::Ethereum,
            token0: "ETH".to_string(),
            token1: "USDC".to_string(),
            pair: "ETH/USDC".to_string(),
            normalized_pair: "ETH/USD".to_string(),
            token0_categories: vec![AssetCategory::EthCorrelated],
            token1_categories: vec![AssetCategory::UsdCorrelated],
            pool_type: PoolType::ConcentratedLiquidity,
            fee_tier: "0.30%".to_string(),
            fee_rate_bps: 30,
            tvl_usd: 50_000_000.0,
            volume_24h_usd: 10_000_000.0,
            volume_7d_usd: 70_000_000.0,
            turnover_ratio_24h: 0.2,
            turnover_ratio_7d: 0.2,
            fees_24h_usd: 3000.0,
            fees_7d_usd: 21000.0,
            fee_apr_24h: 21.9,
            fee_apr_7d: 15.33,
            rewards_apr: 0.0,
            total_apr: 21.9,
            pool_address: "0xabc".to_string(),
            url: "https://app.uniswap.org".to_string(),
            last_update: chrono::Utc::now(),
            pool_vault_id: "abc123".to_string(),
        };

        let suggestion = pool_to_suggestion(&pool, 5);
        assert_eq!(suggestion.rank, 5);
        assert_eq!(suggestion.protocol, Protocol::Uniswap);
        assert_eq!(suggestion.chain, Chain::Ethereum);
        assert_eq!(suggestion.token0, "ETH");
        assert_eq!(suggestion.token1, "USDC");
        assert_eq!(suggestion.fee_rate_bps, 30);
        assert!((suggestion.fee_apr_7d - 15.33).abs() < 0.01);
    }
}
