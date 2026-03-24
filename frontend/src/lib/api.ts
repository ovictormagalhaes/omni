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
}

export interface SearchRatesParams {
  action: 'supply' | 'borrow'
  assets?: string
  chains?: string
  protocols?: string
  operation_types?: string
  asset_categories?: string
  min_liquidity?: number
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

// ─── Available Assets ─────────────────────────────────────────────────────────

export interface AssetInfo {
  symbol: string
  categories: string[]
}

export interface AssetsResponse {
  success: boolean
  assets: AssetInfo[]
  count: number
}

// Cache a nível de módulo — a requisição HTTP acontece apenas uma vez por sessão
const _assetsCache: Promise<AssetInfo[]> = (async () => {
  try {
    const response = await axios.get<AssetsResponse>(`${API_URL}/api/v1/assets`)
    return response.data.assets
  } catch {
    return []
  }
})()

export async function fetchAvailableAssets(): Promise<AssetInfo[]> {
  return _assetsCache
}

export async function fetchVaultHistory(
  params: FetchVaultHistoryParams,
): Promise<VaultHistoryData> {
  const response = await axios.get<VaultHistoryData>(`${API_URL}/api/v1/rates/history`, {
    params,
  })
  return response.data
}
