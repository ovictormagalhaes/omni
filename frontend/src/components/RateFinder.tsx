import { useState, useEffect } from 'react'
import { Search, TrendingUp, TrendingDown, ChevronDown, BarChart2 } from 'lucide-react'
import { searchRates, fetchAvailableAssets, AssetInfo, RateResult } from '../lib/api'
import { ProtocolIcon } from './ProtocolIcon'
import { ChainIcon } from './ChainIcon'
import VaultDetailDrawer from './VaultDetailDrawer'

type Action = 'supply' | 'borrow'
type Chain = 'ethereum' | 'solana' | 'bsc' | 'bitcoin' | 'tron' | 'base' | 'arbitrum' | 'polygon' | 'optimism' | 'avalanche' | 'sui' | 'hyperliquid' | 'scroll' | 'mantle' | 'linea' | 'blast' | 'fantom' | 'zksync' | 'aptos' | 'celo'
type Protocol = 'aave' | 'kamino' | 'morpho' | 'fluid' | 'sparklend' | 'justlend' | 'euler' | 'jupiter' | 'lido' | 'marinade' | 'jito' | 'rocketpool'
type OperationType = 'lending' | 'vault' | 'staking'
type AssetCategory = 'usd-correlated' | 'stablecoin' | 'btc-correlated' | 'eth-correlated' | 'sol-correlated' | 'other'

const OPERATION_TYPES: { value: OperationType; label: string }[] = [
  { value: 'lending', label: 'Lending' },
  { value: 'vault', label: 'Vault' },
  { value: 'staking', label: 'Staking' },
]
const ASSET_CATEGORIES: { value: AssetCategory; label: string }[] = [
  { value: 'usd-correlated', label: 'USD Correlated' },
  { value: 'stablecoin', label: 'Stablecoin' },
  { value: 'btc-correlated', label: 'BTC Correlated' },
  { value: 'eth-correlated', label: 'ETH Correlated' },
  { value: 'sol-correlated', label: 'SOL Correlated' },
  { value: 'other', label: 'Other' },
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
]

export default function RateFinder() {
  const [action, setAction] = useState<Action>('supply')
  const [selectedAssets, setSelectedAssets] = useState<string[]>([])
  const [availableAssets, setAvailableAssets] = useState<AssetInfo[]>([])
  const [assetsLoading, setAssetsLoading] = useState(true)
  const [isAssetDropdownOpen, setIsAssetDropdownOpen] = useState(false)
  const [selectedChains, setSelectedChains] = useState<Chain[]>(CHAINS.map(c => c.value))
  const [selectedProtocols, setSelectedProtocols] = useState<Protocol[]>(PROTOCOLS.map(p => p.value))
  const [selectedOperationTypes, setSelectedOperationTypes] = useState<OperationType[]>(['lending', 'vault', 'staking'])
  const [selectedAssetCategory, setSelectedAssetCategory] = useState<AssetCategory | null>(null)
  const [searchToken, setSearchToken] = useState<string>('')
  const [minLiquidity, setMinLiquidity] = useState<number>(1000000)
  const [results, setResults] = useState<RateResult[]>([])
  const [visibleCount, setVisibleCount] = useState<number>(9) // Mostrar 9 resultados inicialmente
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [isChainDropdownOpen, setIsChainDropdownOpen] = useState(false)
  const [isProtocolDropdownOpen, setIsProtocolDropdownOpen] = useState(false)
  const [selectedVault, setSelectedVault] = useState<RateResult | null>(null)

  const toggleAsset = (symbol: string) => {
    if (selectedAssets.includes(symbol)) {
      setSelectedAssets(selectedAssets.filter(a => a !== symbol))
    } else {
      setSelectedAssets([...selectedAssets, symbol])
    }
  }

  const selectAllAssets = () => {
    setSelectedAssets(availableAssets.map(a => a.symbol))
  }

  const deselectAllAssets = () => {
    setSelectedAssets([])
  }

  const renderAssetLabel = () => {
    if (assetsLoading) return <span>Loading assets...</span>
    if (selectedAssets.length === 0 || selectedAssets.length === availableAssets.length) {
      return <span>All Assets</span>
    }
    if (selectedAssets.length === 1) {
      return <span>{selectedAssets[0]}</span>
    }
    return <span>{selectedAssets.length} assets selected</span>
  }

  const toggleChain = (chain: Chain) => {
    if (selectedChains.includes(chain)) {
      setSelectedChains(selectedChains.filter(c => c !== chain))
    } else {
      setSelectedChains([...selectedChains, chain])
    }
  }

  // Load available assets from backend on mount
  useEffect(() => {
    setAssetsLoading(true)
    fetchAvailableAssets().then((assets) => {
      if (assets.length > 0) {
        setAvailableAssets(assets)
        setSelectedAssets(assets.map(a => a.symbol)) // start with all selected
      }
      setAssetsLoading(false)
    })
  }, [])

  const toggleProtocol = (protocol: Protocol) => {
    if (selectedProtocols.includes(protocol)) {
      setSelectedProtocols(selectedProtocols.filter(p => p !== protocol))
    } else {
      setSelectedProtocols([...selectedProtocols, protocol])
    }
  }

  const toggleOperationType = (operationType: OperationType) => {
    if (selectedOperationTypes.includes(operationType)) {
      setSelectedOperationTypes(selectedOperationTypes.filter(ot => ot !== operationType))
    } else {
      setSelectedOperationTypes([...selectedOperationTypes, operationType])
    }
  }

  const selectAllChains = () => {
    setSelectedChains(CHAINS.map(c => c.value))
  }

  const deselectAllChains = () => {
    setSelectedChains([])
  }

  const selectAllProtocols = () => {
    setSelectedProtocols(PROTOCOLS.map(p => p.value))
  }

  const deselectAllProtocols = () => {
    setSelectedProtocols([])
  }

  const renderChainLabel = () => {
    if (selectedChains.length === CHAINS.length) return <span>All Chains</span>
    if (selectedChains.length === 1) {
      const chain = CHAINS.find(c => c.value === selectedChains[0])
      return (
        <span className="flex items-center">
          <ChainIcon chain={selectedChains[0]} className="w-5 h-5 mr-2" />
          {chain?.label || 'Select chains'}
        </span>
      )
    }
    return <span>{selectedChains.length} chains selected</span>
  }

  const renderProtocolLabel = () => {
    if (selectedProtocols.length === PROTOCOLS.length) return <span>All Protocols</span>
    if (selectedProtocols.length === 1) {
      const protocol = PROTOCOLS.find(p => p.value === selectedProtocols[0])
      return (
        <span className="flex items-center">
          <ProtocolIcon protocol={selectedProtocols[0]} className="w-5 h-5 mr-2" />
          {protocol?.label || 'Select protocols'}
        </span>
      )
    }
    return <span>{selectedProtocols.length} protocols selected</span>
  }

  const loadMore = () => {
    setVisibleCount(prevCount => prevCount + 9)
  }

  const handleSearch = async () => {
    setLoading(true)
    setError(null)
    setVisibleCount(9) // Reset para 9 resultados
    setIsChainDropdownOpen(false)
    setIsProtocolDropdownOpen(false)
    setIsAssetDropdownOpen(false)

    try {
      const data = await searchRates({
        action,
        assets: selectedAssets.length > 0 && selectedAssets.length < availableAssets.length
          ? selectedAssets.join(',')
          : undefined,
        // Only send chains if not all are selected
        chains: selectedChains.length > 0 && selectedChains.length < CHAINS.length 
          ? selectedChains.join(',') 
          : undefined,
        // Only send protocols if not all are selected
        protocols: selectedProtocols.length > 0 && selectedProtocols.length < PROTOCOLS.length
          ? selectedProtocols.join(',')
          : undefined,
        operation_types: selectedOperationTypes.length > 0 ? selectedOperationTypes.join(',') : undefined,
        asset_categories: selectedAssetCategory || undefined,
        min_liquidity: minLiquidity,
      })

      setResults(data.results)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch rates')
    } finally {
      setLoading(false)
    }
  }

  const formatNumber = (num: number) => {
    if (num >= 1_000_000) {
      return `$${(num / 1_000_000).toFixed(1)}M`
    }
    if (num >= 1_000) {
      return `$${(num / 1_000).toFixed(1)}K`
    }
    return `$${num.toFixed(0)}`
  }

  return (
    <div className="space-y-8" id="rates">
      {/* Filters Card */}
      <div className="card">
        <h2 className="text-2xl font-bold mb-6 text-white">Find Best Rates</h2>

        {/* Action Toggle */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-omni-silver mb-2">
            I want to:
          </label>
          <div className="flex gap-4">
            <button
              onClick={() => setAction('supply')}
              className={`flex-1 py-3 px-6 rounded-lg font-semibold transition-all ${
                action === 'supply'
                  ? 'bg-omni-blue text-white'
                  : 'bg-slate-700 text-omni-silver hover:bg-slate-600'
              }`}
            >
              <TrendingUp className="inline w-5 h-5 mr-2" />
              Supply
            </button>
            <button
              onClick={() => setAction('borrow')}
              className={`flex-1 py-3 px-6 rounded-lg font-semibold transition-all ${
                action === 'borrow'
                  ? 'bg-omni-red text-white'
                  : 'bg-slate-700 text-omni-silver hover:bg-slate-600'
              }`}
            >
              <TrendingDown className="inline w-5 h-5 mr-2" />
              Borrow
            </button>
          </div>
        </div>

        {/* Asset Selector */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-omni-silver mb-2">
            Asset
          </label>
          <div className="relative">
            <button
              onClick={() => setIsAssetDropdownOpen(!isAssetDropdownOpen)}
              disabled={assetsLoading}
              className="w-full bg-slate-700 border border-slate-600 rounded-lg px-4 py-3 text-white focus:outline-none focus:border-omni-blue transition-colors flex items-center justify-between disabled:opacity-60"
            >
              {renderAssetLabel()}
              <ChevronDown className={`w-5 h-5 transition-transform ${isAssetDropdownOpen ? 'rotate-180' : ''}`} />
            </button>

            {isAssetDropdownOpen && (
              <div className="absolute z-10 w-full mt-2 bg-slate-700 border border-slate-600 rounded-lg shadow-lg max-h-64 overflow-y-auto">
                <div className="p-2">
                  {/* Select/Deselect All Buttons */}
                  <div className="flex gap-2 mb-2 pb-2 border-b border-slate-600">
                    <button
                      onClick={selectAllAssets}
                      className="flex-1 px-3 py-1.5 text-xs font-medium bg-omni-blue text-white rounded hover:bg-blue-600 transition-colors"
                    >
                      ✓ Select All
                    </button>
                    <button
                      onClick={deselectAllAssets}
                      className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-600 text-white rounded hover:bg-slate-500 transition-colors"
                    >
                      ✗ Deselect All
                    </button>
                  </div>
                  {availableAssets.map((a) => (
                    <label
                      key={a.symbol}
                      className="flex items-center px-3 py-2 hover:bg-slate-600 rounded cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={selectedAssets.includes(a.symbol)}
                        onChange={() => toggleAsset(a.symbol)}
                        className="mr-3 w-4 h-4 text-omni-blue bg-slate-600 border-slate-500 rounded focus:ring-omni-blue"
                      />
                      <span className="text-white font-mono text-sm">{a.symbol}</span>
                    </label>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Chain Filters */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-omni-silver mb-2">
            Chains
          </label>
          <div className="relative">
            <button
              onClick={() => setIsChainDropdownOpen(!isChainDropdownOpen)}
              className="w-full bg-slate-700 border border-slate-600 rounded-lg px-4 py-3 text-white focus:outline-none focus:border-omni-blue transition-colors flex items-center justify-between"
            >
              {renderChainLabel()}
              <ChevronDown className={`w-5 h-5 transition-transform ${isChainDropdownOpen ? 'rotate-180' : ''}`} />
            </button>
            
            {isChainDropdownOpen && (
              <div className="absolute z-10 w-full mt-2 bg-slate-700 border border-slate-600 rounded-lg shadow-lg max-h-96 overflow-y-auto">
                <div className="p-2">
                  {/* Select/Deselect All Buttons */}
                  <div className="flex gap-2 mb-2 pb-2 border-b border-slate-600">
                    <button
                      onClick={selectAllChains}
                      className="flex-1 px-3 py-1.5 text-xs font-medium bg-omni-blue text-white rounded hover:bg-blue-600 transition-colors"
                    >
                      ✓ Select All
                    </button>
                    <button
                      onClick={deselectAllChains}
                      className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-600 text-white rounded hover:bg-slate-500 transition-colors"
                    >
                      ✗ Deselect All
                    </button>
                  </div>
                  {CHAINS.map((chain) => (
                    <label
                      key={chain.value}
                      className="flex items-center px-3 py-2 hover:bg-slate-600 rounded cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={selectedChains.includes(chain.value)}
                        onChange={() => toggleChain(chain.value)}
                        className="mr-3 w-4 h-4 text-omni-blue bg-slate-600 border-slate-500 rounded focus:ring-omni-blue"
                      />
                      <ChainIcon chain={chain.value} className="w-4 h-4 mr-2" />
                      <span className="text-white">{chain.label}</span>
                    </label>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Minimum Liquidity Filter */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-omni-silver mb-2">
            Minimum Liquidity (USD)
          </label>
          <select
            value={minLiquidity}
            onChange={(e) => setMinLiquidity(Number(e.target.value))}
            className="w-full bg-slate-700 border border-slate-600 rounded-lg px-4 py-3 text-white focus:outline-none focus:border-omni-blue transition-colors"
          >
            <option value={10000}>$10K+</option>
            <option value={100000}>$100K+ (Recommended)</option>
            <option value={1000000}>$1M+</option>
            <option value={10000000}>$10M+</option>
          </select>
        </div>

        {/* Protocol Filters */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-omni-silver mb-2">
            Protocols
          </label>
          <div className="relative">
            <button
              onClick={() => setIsProtocolDropdownOpen(!isProtocolDropdownOpen)}
              className="w-full bg-slate-700 border border-slate-600 rounded-lg px-4 py-3 text-white focus:outline-none focus:border-omni-blue transition-colors flex items-center justify-between"
            >
              {renderProtocolLabel()}
              <ChevronDown className={`w-5 h-5 transition-transform ${isProtocolDropdownOpen ? 'rotate-180' : ''}`} />
            </button>
            
            {isProtocolDropdownOpen && (
              <div className="absolute z-10 w-full mt-2 bg-slate-700 border border-slate-600 rounded-lg shadow-lg max-h-64 overflow-y-auto">
                <div className="p-2">
                  {/* Select/Deselect All Buttons */}
                  <div className="flex gap-2 mb-2 pb-2 border-b border-slate-600">
                    <button
                      onClick={selectAllProtocols}
                      className="flex-1 px-3 py-1.5 text-xs font-medium bg-omni-blue text-white rounded hover:bg-blue-600 transition-colors"
                    >
                      ✓ Select All
                    </button>
                    <button
                      onClick={deselectAllProtocols}
                      className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-600 text-white rounded hover:bg-slate-500 transition-colors"
                    >
                      ✗ Deselect All
                    </button>
                  </div>
                  {PROTOCOLS.map((protocol) => (
                    <label
                      key={protocol.value}
                      className="flex items-center px-3 py-2 hover:bg-slate-600 rounded cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={selectedProtocols.includes(protocol.value)}
                        onChange={() => toggleProtocol(protocol.value)}
                        className="mr-3 w-4 h-4 text-omni-blue bg-slate-600 border-slate-500 rounded focus:ring-omni-blue"
                      />
                      <ProtocolIcon protocol={protocol.value} className="w-4 h-4 mr-2" />
                      <span className="text-white">{protocol.label}</span>
                    </label>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Operation Type Filters */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-omni-silver mb-2">
            Operation Types
          </label>
          <div className="flex flex-wrap gap-3">
            {OPERATION_TYPES.map((opType) => (
              <button
                key={opType.value}
                onClick={() => toggleOperationType(opType.value)}
                className={`px-4 py-2 rounded-lg font-medium transition-all ${
                  selectedOperationTypes.includes(opType.value)
                    ? 'bg-omni-blue text-white'
                    : 'bg-slate-700 text-omni-silver hover:bg-slate-600'
                }`}
              >
                {selectedOperationTypes.includes(opType.value) && '✓ '}
                {opType.label}
              </button>
            ))}
          </div>
        </div>

        {/* Asset Category Filters */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-omni-silver mb-2">
            Asset Category
          </label>
          <div className="flex flex-wrap gap-3">
            {ASSET_CATEGORIES.map((category) => (
              <button
                key={category.value}
                onClick={() => setSelectedAssetCategory(
                  selectedAssetCategory === category.value ? null : category.value
                )}
                className={`px-4 py-2 rounded-lg font-medium transition-all ${
                  selectedAssetCategory === category.value
                    ? 'bg-omni-gold text-slate-900'
                    : 'bg-slate-700 text-omni-silver hover:bg-slate-600'
                }`}
              >
                {selectedAssetCategory === category.value && '✓ '}
                {category.label}
              </button>
            ))}
          </div>
        </div>

        {/* Token Search */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-omni-silver mb-2">
            Search by Token Symbol
          </label>
          <input
            type="text"
            value={searchToken}
            onChange={(e) => setSearchToken(e.target.value)}
            placeholder="e.g., USDC, ETH, BTC..."
            className="w-full bg-slate-700 border border-slate-600 rounded-lg px-4 py-3 text-white placeholder-omni-silver placeholder-text-xs focus:outline-none focus:ring-2 focus:ring-omni-blue"
          />
        </div>

        {/* Search Button */}
        <button
          onClick={handleSearch}
          disabled={loading || selectedChains.length === 0 || selectedProtocols.length === 0 || selectedOperationTypes.length === 0}
          className="btn-primary w-full disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Search className="inline w-5 h-5 mr-2" />
          {loading ? 'Searching...' : 'Search Rates'}
        </button>

        {error && (
          <div className="mt-4 p-4 bg-red-900/20 border border-red-500 rounded-lg text-red-400">
            {error}
          </div>
        )}
      </div>

      {/* Results Table */}
      {results.length > 0 && (() => {
        // Filter results by search token
        const filteredResults = searchToken
          ? results.filter(r => r.asset.toLowerCase().includes(searchToken.toLowerCase()))
          : results;

        return (
        <div className="card">
          <div className="flex items-center justify-between mb-6">
            <h3 className="text-xl font-bold text-white">
              📊 Best {action === 'supply' ? 'Supply' : 'Borrow'} Rates
            </h3>
            <span className="text-sm text-omni-silver">
              Showing {Math.min(visibleCount, filteredResults.length)} of {filteredResults.length} result{filteredResults.length !== 1 ? 's' : ''}
            </span>
          </div>

          {/* Cards Grid */}
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {filteredResults.slice(0, visibleCount).map((result, index) => {
              // Handle both array and single category, with fallback for unknown tokens (empty array)
              const category = Array.isArray(result.assetCategory) 
                ? (result.assetCategory.length > 0 ? result.assetCategory[0] : null)
                : result.assetCategory;
              return (
              <div
                key={`${result.protocol}-${result.chain}-${index}`}
                className="rounded-lg bg-slate-800/50 border border-slate-700 p-4 hover:border-omni-blue transition-all"
              >
                {/* Header */}
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-2">
                    <ProtocolIcon protocol={result.protocol} className="w-5 h-5" />
                    <span className="text-white font-semibold text-lg capitalize">{result.protocol}</span>
                    <span className={`px-2 py-0.5 rounded text-xs font-semibold ${
                      result.operationType === 'vault' 
                        ? 'bg-orange-500/20 text-orange-400' 
                        : 'bg-green-500/20 text-green-400'
                    }`}>
                      {result.operationType === 'vault' ? 'Vault' : 'Lending'}
                    </span>
                  </div>
                  <div className="flex items-center gap-1">
                    <ChainIcon chain={result.chain} className="w-4 h-4" />
                    <span className="text-xs text-omni-silver capitalize">{result.chain}</span>
                  </div>
                </div>

                {/* Vault Name */}
                <div className="mb-2">
                  <span className="text-xs text-omni-silver">
                    {result.vaultName ? `🏦 ${result.vaultName}` : '\u00A0'}
                  </span>
                </div>

                {/* Asset Info */}
                <div className="flex items-center gap-2 mb-4 pb-3 border-b border-slate-700">
                  <span className="text-2xl font-mono font-bold text-white">{result.asset}</span>
                  <span 
                    className={`px-2 py-1 rounded text-xs font-semibold ${
                      category === 'usd-correlated' 
                        ? 'bg-blue-500/20 text-blue-400 border border-blue-500/30' 
                        : category === 'stablecoin'
                        ? 'bg-green-500/20 text-green-400 border border-green-500/30'
                        : category === 'btc-correlated'
                        ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30'
                        : category === 'eth-correlated'
                        ? 'bg-purple-500/20 text-purple-400 border border-purple-500/30'
                        : category === 'sol-correlated'
                        ? 'bg-violet-500/20 text-violet-400 border border-violet-500/30'
                        : 'bg-slate-500/20 text-slate-400 border border-slate-500/30'
                    }`}
                    title={!category ? 'Token not yet categorized' : undefined}
                  >
                    {category === 'usd-correlated' ? '💵 USD' 
                      : category === 'stablecoin' ? '💰 Stable'
                      : category === 'btc-correlated' ? '₿ BTC'
                      : category === 'eth-correlated' ? '◈ ETH'
                      : category === 'sol-correlated' ? '☀️ SOL'
                      : '🔷 Other'}
                  </span>
                </div>

                {/* Stats */}
                <div className="rounded-lg bg-slate-900/50 p-3 mb-4">
                  <div className="flex flex-col gap-2.5">
                    {/* Net APY */}
                    <div className="flex justify-between items-center pb-2.5 border-b border-slate-700">
                      <span className="text-xs text-omni-silver font-medium">
                        🎯 Net {action === 'supply' ? 'APY' : 'APR'}
                      </span>
                      <span className="font-mono font-bold text-xl text-emerald-400">
                        {result.netApy.toFixed(2)}%
                      </span>
                    </div>

                    {/* Base APY */}
                    <div className="flex justify-between items-center">
                      <span className="text-xs text-omni-silver font-medium">
                        📊 Base {action === 'supply' ? 'APY' : 'APR'}
                      </span>
                      <span className="text-sm text-white font-mono">
                        {result.apy.toFixed(2)}%
                      </span>
                    </div>

                    {/* Rewards APY */}
                    <div className="flex justify-between items-center pb-2 border-b border-slate-700">
                      <span className="text-xs text-omni-silver font-medium">
                        🎁 Rewards APY
                      </span>
                      <span className={`text-sm font-mono font-semibold ${
                        result.rewards > 0 ? 'text-emerald-400' : 'text-slate-500'
                      }`}>
                        {result.rewards > 0 ? '+' : ''}{result.rewards.toFixed(2)}%
                      </span>
                    </div>

                    {/* Historical APY Metrics */}
                    {result.apyMetrics && result.apyMetrics.daysWithData > 0 && (
                      <div className="pt-2 pb-2 border-b border-slate-700">
                        <div className="text-xs text-omni-silver font-medium mb-2">📊 Historical APY</div>
                        <div className="grid grid-cols-2 gap-2">
                          {result.apyMetrics.apy7d !== undefined && result.apyMetrics.daysWithData >= 7 && (
                            <div className="flex justify-between items-center">
                              <span className="text-xs text-slate-400">7d avg</span>
                              <span className="text-xs font-mono text-white">{result.apyMetrics.apy7d.toFixed(2)}%</span>
                            </div>
                          )}
                          {result.apyMetrics.apy30d !== undefined && result.apyMetrics.daysWithData >= 30 && (
                            <div className="flex justify-between items-center">
                              <span className="text-xs text-slate-400">30d avg</span>
                              <span className="text-xs font-mono text-white">{result.apyMetrics.apy30d.toFixed(2)}%</span>
                            </div>
                          )}
                          {result.apyMetrics.apy60d !== undefined && result.apyMetrics.daysWithData >= 60 && (
                            <div className="flex justify-between items-center">
                              <span className="text-xs text-slate-400">60d avg</span>
                              <span className="text-xs font-mono text-white">{result.apyMetrics.apy60d.toFixed(2)}%</span>
                            </div>
                          )}
                          {result.apyMetrics.apy90d !== undefined && result.apyMetrics.daysWithData >= 90 && (
                            <div className="flex justify-between items-center">
                              <span className="text-xs text-slate-400">90d avg</span>
                              <span className="text-xs font-mono text-white">{result.apyMetrics.apy90d.toFixed(2)}%</span>
                            </div>
                          )}
                        </div>
                        <div className="flex justify-between items-center mt-2">
                          <span className="text-xs text-slate-400">Volatility</span>
                          <span className="text-xs font-mono text-slate-300">{result.apyMetrics.volatility.toFixed(2)}%</span>
                        </div>
                      </div>
                    )}

                    {/* Reserve Liquidity */}
                    <div className="flex justify-between items-center pt-2">
                      <span className="text-xs text-omni-silver font-medium">💰 Reserve Liquidity</span>
                      <span className="text-xs text-white font-semibold">{formatNumber(result.totalLiquidity || result.liquidity)}</span>
                    </div>

                    {/* Available Liquidity */}
                    <div className="flex justify-between items-center">
                      <span className="text-xs text-omni-silver font-medium">✅ Available Liquidity</span>
                      <span className="text-xs text-white">{formatNumber(result.liquidity)}</span>
                    </div>

                    {/* Utilization */}
                    <div className="flex justify-between items-center">
                      <span className="text-xs text-omni-silver font-medium">📈 Utilization Rate</span>
                      <span className="text-xs text-white">{result.utilizationRate.toFixed(2)}%</span>
                    </div>
                  </div>
                </div>

                {/* Action Buttons */}
                <div className="flex gap-2">
                  {/* Details button */}
                  <button
                    onClick={() => setSelectedVault(result)}
                    className="flex items-center justify-center gap-1.5 px-3 py-2.5 bg-slate-700 hover:bg-slate-600 text-slate-300 hover:text-white rounded-lg transition-colors text-sm font-medium border border-slate-600"
                    title="View APY history & details"
                  >
                    <BarChart2 className="w-4 h-4" />
                  </button>

                  {/* External link button */}
                  <a
                    href={result.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="flex items-center justify-center flex-1 gap-2 px-4 py-2.5 bg-omni-blue hover:bg-blue-600 text-white rounded-lg transition-colors text-sm font-medium"
                  >
                    {action === 'supply' ? 'Supply Now' : 'Borrow Now'}
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                      <path d="M6.02761 3.42871C5.07671 3.47152 4.14468 3.57774 3.23689 3.68288C2.27739 3.79401 1.51075 4.56009 1.40314 5.51999C1.2722 6.68798 1.14258 7.89621 1.14258 9.13316C1.14258 10.3701 1.2722 11.5783 1.40314 12.7463C1.51075 13.7062 2.27737 14.4723 3.23687 14.5834C4.41124 14.7195 5.6262 14.8573 6.87007 14.8573C8.11395 14.8573 9.3289 14.7195 10.5033 14.5834C11.4628 14.4723 12.2294 13.7062 12.337 12.7463C12.4322 11.8975 12.5266 11.0274 12.5711 10.1405" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
                      <path d="M9.71484 1.71436H14.2863V6.28578" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                      <path d="M7.42773 8.5715L14.2849 1.71436" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                    </svg>
                  </a>
                </div>
              </div>
            );
            })}
          </div>

          {/* Load More Button */}
          {visibleCount < filteredResults.length && (
            <div className="mt-6 text-center">
              <button
                onClick={loadMore}
                className="inline-flex items-center px-6 py-3 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors font-medium"
              >
                <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                </svg>
                Load More
              </button>
            </div>
          )}
        </div>
        );
      })()}

      {/* Vault Detail Drawer */}
      <VaultDetailDrawer
        vault={selectedVault}
        action={action}
        onClose={() => setSelectedVault(null)}
      />
    </div>
  )
}
