import axios from 'axios'

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:8080'

export interface RateResult {
  protocol: string
  chain: string
  asset: string
  assetCategory: Array<'usd-correlated' | 'stablecoin' | 'btc-correlated' | 'eth-correlated' | 'sol-correlated' | 'other'>
  apy: number
  rewards: number
  netApy: number
  liquidity: number
  totalLiquidity?: number
  utilizationRate: number
  operationType: string
  url: string
  vaultName?: string
  vaultId?: string
  lastUpdate: string
  collateralLtv: number
  apyMetrics?: {
    instant: number
    apy7d?: number
    apy30d?: number
    apy60d?: number
    apy90d?: number
    volatility: number
    daysWithData: number
  }
}

export interface RateResponse {
  success: boolean
  timestamp: string
  results: RateResult[]
  count: number
  page: number
  pageSize: number
  totalCount: number
  totalPages: number
}

export interface SearchRatesParams {
  action: 'supply' | 'borrow'
  chains?: string
  protocols?: string
  operation_types?: string
  asset_categories?: string
  token?: string
  min_liquidity?: number
  page?: number
  page_size?: number
}

// ─── Vault History (APY chart) ───────────────────────────────────────────────

export interface VaultHistoryPoint {
  date: string           // ISO-8601
  net_apy: number
  base_apy: number
  rewards_apy: number
  liquidity_usd: number
  utilization_rate: number
}

export interface VaultHistoryData {
  success: boolean
  vault_id: string
  vault_name?: string
  protocol?: string
  chain?: string
  asset?: string
  operation_type?: string
  url?: string
  days: number
  points: VaultHistoryPoint[]
  avg_apy: number
  min_apy: number
  max_apy: number
  data_available: boolean
}

export interface FetchVaultHistoryParams {
  vault_id?: string
  protocol?: string
  chain?: string
  asset?: string
  days?: number
}

// ─── API calls ───────────────────────────────────────────────────────────────

export async function searchRates(params: SearchRatesParams): Promise<RateResponse> {
  const response = await axios.get<RateResponse>(`${API_URL}/api/v1/rates`, {
    params,
  })
  return response.data
}

export async function fetchVaultHistory(
  params: FetchVaultHistoryParams,
): Promise<VaultHistoryData> {
  const response = await axios.get<VaultHistoryData>(`${API_URL}/api/v1/rates/history`, {
    params,
  })
  return response.data
}

// ─── Liquidity Pool Types ─────────────────────────────────────────────────────

export type AssetCategory = 'usd-correlated' | 'stablecoin' | 'btc-correlated' | 'eth-correlated' | 'sol-correlated' | 'other'

export interface PoolResult {
  protocol: string
  chain: string
  token0: string
  token1: string
  pair: string
  normalizedPair: string
  token0Categories: AssetCategory[]
  token1Categories: AssetCategory[]
  poolType: 'standard' | 'concentrated'
  feeTier: string
  feeRateBps: number
  tvlUsd: number
  volume24h: number
  volume7d: number
  turnoverRatio24h: number
  turnoverRatio7d: number
  fees24h: number
  fees7d: number
  feeApr24h: number
  feeApr7d: number
  rewardsApr: number
  totalApr: number
  poolAddress: string
  url: string
  lastUpdate: string
  poolVaultId: string
}

export interface PoolResponse {
  success: boolean
  timestamp: string
  results: PoolResult[]
  count: number
  page: number
  pageSize: number
  totalCount: number
  totalPages: number
}

export interface SearchPoolsParams {
  asset_categories_0?: string
  asset_categories_1?: string
  token_a?: string
  token_b?: string
  token?: string
  pair?: string
  chains?: string
  protocols?: string
  pool_type?: string
  min_tvl?: number
  min_volume?: number
  page?: number
  page_size?: number
}

export interface PoolHistoryPoint {
  date: string
  tvl_usd: number
  volume_24h_usd: number
  fee_rate_bps: number
  turnover_ratio_24h: number
  fee_apr_24h: number
  fee_apr_7d: number
  rewards_apr: number
}

export interface PoolHistoryData {
  success: boolean
  pool_vault_id: string
  pair?: string
  protocol?: string
  chain?: string
  url?: string
  days: number
  points: PoolHistoryPoint[]
  avg_fee_apr: number
  min_fee_apr: number
  max_fee_apr: number
  avg_tvl: number
  data_available: boolean
}

// ─── Pool API calls ──────────────────────────────────────────────────────────

export async function searchPools(params: SearchPoolsParams): Promise<PoolResponse> {
  const response = await axios.get<PoolResponse>(`${API_URL}/api/v1/pools`, {
    params,
  })
  return response.data
}

export async function fetchPoolHistory(
  params: { pool_vault_id?: string; protocol?: string; chain?: string; pair?: string },
): Promise<PoolHistoryData> {
  const response = await axios.get<PoolHistoryData>(`${API_URL}/api/v1/pools/history`, {
    params,
  })
  return response.data
}
