import { useState, useEffect, useRef } from 'react'
import { Search, TrendingUp, TrendingDown, ChevronDown, BarChart2, SlidersHorizontal, Info } from 'lucide-react'
import { searchRates, RateResult } from '../lib/api'
import { ProtocolIcon } from './ProtocolIcon'
import { ChainIcon } from './ChainIcon'
import VaultDetailDrawer from './VaultDetailDrawer'
import AssetCategoryFilter, { type AssetCategory } from './AssetCategoryFilter'

type Action = 'supply' | 'borrow'
type Chain = 'ethereum' | 'solana' | 'bsc' | 'bitcoin' | 'tron' | 'base' | 'arbitrum' | 'polygon' | 'optimism' | 'avalanche' | 'sui' | 'hyperliquid' | 'scroll' | 'mantle' | 'linea' | 'blast' | 'fantom' | 'zksync' | 'aptos' | 'celo'
type Protocol = 'aave' | 'kamino' | 'morpho' | 'fluid' | 'sparklend' | 'justlend' | 'euler' | 'jupiter' | 'lido' | 'marinade' | 'jito' | 'rocketpool' | 'compound' | 'venus' | 'pendle' | 'ethena' | 'etherfi' | 'benqi' | 'radiant' | 'silo' | 'sky' | 'fraxeth' | 'aura' | 'convex' | 'yearn' | 'stargate' | 'gmx'
type OperationType = 'lending' | 'vault' | 'staking'

const OPERATION_TYPES: { value: OperationType; label: string; description: string }[] = [
  { value: 'lending', label: 'Lending', description: 'Traditional lending/borrowing protocols where you supply assets to earn interest or borrow against collateral (e.g. Aave, Kamino Lend, Morpho)' },
  { value: 'vault', label: 'Vault', description: 'Automated yield strategies that deposit into optimized vaults for enhanced returns (e.g. Morpho Vaults, Euler Vaults)' },
  { value: 'staking', label: 'Staking', description: 'Liquid staking protocols that let you stake native tokens while maintaining liquidity (e.g. Lido, Jito, Marinade, Rocket Pool)' },
]
const CHAINS: { value: Chain; label: string }[] = [
  { value: 'aptos', label: 'Aptos' },
  { value: 'arbitrum', label: 'Arbitrum' },
  { value: 'avalanche', label: 'Avalanche' },
  { value: 'base', label: 'Base' },
  { value: 'bitcoin', label: 'Bitcoin' },
  { value: 'blast', label: 'Blast' },
  { value: 'bsc', label: 'BSC' },
  { value: 'celo', label: 'Celo' },
  { value: 'ethereum', label: 'Ethereum' },
  { value: 'fantom', label: 'Fantom' },
  { value: 'hyperliquid', label: 'Hyperliquid' },
  { value: 'linea', label: 'Linea' },
  { value: 'mantle', label: 'Mantle' },
  { value: 'optimism', label: 'Optimism' },
  { value: 'polygon', label: 'Polygon' },
  { value: 'scroll', label: 'Scroll' },
  { value: 'solana', label: 'Solana' },
  { value: 'sui', label: 'Sui' },
  { value: 'tron', label: 'Tron' },
  { value: 'zksync', label: 'zkSync Era' },
]
const PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: 'aave', label: 'Aave v3' },
  { value: 'euler', label: 'Euler' },
  { value: 'fluid', label: 'Fluid' },
  { value: 'jito', label: 'Jito' },
  { value: 'jupiter', label: 'Jupiter' },
  { value: 'justlend', label: 'JustLend' },
  { value: 'kamino', label: 'Kamino' },
  { value: 'lido', label: 'Lido' },
  { value: 'marinade', label: 'Marinade' },
  { value: 'morpho', label: 'Morpho' },
  { value: 'rocketpool', label: 'Rocket Pool' },
  { value: 'sparklend', label: 'SparkLend' },
  { value: 'compound', label: 'Compound V3' },
  { value: 'ethena', label: 'Ethena' },
  { value: 'etherfi', label: 'EtherFi' },
  { value: 'pendle', label: 'Pendle' },
  { value: 'venus', label: 'Venus' },
  { value: 'benqi', label: 'Benqi' },
  { value: 'radiant', label: 'Radiant' },
  { value: 'silo', label: 'Silo' },
  { value: 'sky', label: 'Sky (Maker)' },
  { value: 'fraxeth', label: 'Frax ETH' },
  { value: 'aura', label: 'Aura' },
  { value: 'convex', label: 'Convex' },
  { value: 'yearn', label: 'Yearn' },
  { value: 'stargate', label: 'Stargate' },
  { value: 'gmx', label: 'GMX' },
]

export default function RateFinder() {
  const [action, setAction] = useState<Action>('supply')
  const [selectedChains, setSelectedChains] = useState<Chain[]>(CHAINS.map(c => c.value))
  const [selectedProtocols, setSelectedProtocols] = useState<Protocol[]>(PROTOCOLS.map(p => p.value))
  const [selectedOperationTypes, setSelectedOperationTypes] = useState<OperationType[]>(['lending', 'vault', 'staking'])
  const [tokenFilterMode, setTokenFilterMode] = useState<'category' | 'name'>('category')
  const [selectedAssetCategory, setSelectedAssetCategory] = useState<AssetCategory | null>(null)
  const [searchToken, setSearchToken] = useState<string>('')
  const [minLiquidity, setMinLiquidity] = useState<number>(1000000)
  const [results, setResults] = useState<RateResult[]>([])
  const [loading, setLoading] = useState(false)
  const [page, setPage] = useState(1)
  const [totalCount, setTotalCount] = useState(0)
  const [hasMore, setHasMore] = useState(false)
  const PAGE_SIZE = 20
  const [error, setError] = useState<string | null>(null)
  const [isChainDropdownOpen, setIsChainDropdownOpen] = useState(false)
  const [isProtocolDropdownOpen, setIsProtocolDropdownOpen] = useState(false)
  const [selectedVault, setSelectedVault] = useState<RateResult | null>(null)
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

  const toggleChain = (chain: Chain) => {
    setSelectedChains(prev => prev.includes(chain) ? prev.filter(c => c !== chain) : [...prev, chain])
  }

  const toggleProtocol = (protocol: Protocol) => {
    setSelectedProtocols(prev => prev.includes(protocol) ? prev.filter(p => p !== protocol) : [...prev, protocol])
  }

  const toggleOperationType = (operationType: OperationType) => {
    setSelectedOperationTypes(prev =>
      prev.includes(operationType) ? prev.filter(ot => ot !== operationType) : [...prev, operationType]
    )
  }

  const renderChainLabel = () => {
    if (selectedChains.length === CHAINS.length) return 'All Chains'
    if (selectedChains.length === 1) {
      const chain = CHAINS.find(c => c.value === selectedChains[0])
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
    if (selectedProtocols.length === PROTOCOLS.length) return 'All Protocols'
    if (selectedProtocols.length === 1) {
      const protocol = PROTOCOLS.find(p => p.value === selectedProtocols[0])
      return (
        <span className="flex items-center">
          <ProtocolIcon protocol={selectedProtocols[0]} className="w-4 h-4 mr-2" />
          {protocol?.label || 'Select protocols'}
        </span>
      )
    }
    return `${selectedProtocols.length} protocols selected`
  }

  const buildParams = (targetPage: number) => ({
    action,
    chains: selectedChains.length > 0 && selectedChains.length < CHAINS.length
      ? selectedChains.join(',')
      : undefined,
    protocols: selectedProtocols.length > 0 && selectedProtocols.length < PROTOCOLS.length
      ? selectedProtocols.join(',')
      : undefined,
    operation_types: selectedOperationTypes.length > 0 ? selectedOperationTypes.join(',') : undefined,
    asset_categories: selectedAssetCategory || undefined,
    token: searchToken.trim() || undefined,
    min_liquidity: minLiquidity,
    page: targetPage,
    page_size: PAGE_SIZE,
  })

  const handleSearch = async () => {
    setLoading(true)
    setError(null)
    setIsChainDropdownOpen(false)
    setIsProtocolDropdownOpen(false)

    try {
      const data = await searchRates(buildParams(1))
      setResults(data.results)
      setPage(1)
      setTotalCount(data.totalCount)
      setHasMore(data.page < data.totalPages)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch rates')
    } finally {
      setLoading(false)
    }
  }

  const handleLoadMore = async () => {
    const nextPage = page + 1
    setLoading(true)
    try {
      const data = await searchRates(buildParams(nextPage))
      setResults(prev => [...prev, ...data.results])
      setPage(nextPage)
      setHasMore(data.page < data.totalPages)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load more')
    } finally {
      setLoading(false)
    }
  }

  const formatNumber = (num: number) => {
    if (num >= 1_000_000) return `$${(num / 1_000_000).toFixed(1)}M`
    if (num >= 1_000) return `$${(num / 1_000).toFixed(1)}K`
    return `$${num.toFixed(0)}`
  }

  return (
    <div className="space-y-6" id="rates">
      {/* Filters Card */}
      <div className="card">
        <div className="flex items-center gap-3 mb-6">
          <div className="w-10 h-10 rounded-xl bg-omni-blue/10 border border-omni-blue/20 flex items-center justify-center">
            <SlidersHorizontal className="w-5 h-5 text-omni-blue-light" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-white">Earn Finder</h2>
            <p className="text-xs text-slate-500">Configure filters and search across DeFi</p>
          </div>
        </div>

        {/* Action Toggle */}
        <div className="mb-5">
          <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">I want to</label>
          <div className="flex gap-2">
            <button
              onClick={() => setAction('supply')}
              className={`flex-1 py-3 px-6 rounded-xl font-semibold text-sm transition-all duration-200 ${
                action === 'supply'
                  ? 'bg-omni-blue text-white shadow-lg shadow-omni-blue/20'
                  : 'bg-slate-800/60 text-slate-400 hover:bg-slate-700/60 hover:text-white border border-slate-700/30'
              }`}
            >
              <TrendingUp className="inline w-4 h-4 mr-2 -mt-0.5" />
              Supply
            </button>
            <button
              onClick={() => setAction('borrow')}
              className={`flex-1 py-3 px-6 rounded-xl font-semibold text-sm transition-all duration-200 ${
                action === 'borrow'
                  ? 'bg-omni-red text-white shadow-lg shadow-omni-red/20'
                  : 'bg-slate-800/60 text-slate-400 hover:bg-slate-700/60 hover:text-white border border-slate-700/30'
              }`}
            >
              <TrendingDown className="inline w-4 h-4 mr-2 -mt-0.5" />
              Borrow
            </button>
          </div>
        </div>

        {/* Filters grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-5">
          {/* Chain Filters */}
          <div className="relative" ref={chainDropdownRef}>
            <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Chains</label>
            <button
              onClick={() => setIsChainDropdownOpen(!isChainDropdownOpen)}
              className="input-field flex items-center justify-between"
            >
              <span className="text-sm">{renderChainLabel()}</span>
              <ChevronDown className={`w-4 h-4 text-slate-500 transition-transform ${isChainDropdownOpen ? 'rotate-180' : ''}`} />
            </button>
            {isChainDropdownOpen && (
              <div className="dropdown-menu max-h-80">
                <div className="p-2">
                  <div className="flex gap-2 mb-2 pb-2 border-b border-slate-700/50">
                    <button onClick={() => setSelectedChains(CHAINS.map(c => c.value))} className="flex-1 px-3 py-1.5 text-xs font-medium bg-omni-blue/10 text-omni-blue-light rounded-lg hover:bg-omni-blue/20 transition-colors">Select All</button>
                    <button onClick={() => setSelectedChains([])} className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-700/50 text-slate-400 rounded-lg hover:bg-slate-700 transition-colors">Clear</button>
                  </div>
                  {CHAINS.map((chain) => (
                    <label key={chain.value} className="flex items-center px-3 py-2 hover:bg-slate-700/50 rounded-lg cursor-pointer transition-colors">
                      <input type="checkbox" checked={selectedChains.includes(chain.value)} onChange={() => toggleChain(chain.value)} className="mr-3 w-3.5 h-3.5 text-omni-blue bg-slate-700 border-slate-600 rounded focus:ring-omni-blue" />
                      <ChainIcon chain={chain.value} className="w-4 h-4 mr-2" />
                      <span className="text-sm text-slate-300">{chain.label}</span>
                    </label>
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* Minimum Liquidity */}
          <div>
            <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Min Liquidity</label>
            <select
              value={minLiquidity}
              onChange={(e) => setMinLiquidity(Number(e.target.value))}
              className="input-field text-sm"
            >
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
              onClick={() => setIsProtocolDropdownOpen(!isProtocolDropdownOpen)}
              className="input-field flex items-center justify-between"
            >
              <span className="text-sm">{renderProtocolLabel()}</span>
              <ChevronDown className={`w-4 h-4 text-slate-500 transition-transform ${isProtocolDropdownOpen ? 'rotate-180' : ''}`} />
            </button>
            {isProtocolDropdownOpen && (
              <div className="dropdown-menu">
                <div className="p-2">
                  <div className="flex gap-2 mb-2 pb-2 border-b border-slate-700/50">
                    <button onClick={() => setSelectedProtocols(PROTOCOLS.map(p => p.value))} className="flex-1 px-3 py-1.5 text-xs font-medium bg-omni-blue/10 text-omni-blue-light rounded-lg hover:bg-omni-blue/20 transition-colors">Select All</button>
                    <button onClick={() => setSelectedProtocols([])} className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-700/50 text-slate-400 rounded-lg hover:bg-slate-700 transition-colors">Clear</button>
                  </div>
                  {PROTOCOLS.map((protocol) => (
                    <label key={protocol.value} className="flex items-center px-3 py-2 hover:bg-slate-700/50 rounded-lg cursor-pointer transition-colors">
                      <input type="checkbox" checked={selectedProtocols.includes(protocol.value)} onChange={() => toggleProtocol(protocol.value)} className="mr-3 w-3.5 h-3.5 text-omni-blue bg-slate-700 border-slate-600 rounded focus:ring-omni-blue" />
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
            { label: 'Ethereum Only', chains: ['ethereum'] as Chain[] },
            { label: 'Solana Only', chains: ['solana'] as Chain[] },
            { label: 'EVM Chains', chains: ['ethereum', 'arbitrum', 'base', 'polygon', 'optimism'] as Chain[] },
            { label: 'All Chains', chains: CHAINS.map(c => c.value) },
          ].map(q => (
            <button key={q.label} onClick={() => { setSelectedChains(q.chains); setSelectedProtocols(PROTOCOLS.map(p => p.value)) }}
              className="px-3 py-1 text-xs font-medium bg-slate-800/60 text-slate-400 border border-slate-700/30 rounded-lg hover:text-white hover:border-slate-600 transition-colors"
            >{q.label}</button>
          ))}
        </div>

        {/* Operation Type + Category */}
        <div className="space-y-4 mb-5">
          <div>
            <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Operation Type</label>
            <div className="flex flex-wrap gap-2">
              {OPERATION_TYPES.map((opType) => (
                <div key={opType.value} className="relative group/tooltip">
                  <button
                    onClick={() => toggleOperationType(opType.value)}
                    className={`flex items-center gap-1.5 px-3.5 py-2 rounded-lg text-sm font-medium transition-all duration-200 ${
                      selectedOperationTypes.includes(opType.value)
                        ? 'bg-omni-blue/15 text-omni-blue-light border border-omni-blue/30'
                        : 'bg-slate-800/60 text-slate-500 border border-slate-700/30 hover:text-slate-300 hover:border-slate-600'
                    }`}
                  >
                    {opType.label}
                    <Info className="w-3 h-3 opacity-40 group-hover/tooltip:opacity-70 transition-opacity" />
                  </button>
                  <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-xs text-slate-300 w-56 opacity-0 invisible group-hover/tooltip:opacity-100 group-hover/tooltip:visible transition-all duration-200 z-50 pointer-events-none shadow-xl">
                    {opType.description}
                    <div className="absolute top-full left-1/2 -translate-x-1/2 -mt-px w-2 h-2 bg-slate-900 border-r border-b border-slate-700 rotate-45" />
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Token Filter Mode Toggle */}
          <div>
            <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Filter Asset By</label>
            <div className="flex rounded-lg overflow-hidden border border-slate-700/30 w-fit">
              <button
                onClick={() => { setTokenFilterMode('category'); setSearchToken('') }}
                className={`px-4 py-1.5 text-xs font-medium transition-all duration-200 ${
                  tokenFilterMode === 'category'
                    ? 'bg-omni-blue/15 text-omni-blue-light'
                    : 'bg-slate-800/60 text-slate-500 hover:text-slate-300'
                }`}
              >Category</button>
              <button
                onClick={() => { setTokenFilterMode('name'); setSelectedAssetCategory(null) }}
                className={`px-4 py-1.5 text-xs font-medium transition-all duration-200 ${
                  tokenFilterMode === 'name'
                    ? 'bg-omni-blue/15 text-omni-blue-light'
                    : 'bg-slate-800/60 text-slate-500 hover:text-slate-300'
                }`}
              >Name</button>
            </div>
          </div>

          {tokenFilterMode === 'category' ? (
            <AssetCategoryFilter selected={selectedAssetCategory} onSelect={setSelectedAssetCategory} />
          ) : (
            <div>
              <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Token Name</label>
              <input
                type="text"
                value={searchToken}
                onChange={(e) => setSearchToken(e.target.value)}
                placeholder="e.g. USDC, ETH, BTC..."
                className="input-field text-sm"
              />
            </div>
          )}
        </div>

        {/* Search Button */}
        <button
          onClick={handleSearch}
          disabled={loading || selectedChains.length === 0 || selectedProtocols.length === 0 || selectedOperationTypes.length === 0}
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
              Search
            </span>
          )}
        </button>

        {error && (
          <div className="mt-4 p-3 bg-red-500/10 border border-red-500/20 rounded-xl text-red-400 text-sm">
            {error}
          </div>
        )}
      </div>

      {/* Results */}
      {results.length > 0 && (
        <div className="card">
          <div className="flex items-center justify-between mb-6">
            <h3 className="text-lg font-bold text-white flex items-center gap-2">
              <BarChart2 className="w-5 h-5 text-omni-blue-light" />
              Best {action === 'supply' ? 'Supply' : 'Borrow'} Results
            </h3>
            <span className="text-xs text-slate-500 bg-slate-800/60 px-3 py-1 rounded-full">
              {results.length} of {totalCount}
            </span>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {results.map((result, index) => {
              const category = Array.isArray(result.assetCategory)
                ? (result.assetCategory.length > 0 ? result.assetCategory[0] : null)
                : result.assetCategory
              return (
              <div
                key={`${result.protocol}-${result.chain}-${index}`}
                className="group rounded-xl bg-slate-800/40 border border-slate-700/40 p-4 hover:border-omni-blue/30 hover:bg-slate-800/60 transition-all duration-300"
              >
                {/* Header */}
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-2">
                    <ProtocolIcon protocol={result.protocol} className="w-5 h-5" />
                    <span className="text-white font-semibold text-sm capitalize">{result.protocol}</span>
                    <span className={`px-2 py-0.5 rounded-full text-[10px] font-semibold ${
                      action === 'supply'
                        ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                        : 'bg-red-500/10 text-red-400 border border-red-500/20'
                    }`}>
                      {action === 'supply' ? 'Supply' : 'Borrow'}
                    </span>
                    <span className={`px-2 py-0.5 rounded-full text-[10px] font-semibold ${
                      result.operationType === 'vault'
                        ? 'bg-orange-500/10 text-orange-400 border border-orange-500/20'
                        : result.operationType === 'staking'
                        ? 'bg-blue-500/10 text-blue-400 border border-blue-500/20'
                        : 'bg-slate-500/10 text-slate-400 border border-slate-500/20'
                    }`}>
                      {result.operationType === 'vault' ? 'Vault' : result.operationType === 'staking' ? 'Staking' : 'Lending'}
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <ChainIcon chain={result.chain} className="w-3.5 h-3.5" />
                    <span className="text-[10px] text-slate-500 capitalize">{result.chain}</span>
                  </div>
                </div>

                {/* Asset + Category + Vault Name */}
                <div className="flex items-center justify-between mb-3 pb-3 border-b border-slate-700/30">
                  <div className="flex items-center gap-2">
                  <span className="text-xl font-mono font-bold text-white">{result.asset}</span>
                  <span className={`px-2 py-0.5 rounded-full text-[10px] font-medium border ${
                    category === 'usd-correlated' ? 'bg-blue-500/10 text-blue-400 border-blue-500/20'
                    : category === 'stablecoin' ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
                    : category === 'btc-correlated' ? 'bg-orange-500/10 text-orange-400 border-orange-500/20'
                    : category === 'eth-correlated' ? 'bg-purple-500/10 text-purple-400 border-purple-500/20'
                    : category === 'sol-correlated' ? 'bg-violet-500/10 text-violet-400 border-violet-500/20'
                    : 'bg-slate-500/10 text-slate-400 border-slate-500/20'
                  }`}>
                    {category === 'usd-correlated' ? 'USD'
                      : category === 'stablecoin' ? 'Stable'
                      : category === 'btc-correlated' ? 'BTC'
                      : category === 'eth-correlated' ? 'ETH'
                      : category === 'sol-correlated' ? 'SOL'
                      : 'Other'}
                  </span>
                  </div>
                  {result.vaultName && (
                    <span className="text-[11px] text-slate-500 truncate ml-2 max-w-[50%] text-right">{result.vaultName}</span>
                  )}
                </div>

                {/* Stats */}
                <div className="rounded-xl bg-slate-900/40 p-3 mb-3 space-y-2">
                  <div className="flex justify-between items-center pb-2 border-b border-slate-700/30">
                    <span className="text-[11px] text-slate-500">Net {action === 'supply' ? 'APY' : 'APR'}</span>
                    <span className="font-mono font-bold text-lg text-emerald-400">{result.netApy.toFixed(2)}%</span>
                  </div>
                  <div className="flex justify-between items-center">
                    <span className="text-[11px] text-slate-500">Base</span>
                    <span className="text-xs text-white font-mono">{result.apy.toFixed(2)}%</span>
                  </div>
                  <div className="flex justify-between items-center pb-2 border-b border-slate-700/30">
                    <span className="text-[11px] text-slate-500">Rewards</span>
                    <span className={`text-xs font-mono font-medium ${result.rewards > 0 ? 'text-emerald-400' : 'text-slate-600'}`}>
                      {result.rewards > 0 ? '+' : ''}{result.rewards.toFixed(2)}%
                    </span>
                  </div>

                  {result.apyMetrics && result.apyMetrics.daysWithData > 0 && (
                    <div className="pt-1 pb-2 border-b border-slate-700/30">
                      <div className="text-[11px] text-slate-500 mb-1.5">Historical APY</div>
                      <div className="grid grid-cols-2 gap-1.5">
                        {result.apyMetrics.apy7d !== undefined && result.apyMetrics.daysWithData >= 7 && (
                          <div className="flex justify-between">
                            <span className="text-[10px] text-slate-600">7d</span>
                            <span className="text-[10px] font-mono text-slate-400">{result.apyMetrics.apy7d.toFixed(2)}%</span>
                          </div>
                        )}
                        {result.apyMetrics.apy30d !== undefined && result.apyMetrics.daysWithData >= 30 && (
                          <div className="flex justify-between">
                            <span className="text-[10px] text-slate-600">30d</span>
                            <span className="text-[10px] font-mono text-slate-400">{result.apyMetrics.apy30d.toFixed(2)}%</span>
                          </div>
                        )}
                        {result.apyMetrics.apy60d !== undefined && result.apyMetrics.daysWithData >= 60 && (
                          <div className="flex justify-between">
                            <span className="text-[10px] text-slate-600">60d</span>
                            <span className="text-[10px] font-mono text-slate-400">{result.apyMetrics.apy60d.toFixed(2)}%</span>
                          </div>
                        )}
                        {result.apyMetrics.apy90d !== undefined && result.apyMetrics.daysWithData >= 90 && (
                          <div className="flex justify-between">
                            <span className="text-[10px] text-slate-600">90d</span>
                            <span className="text-[10px] font-mono text-slate-400">{result.apyMetrics.apy90d.toFixed(2)}%</span>
                          </div>
                        )}
                      </div>
                      <div className="flex justify-between mt-1.5">
                        <span className="text-[10px] text-slate-600">Volatility</span>
                        <span className="text-[10px] font-mono text-slate-500">{result.apyMetrics.volatility.toFixed(2)}%</span>
                      </div>
                    </div>
                  )}

                  <div className="flex justify-between items-center pt-1">
                    <span className="text-[11px] text-slate-500">Reserve Liquidity</span>
                    <span className="text-xs text-white font-medium">{formatNumber(result.totalLiquidity || result.liquidity)}</span>
                  </div>
                  <div className="flex justify-between items-center">
                    <span className="text-[11px] text-slate-500">Available</span>
                    <span className="text-xs text-slate-400">{formatNumber(result.liquidity)}</span>
                  </div>
                  <div className="flex justify-between items-center">
                    <span className="text-[11px] text-slate-500">Utilization</span>
                    <span className="text-xs text-slate-400">{result.utilizationRate.toFixed(2)}%</span>
                  </div>
                </div>

                {/* Actions */}
                <div className="flex gap-2">
                  <button
                    onClick={() => setSelectedVault(result)}
                    className="flex items-center justify-center p-2.5 bg-slate-800/60 hover:bg-slate-700/60 text-slate-400 hover:text-white rounded-lg transition-all border border-slate-700/30 hover:border-slate-600"
                    title="View APY history & details"
                  >
                    <BarChart2 className="w-4 h-4" />
                  </button>
                  <a
                    href={result.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="flex items-center justify-center flex-1 gap-2 px-4 py-2.5 bg-omni-blue hover:bg-omni-blue-dark text-white rounded-lg transition-all text-sm font-medium"
                  >
                    {action === 'supply' ? 'Supply' : 'Borrow'}
                    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                      <path d="M6.02761 3.42871C5.07671 3.47152 4.14468 3.57774 3.23689 3.68288C2.27739 3.79401 1.51075 4.56009 1.40314 5.51999C1.2722 6.68798 1.14258 7.89621 1.14258 9.13316C1.14258 10.3701 1.2722 11.5783 1.40314 12.7463C1.51075 13.7062 2.27737 14.4723 3.23687 14.5834C4.41124 14.7195 5.6262 14.8573 6.87007 14.8573C8.11395 14.8573 9.3289 14.7195 10.5033 14.5834C11.4628 14.4723 12.2294 13.7062 12.337 12.7463C12.4322 11.8975 12.5266 11.0274 12.5711 10.1405" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
                      <path d="M9.71484 1.71436H14.2863V6.28578" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                      <path d="M7.42773 8.5715L14.2849 1.71436" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                    </svg>
                  </a>
                </div>
              </div>
              )
            })}
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

      <VaultDetailDrawer
        vault={selectedVault}
        action={action}
        onClose={() => setSelectedVault(null)}
      />
    </div>
  )
}
