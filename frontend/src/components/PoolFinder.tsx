import { useState, useEffect, useRef } from 'react'
import { Search, ChevronDown, ExternalLink, BarChart2, SlidersHorizontal, Info } from 'lucide-react'
import { searchPools, PoolResult } from '../lib/api'
import { ProtocolIcon } from './ProtocolIcon'
import { ChainIcon } from './ChainIcon'
import PoolDetailDrawer from './PoolDetailDrawer'
import AssetCategoryFilter, { type AssetCategory } from './AssetCategoryFilter'

type PoolProtocol = 'uniswap' | 'uniswapv4' | 'raydium' | 'curve' | 'pancakeswap' | 'aerodrome' | 'velodrome' | 'orca' | 'meteora' | 'sushiswap' | 'camelot' | 'traderjoe' | 'balancer' | 'maverick'
type PoolChain = 'ethereum' | 'solana' | 'arbitrum' | 'base' | 'polygon' | 'optimism' | 'avalanche' | 'fantom' | 'bsc' | 'celo'
type PoolTypeValue = 'concentrated' | 'standard'

const POOL_TYPES: { value: PoolTypeValue; label: string; description: string }[] = [
  { value: 'concentrated', label: 'CLMM', description: 'Concentrated Liquidity (Uniswap V3 style) — capital efficient, higher fees per $ of liquidity' },
  { value: 'standard', label: 'Standard', description: 'Classic x*y=k AMM — simpler, full-range liquidity' },
]

const POOL_CHAINS: { value: PoolChain; label: string }[] = [
  { value: 'arbitrum', label: 'Arbitrum' },
  { value: 'avalanche', label: 'Avalanche' },
  { value: 'base', label: 'Base' },
  { value: 'bsc', label: 'BNB Chain' },
  { value: 'celo', label: 'Celo' },
  { value: 'ethereum', label: 'Ethereum' },
  { value: 'fantom', label: 'Fantom' },
  { value: 'optimism', label: 'Optimism' },
  { value: 'polygon', label: 'Polygon' },
  { value: 'solana', label: 'Solana' },
]

const POOL_PROTOCOLS: { value: PoolProtocol; label: string }[] = [
  { value: 'aerodrome', label: 'Aerodrome' },
  { value: 'curve', label: 'Curve' },
  { value: 'meteora', label: 'Meteora' },
  { value: 'orca', label: 'Orca' },
  { value: 'pancakeswap', label: 'PancakeSwap' },
  { value: 'raydium', label: 'Raydium' },
  { value: 'uniswap', label: 'Uniswap V3' },
  { value: 'uniswapv4', label: 'Uniswap V4' },
  { value: 'velodrome', label: 'Velodrome' },
  { value: 'sushiswap', label: 'SushiSwap' },
  { value: 'camelot', label: 'Camelot' },
  { value: 'traderjoe', label: 'Trader Joe' },
  { value: 'balancer', label: 'Balancer' },
  { value: 'maverick', label: 'Maverick' },
]


function formatUsd(value: number): string {
  if (value >= 1e9) return `$${(value / 1e9).toFixed(2)}B`
  if (value >= 1e6) return `$${(value / 1e6).toFixed(2)}M`
  if (value >= 1e3) return `$${(value / 1e3).toFixed(1)}K`
  return `$${value.toFixed(0)}`
}

function formatApr(value: number): string {
  return `${value.toFixed(2)}%`
}

function formatTurnover(value: number): string {
  if (value >= 1) return `${value.toFixed(2)}x`
  return `${(value * 100).toFixed(1)}%`
}

export default function PoolFinder() {
  const [tokenAMode, setTokenAMode] = useState<'category' | 'name'>('category')
  const [tokenBMode, setTokenBMode] = useState<'category' | 'name'>('category')
  const [selectedCategory0, setSelectedCategory0] = useState<AssetCategory | null>(null)
  const [selectedCategory1, setSelectedCategory1] = useState<AssetCategory | null>(null)
  const [searchTokenA, setSearchTokenA] = useState('')
  const [searchTokenB, setSearchTokenB] = useState('')
  const [selectedChains, setSelectedChains] = useState<PoolChain[]>(POOL_CHAINS.map(c => c.value))
  const [selectedProtocols, setSelectedProtocols] = useState<PoolProtocol[]>(POOL_PROTOCOLS.map(p => p.value))
  const [selectedPoolTypes, setSelectedPoolTypes] = useState<PoolTypeValue[]>(POOL_TYPES.map(p => p.value))
  const [minTvl, setMinTvl] = useState(100000)
  const [minVolume, setMinVolume] = useState(100000)
  const [results, setResults] = useState<PoolResult[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [page, setPage] = useState(1)
  const [totalCount, setTotalCount] = useState(0)
  const [hasMore, setHasMore] = useState(false)
  const PAGE_SIZE = 20
  const [isChainDropdownOpen, setIsChainDropdownOpen] = useState(false)
  const [isProtocolDropdownOpen, setIsProtocolDropdownOpen] = useState(false)
  const [selectedPool, setSelectedPool] = useState<PoolResult | null>(null)
  const [feePeriod, setFeePeriod] = useState<'24h' | '7d'>('24h')
  const chainDropdownRef = useRef<HTMLDivElement>(null)
  const protocolDropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (chainDropdownRef.current && !chainDropdownRef.current.contains(e.target as Node)) setIsChainDropdownOpen(false)
      if (protocolDropdownRef.current && !protocolDropdownRef.current.contains(e.target as Node)) setIsProtocolDropdownOpen(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  const toggleChain = (chain: PoolChain) => {
    setSelectedChains(prev => prev.includes(chain) ? prev.filter(c => c !== chain) : [...prev, chain])
  }

  const toggleProtocol = (protocol: PoolProtocol) => {
    setSelectedProtocols(prev => prev.includes(protocol) ? prev.filter(p => p !== protocol) : [...prev, protocol])
  }

  const togglePoolType = (pt: PoolTypeValue) => {
    setSelectedPoolTypes(prev => prev.includes(pt) ? prev.filter(t => t !== pt) : [...prev, pt])
  }

  const renderChainLabel = () => {
    if (selectedChains.length === POOL_CHAINS.length) return 'All Chains'
    if (selectedChains.length === 1) {
      const chain = POOL_CHAINS.find(c => c.value === selectedChains[0])
      return (
        <span className="flex items-center">
          <ChainIcon chain={selectedChains[0]} className="w-4 h-4 mr-2" />
          {chain?.label || 'Select chains'}
        </span>
      )
    }
    return `${selectedChains.length} chains selected`
  }

  const renderProtocolLabel = () => {
    if (selectedProtocols.length === POOL_PROTOCOLS.length) return 'All Protocols'
    if (selectedProtocols.length === 1) {
      const protocol = POOL_PROTOCOLS.find(p => p.value === selectedProtocols[0])
      return (
        <span className="flex items-center">
          <ProtocolIcon protocol={selectedProtocols[0]} className="w-4 h-4 mr-2" />
          {protocol?.label || 'Select protocols'}
        </span>
      )
    }
    return `${selectedProtocols.length} protocols selected`
  }

  const buildParams = (targetPage: number): Record<string, string | number> => {
    const params: Record<string, string | number> = {
      min_tvl: minTvl,
      page: targetPage,
      page_size: PAGE_SIZE,
    }
    if (minVolume > 0) params.min_volume = minVolume
    if (selectedCategory0) params.asset_categories_0 = selectedCategory0
    if (selectedCategory1) params.asset_categories_1 = selectedCategory1
    if (searchTokenA.trim()) params.token_a = searchTokenA.trim()
    if (searchTokenB.trim()) params.token_b = searchTokenB.trim()
    if (selectedChains.length > 0 && selectedChains.length < POOL_CHAINS.length) params.chains = selectedChains.join(',')
    if (selectedProtocols.length > 0 && selectedProtocols.length < POOL_PROTOCOLS.length) params.protocols = selectedProtocols.join(',')
    if (selectedPoolTypes.length === 1) params.pool_type = selectedPoolTypes[0]
    return params
  }

  const handleSearch = async () => {
    setLoading(true)
    setError(null)
    setIsChainDropdownOpen(false)
    setIsProtocolDropdownOpen(false)

    try {
      const data = await searchPools(buildParams(1))
      setResults(data.results)
      setPage(1)
      setTotalCount(data.totalCount)
      setHasMore(data.page < data.totalPages)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to fetch pools')
    } finally {
      setLoading(false)
    }
  }

  const handleLoadMore = async () => {
    const nextPage = page + 1
    setLoading(true)
    try {
      const data = await searchPools(buildParams(nextPage))
      setResults(prev => [...prev, ...data.results])
      setPage(nextPage)
      setHasMore(data.page < data.totalPages)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to load more')
    } finally {
      setLoading(false)
    }
  }


  return (
    <div className="space-y-6" id="pools">
      {/* ─── Filters Card ─────────────────────────────────────────────── */}
      <div className="card">
        <div className="flex items-center gap-3 mb-6">
          <div className="w-10 h-10 rounded-xl bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center">
            <SlidersHorizontal className="w-5 h-5 text-emerald-400" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-white">Liquidity Finder</h2>
            <p className="text-xs text-slate-500">Compare liquidity pools across DEXes and chains</p>
          </div>
        </div>

        {/* Filters grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-5">
          {/* Chain Filters */}
          <div className="relative" ref={chainDropdownRef}>
            <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Chains</label>
            <button
              onClick={() => { setIsChainDropdownOpen(!isChainDropdownOpen); setIsProtocolDropdownOpen(false) }}
              className="input-field flex items-center justify-between"
            >
              <span className="text-sm">{renderChainLabel()}</span>
              <ChevronDown className={`w-4 h-4 text-slate-500 transition-transform ${isChainDropdownOpen ? 'rotate-180' : ''}`} />
            </button>
            {isChainDropdownOpen && (
              <div className="dropdown-menu max-h-80">
                <div className="p-2">
                  <div className="flex gap-2 mb-2 pb-2 border-b border-slate-700/50">
                    <button onClick={() => setSelectedChains(POOL_CHAINS.map(c => c.value))} className="flex-1 px-3 py-1.5 text-xs font-medium bg-emerald-500/10 text-emerald-400 rounded-lg hover:bg-emerald-500/20 transition-colors">Select All</button>
                    <button onClick={() => setSelectedChains([])} className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-700/50 text-slate-400 rounded-lg hover:bg-slate-700 transition-colors">Clear</button>
                  </div>
                  {POOL_CHAINS.map(chain => (
                    <label key={chain.value} className="flex items-center px-3 py-2 hover:bg-slate-700/50 rounded-lg cursor-pointer transition-colors">
                      <input type="checkbox" checked={selectedChains.includes(chain.value)} onChange={() => toggleChain(chain.value)} className="mr-3 w-3.5 h-3.5 text-emerald-500 bg-slate-700 border-slate-600 rounded focus:ring-emerald-500" />
                      <ChainIcon chain={chain.value} className="w-4 h-4 mr-2" />
                      <span className="text-sm text-slate-300">{chain.label}</span>
                    </label>
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* Min TVL */}
          <div>
            <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Min TVL</label>
            <select
              value={minTvl}
              onChange={e => setMinTvl(Number(e.target.value))}
              className="input-field text-sm"
            >
              <option value={10000}>$10K+</option>
              <option value={100000}>$100K+</option>
              <option value={1000000}>$1M+</option>
              <option value={10000000}>$10M+</option>
            </select>
          </div>

          {/* Min Volume 24h */}
          <div>
            <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Min Volume 24h</label>
            <select
              value={minVolume}
              onChange={e => setMinVolume(Number(e.target.value))}
              className="input-field text-sm"
            >
              <option value={0}>Any</option>
              <option value={10000}>$10K+</option>
              <option value={100000}>$100K+</option>
              <option value={1000000}>$1M+</option>
              <option value={10000000}>$10M+</option>
            </select>
          </div>

          {/* Protocol Filters */}
          <div className="relative" ref={protocolDropdownRef}>
            <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Protocols</label>
            <button
              onClick={() => { setIsProtocolDropdownOpen(!isProtocolDropdownOpen); setIsChainDropdownOpen(false) }}
              className="input-field flex items-center justify-between"
            >
              <span className="text-sm">{renderProtocolLabel()}</span>
              <ChevronDown className={`w-4 h-4 text-slate-500 transition-transform ${isProtocolDropdownOpen ? 'rotate-180' : ''}`} />
            </button>
            {isProtocolDropdownOpen && (
              <div className="dropdown-menu">
                <div className="p-2">
                  <div className="flex gap-2 mb-2 pb-2 border-b border-slate-700/50">
                    <button onClick={() => setSelectedProtocols(POOL_PROTOCOLS.map(p => p.value))} className="flex-1 px-3 py-1.5 text-xs font-medium bg-emerald-500/10 text-emerald-400 rounded-lg hover:bg-emerald-500/20 transition-colors">Select All</button>
                    <button onClick={() => setSelectedProtocols([])} className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-700/50 text-slate-400 rounded-lg hover:bg-slate-700 transition-colors">Clear</button>
                  </div>
                  {POOL_PROTOCOLS.map(protocol => (
                    <label key={protocol.value} className="flex items-center px-3 py-2 hover:bg-slate-700/50 rounded-lg cursor-pointer transition-colors">
                      <input type="checkbox" checked={selectedProtocols.includes(protocol.value)} onChange={() => toggleProtocol(protocol.value)} className="mr-3 w-3.5 h-3.5 text-emerald-500 bg-slate-700 border-slate-600 rounded focus:ring-emerald-500" />
                      <ProtocolIcon protocol={protocol.value} className="w-4 h-4 mr-2" />
                      <span className="text-sm text-slate-300">{protocol.label}</span>
                    </label>
                  ))}
                </div>
              </div>
            )}
          </div>

        </div>

        {/* Quick picks */}
        <div className="flex flex-wrap gap-2 mt-4 pt-4 border-t border-slate-700/30 mb-5">
          <span className="text-xs text-slate-500 self-center mr-1">Quick:</span>
          {[
            { label: 'Ethereum Only', chains: ['ethereum'] as PoolChain[] },
            { label: 'Solana Only', chains: ['solana'] as PoolChain[] },
            { label: 'EVM Chains', chains: ['ethereum', 'arbitrum', 'base', 'polygon', 'optimism'] as PoolChain[] },
            { label: 'All Chains', chains: POOL_CHAINS.map(c => c.value) },
          ].map(q => (
            <button key={q.label} onClick={() => { setSelectedChains(q.chains); setSelectedProtocols(POOL_PROTOCOLS.map(p => p.value)) }}
              className="px-3 py-1 text-xs font-medium bg-slate-800/60 text-slate-400 border border-slate-700/30 rounded-lg hover:text-white hover:border-slate-600 transition-colors"
            >{q.label}</button>
          ))}
        </div>

        {/* Pool Type + Asset Category */}
        <div className="space-y-4 mb-5">
          <div>
            <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Pool Type</label>
            <div className="flex flex-wrap gap-2">
              {POOL_TYPES.map(opt => (
                <div key={opt.value} className="relative group/tooltip">
                  <button
                    onClick={() => togglePoolType(opt.value)}
                    className={`flex items-center gap-1.5 px-3.5 py-2 rounded-lg text-sm font-medium transition-all duration-200 ${
                      selectedPoolTypes.includes(opt.value)
                        ? 'bg-emerald-500/15 text-emerald-400 border border-emerald-500/30'
                        : 'bg-slate-800/60 text-slate-500 border border-slate-700/30 hover:text-slate-300 hover:border-slate-600'
                    }`}
                  >
                    {opt.label}
                    <Info className="w-3 h-3 opacity-40 group-hover/tooltip:opacity-70 transition-opacity" />
                  </button>
                  <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-xs text-slate-300 w-56 opacity-0 invisible group-hover/tooltip:opacity-100 group-hover/tooltip:visible transition-all duration-200 z-50 pointer-events-none shadow-xl">
                    {opt.description}
                    <div className="absolute top-full left-1/2 -translate-x-1/2 -mt-px w-2 h-2 bg-slate-900 border-r border-b border-slate-700 rotate-45" />
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Token A filter */}
          <div>
            <div className="flex items-center gap-3 mb-2">
              <label className="text-xs font-medium text-slate-500 uppercase tracking-wider">Token A</label>
              <div className="flex rounded-lg overflow-hidden border border-slate-700/30">
                <button
                  onClick={() => { setTokenAMode('category'); setSearchTokenA('') }}
                  className={`px-3 py-1 text-[10px] font-medium transition-all duration-200 ${
                    tokenAMode === 'category'
                      ? 'bg-emerald-500/15 text-emerald-400'
                      : 'bg-slate-800/60 text-slate-500 hover:text-slate-300'
                  }`}
                >Category</button>
                <button
                  onClick={() => { setTokenAMode('name'); setSelectedCategory0(null) }}
                  className={`px-3 py-1 text-[10px] font-medium transition-all duration-200 ${
                    tokenAMode === 'name'
                      ? 'bg-emerald-500/15 text-emerald-400'
                      : 'bg-slate-800/60 text-slate-500 hover:text-slate-300'
                  }`}
                >Name</button>
              </div>
            </div>
            {tokenAMode === 'category' ? (
              <AssetCategoryFilter selected={selectedCategory0} onSelect={setSelectedCategory0} label="" />
            ) : (
              <input
                type="text"
                value={searchTokenA}
                onChange={e => setSearchTokenA(e.target.value)}
                placeholder="e.g. ETH, WBTC, cbBTC..."
                className="input-field text-sm"
              />
            )}
          </div>

          {/* Token B filter */}
          <div>
            <div className="flex items-center gap-3 mb-2">
              <label className="text-xs font-medium text-slate-500 uppercase tracking-wider">Token B</label>
              <div className="flex rounded-lg overflow-hidden border border-slate-700/30">
                <button
                  onClick={() => { setTokenBMode('category'); setSearchTokenB('') }}
                  className={`px-3 py-1 text-[10px] font-medium transition-all duration-200 ${
                    tokenBMode === 'category'
                      ? 'bg-emerald-500/15 text-emerald-400'
                      : 'bg-slate-800/60 text-slate-500 hover:text-slate-300'
                  }`}
                >Category</button>
                <button
                  onClick={() => { setTokenBMode('name'); setSelectedCategory1(null) }}
                  className={`px-3 py-1 text-[10px] font-medium transition-all duration-200 ${
                    tokenBMode === 'name'
                      ? 'bg-emerald-500/15 text-emerald-400'
                      : 'bg-slate-800/60 text-slate-500 hover:text-slate-300'
                  }`}
                >Name</button>
              </div>
            </div>
            {tokenBMode === 'category' ? (
              <AssetCategoryFilter selected={selectedCategory1} onSelect={setSelectedCategory1} label="" />
            ) : (
              <input
                type="text"
                value={searchTokenB}
                onChange={e => setSearchTokenB(e.target.value)}
                placeholder="e.g. USDC, USDT, DAI..."
                className="input-field text-sm"
              />
            )}
          </div>
        </div>

        {/* Search Button */}
        <button
          onClick={handleSearch}
          disabled={loading || selectedChains.length === 0 || selectedProtocols.length === 0 || selectedPoolTypes.length === 0}
          className="btn-primary w-full disabled:opacity-40 disabled:cursor-not-allowed disabled:shadow-none"
        >
          {loading ? (
            <span className="flex items-center justify-center gap-2">
              <svg className="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8z" />
              </svg>
              Searching...
            </span>
          ) : (
            <span className="flex items-center justify-center gap-2">
              <Search className="w-4 h-4" />
              Search Pools
            </span>
          )}
        </button>

        {error && (
          <div className="mt-4 p-3 bg-red-500/10 border border-red-500/20 rounded-xl text-red-400 text-sm">
            {error}
          </div>
        )}
      </div>

      {/* ─── Results ──────────────────────────────────────────────────── */}
      {results.length > 0 && (
        <div className="card">
          <div className="flex items-center justify-between mb-6">
            <h3 className="text-lg font-bold text-white flex items-center gap-2">
              <BarChart2 className="w-5 h-5 text-emerald-400" />
              Best Pools
            </h3>
            <div className="flex items-center gap-3">
              <div className="flex gap-0.5 p-0.5 bg-slate-800/60 rounded-lg">
                {(['24h', '7d'] as const).map(p => (
                  <button
                    key={p}
                    onClick={() => setFeePeriod(p)}
                    className={`px-2.5 py-1 rounded text-xs font-medium transition-all ${
                      feePeriod === p
                        ? 'bg-emerald-500/20 text-emerald-400'
                        : 'text-slate-500 hover:text-slate-300'
                    }`}
                  >
                    {p}
                  </button>
                ))}
              </div>
              <span className="text-xs text-slate-500 bg-slate-800/60 px-3 py-1 rounded-full">
                {results.length} of {totalCount}
              </span>
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
              {results.map((pool, i) => (
                <div
                  key={`${pool.poolVaultId}-${i}`}
                  className="group rounded-xl bg-slate-800/40 border border-slate-700/40 p-4 hover:border-emerald-500/30 hover:bg-slate-800/60 transition-all duration-300 cursor-pointer"
                  onClick={() => setSelectedPool(pool)}
                >
                  {/* Header */}
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center gap-2">
                      <ProtocolIcon protocol={pool.protocol} className="w-5 h-5" />
                      <span className="text-white font-semibold text-sm capitalize">{pool.protocol}</span>
                      <span className={`px-2 py-0.5 rounded-full text-[10px] font-semibold ${
                        pool.poolType === 'concentrated'
                          ? 'bg-purple-500/10 text-purple-400 border border-purple-500/20'
                          : 'bg-slate-500/10 text-slate-400 border border-slate-500/20'
                      }`}>
                        {pool.poolType === 'concentrated' ? 'CLMM' : 'AMM'}
                      </span>
                      <span className="px-2 py-0.5 rounded-full text-[10px] font-semibold bg-slate-500/10 text-slate-400 border border-slate-500/20">
                        {pool.feeTier}
                      </span>
                    </div>
                    <div className="flex items-center gap-1.5">
                      <ChainIcon chain={pool.chain} className="w-3.5 h-3.5" />
                      <span className="text-[10px] text-slate-500 capitalize">{pool.chain}</span>
                    </div>
                  </div>

                  {/* Pair + Fee APR */}
                  <div className="flex items-center justify-between mb-3 pb-3 border-b border-slate-700/30">
                    <span className="text-xl font-mono font-bold text-white">
                      {pool.token0}<span className="text-slate-600">/</span>{pool.token1}
                    </span>
                    <div className="text-right">
                      <p className="text-lg font-bold text-emerald-400">{formatApr(feePeriod === '24h' ? pool.feeApr24h : pool.feeApr7d)}</p>
                      <p className="text-[10px] text-slate-500">Fee APR {feePeriod}</p>
                    </div>
                  </div>

                  {/* KPIs */}
                  <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
                    <div className="flex justify-between">
                      <span className="text-slate-500 text-xs">Volume {feePeriod}</span>
                      <span className="text-white font-medium text-xs">{formatUsd(feePeriod === '24h' ? pool.volume24h : pool.volume7d)}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-slate-500 text-xs">TVL</span>
                      <span className="text-white font-medium text-xs">{formatUsd(pool.tvlUsd)}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-slate-500 text-xs">Fees {feePeriod}</span>
                      <span className="text-emerald-400 font-medium text-xs">{formatUsd(feePeriod === '24h' ? pool.fees24h : pool.fees7d)}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-slate-500 text-xs">Fee APR {feePeriod}</span>
                      <span className="text-slate-300 text-xs">{formatApr(feePeriod === '24h' ? pool.feeApr24h : pool.feeApr7d)}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-slate-500 text-xs">Turnover {feePeriod}</span>
                      <span className="text-slate-300 text-xs">{formatTurnover(feePeriod === '24h' ? pool.turnoverRatio24h : pool.turnoverRatio7d)}</span>
                    </div>
                    {pool.rewardsApr > 0 && (
                      <div className="flex justify-between">
                        <span className="text-slate-500 text-xs">Rewards</span>
                        <span className="text-omni-gold text-xs">{formatApr(pool.rewardsApr)}</span>
                      </div>
                    )}
                  </div>

                  {/* Footer */}
                  <div className="mt-3 pt-3 border-t border-slate-700/30 flex justify-between items-center">
                    <button
                      onClick={e => { e.stopPropagation(); setSelectedPool(pool) }}
                      className="text-xs text-slate-400 hover:text-emerald-400 flex items-center gap-1"
                    >
                      <BarChart2 className="w-3 h-3" /> Details
                    </button>
                    <a
                      href={pool.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      onClick={e => e.stopPropagation()}
                      className="text-xs text-emerald-400 hover:text-emerald-300 flex items-center gap-1"
                    >
                      Add Liquidity <ExternalLink className="w-3 h-3" />
                    </a>
                  </div>
                </div>
              ))}
            </div>

          {/* Load more */}
          {hasMore && (
            <div className="mt-6 text-center">
              <button
                onClick={handleLoadMore}
                disabled={loading}
                className="inline-flex items-center gap-2 px-6 py-2.5 bg-slate-800/60 hover:bg-slate-700/60 text-slate-300 border border-slate-700/30 hover:border-slate-600 rounded-xl transition-all text-sm font-medium disabled:opacity-40"
              >
                {loading ? 'Loading...' : `Load More (${totalCount - results.length} remaining)`}
                <ChevronDown className="w-4 h-4" />
              </button>
            </div>
          )}
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="card p-12 text-center">
          <div className="inline-block w-8 h-8 border-2 border-emerald-500/30 border-t-emerald-500 rounded-full animate-spin" />
          <p className="text-slate-400 mt-4">Searching pools...</p>
        </div>
      )}

      {/* Pool Detail Drawer */}
      <PoolDetailDrawer
        pool={selectedPool}
        onClose={() => setSelectedPool(null)}
      />
    </div>
  )
}
