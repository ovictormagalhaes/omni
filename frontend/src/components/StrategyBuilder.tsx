import { useState, useEffect } from 'react'
import { ChevronDown, ArrowRight, Zap, TrendingUp, TrendingDown, RefreshCw, DollarSign, ArrowLeft } from 'lucide-react'
import { searchRates, fetchAvailableAssets, AssetInfo, RateResult } from '../lib/api'
import { ProtocolIcon } from './ProtocolIcon'
import { ChainIcon } from './ChainIcon'

type Chain = 'ethereum' | 'solana' | 'bsc' | 'bitcoin' | 'tron' | 'base' | 'arbitrum' | 'polygon' | 'optimism' | 'avalanche' | 'sui' | 'hyperliquid' | 'scroll' | 'mantle' | 'linea' | 'blast' | 'fantom' | 'zksync' | 'aptos' | 'celo'
type Protocol = 'aave' | 'kamino' | 'morpho' | 'fluid' | 'sparklend' | 'justlend' | 'euler' | 'jupiter' | 'lido' | 'marinade' | 'jito' | 'rocketpool'
type AssetCategory = 'usd-correlated' | 'stablecoin' | 'btc-correlated' | 'eth-correlated' | 'sol-correlated' | 'other'
type Step = 1 | 2 | 3 | 4

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

interface StrategyPair {
  supply: RateResult
  borrow: RateResult
  netApy: number
}

// ─── Componente de Filtros ───────────────────────────────────────────────────

interface FilterPanelProps {
  title: string
  icon: React.ReactNode
  accentColor: string
  availableAssets: AssetInfo[]
  assetsLoading: boolean
  selectedAssets: string[]
  onToggleAsset: (symbol: string) => void
  onSelectAllAssets: () => void
  onDeselectAllAssets: () => void
  isAssetOpen: boolean
  onToggleAssetOpen: () => void
  selectedAssetCategory: AssetCategory | null
  onSetAssetCategory: (cat: AssetCategory | null) => void
  selectedChains: Chain[]
  onToggleChain: (chain: Chain) => void
  onSelectAllChains: () => void
  onDeselectAllChains: () => void
  isChainsOpen: boolean
  onToggleChainsOpen: () => void
  selectedProtocols: Protocol[]
  onToggleProtocol: (protocol: Protocol) => void
  onSelectAllProtocols: () => void
  onDeselectAllProtocols: () => void
  isProtocolsOpen: boolean
  onToggleProtocolsOpen: () => void
}

function FilterPanel(props: FilterPanelProps) {
  const assetLabel = () => {
    if (props.assetsLoading) return 'Loading...'
    if (props.selectedAssets.length === 0 || props.selectedAssets.length === props.availableAssets.length) return 'All Assets'
    if (props.selectedAssets.length === 1) return props.selectedAssets[0]
    return `${props.selectedAssets.length} assets`
  }
  const chainLabel = () => {
    if (props.selectedChains.length === CHAINS.length) return 'All Chains'
    if (props.selectedChains.length === 1) return CHAINS.find(c => c.value === props.selectedChains[0])?.label ?? '1 chain'
    return `${props.selectedChains.length} chains`
  }
  const protocolLabel = () => {
    if (props.selectedProtocols.length === PROTOCOLS.length) return 'All Protocols'
    if (props.selectedProtocols.length === 1) return PROTOCOLS.find(p => p.value === props.selectedProtocols[0])?.label ?? '1 protocol'
    return `${props.selectedProtocols.length} protocols`
  }

  return (
    <div className={`card border-l-4 ${props.accentColor}`}>
      <div className="flex items-center gap-2 mb-5">
        {props.icon}
        <h3 className="text-xl font-semibold text-white">{props.title}</h3>
      </div>

      <div className="space-y-4">
        {/* Assets */}
        <div className="relative">
          <label className="block text-sm font-medium text-omni-silver mb-2">Asset</label>
          <button
            onClick={props.onToggleAssetOpen}
            disabled={props.assetsLoading}
            className="w-full bg-slate-700 border border-slate-600 rounded-lg px-4 py-3 text-white flex items-center justify-between hover:border-slate-500 transition-colors disabled:opacity-60"
          >
            <span>{assetLabel()}</span>
            <ChevronDown className={`w-5 h-5 transition-transform ${props.isAssetOpen ? 'rotate-180' : ''}`} />
          </button>
          {props.isAssetOpen && (
            <div className="absolute z-20 w-full mt-1 bg-slate-700 border border-slate-600 rounded-lg shadow-xl max-h-64 overflow-y-auto">
              <div className="p-2">
                <div className="flex gap-2 mb-2 pb-2 border-b border-slate-600">
                  <button onClick={props.onSelectAllAssets} className="flex-1 px-3 py-1.5 text-xs font-medium bg-omni-blue text-white rounded hover:bg-blue-600 transition-colors">✓ All</button>
                  <button onClick={props.onDeselectAllAssets} className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-600 text-white rounded hover:bg-slate-500 transition-colors">✗ None</button>
                </div>
                {props.availableAssets.map(a => (
                  <label key={a.symbol} className="flex items-center px-3 py-2 hover:bg-slate-600 rounded cursor-pointer">
                    <input type="checkbox" checked={props.selectedAssets.includes(a.symbol)} onChange={() => props.onToggleAsset(a.symbol)} className="mr-3 w-4 h-4 text-omni-blue bg-slate-600 border-slate-500 rounded" />
                    <span className="text-white font-mono text-sm">{a.symbol}</span>
                  </label>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Asset Category */}
        <div>
          <label className="block text-sm font-medium text-omni-silver mb-2">Asset Category</label>
          <div className="flex flex-wrap gap-2">
            {ASSET_CATEGORIES.map(cat => (
              <button
                key={cat.value}
                onClick={() => props.onSetAssetCategory(props.selectedAssetCategory === cat.value ? null : cat.value)}
                className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                  props.selectedAssetCategory === cat.value
                    ? 'bg-omni-gold text-slate-900'
                    : 'bg-slate-700 text-omni-silver hover:bg-slate-600'
                }`}
              >
                {props.selectedAssetCategory === cat.value && '✓ '}{cat.label}
              </button>
            ))}
          </div>
        </div>

        {/* Chains */}
        <div className="relative">
          <label className="block text-sm font-medium text-omni-silver mb-2">Chain</label>
          <button
            onClick={props.onToggleChainsOpen}
            className="w-full bg-slate-700 border border-slate-600 rounded-lg px-4 py-3 text-white flex items-center justify-between hover:border-slate-500 transition-colors"
          >
            <span>{chainLabel()}</span>
            <ChevronDown className={`w-5 h-5 transition-transform ${props.isChainsOpen ? 'rotate-180' : ''}`} />
          </button>
          {props.isChainsOpen && (
            <div className="absolute z-20 w-full mt-1 bg-slate-700 border border-slate-600 rounded-lg shadow-xl max-h-64 overflow-y-auto">
              <div className="p-2">
                <div className="flex gap-2 mb-2 pb-2 border-b border-slate-600">
                  <button onClick={props.onSelectAllChains} className="flex-1 px-3 py-1.5 text-xs font-medium bg-omni-blue text-white rounded hover:bg-blue-600 transition-colors">✓ All</button>
                  <button onClick={props.onDeselectAllChains} className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-600 text-white rounded hover:bg-slate-500 transition-colors">✗ None</button>
                </div>
                {CHAINS.map(chain => (
                  <label key={chain.value} className="flex items-center px-3 py-2 hover:bg-slate-600 rounded cursor-pointer">
                    <input type="checkbox" checked={props.selectedChains.includes(chain.value)} onChange={() => props.onToggleChain(chain.value)} className="mr-3 w-4 h-4 text-omni-blue bg-slate-600 border-slate-500 rounded" />
                    <ChainIcon chain={chain.value} className="w-4 h-4 mr-2" />
                    <span className="text-white text-sm">{chain.label}</span>
                  </label>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Protocols */}
        <div className="relative">
          <label className="block text-sm font-medium text-omni-silver mb-2">Protocol</label>
          <button
            onClick={props.onToggleProtocolsOpen}
            className="w-full bg-slate-700 border border-slate-600 rounded-lg px-4 py-3 text-white flex items-center justify-between hover:border-slate-500 transition-colors"
          >
            <span>{protocolLabel()}</span>
            <ChevronDown className={`w-5 h-5 transition-transform ${props.isProtocolsOpen ? 'rotate-180' : ''}`} />
          </button>
          {props.isProtocolsOpen && (
            <div className="absolute z-20 w-full mt-1 bg-slate-700 border border-slate-600 rounded-lg shadow-xl max-h-64 overflow-y-auto">
              <div className="p-2">
                <div className="flex gap-2 mb-2 pb-2 border-b border-slate-600">
                  <button onClick={props.onSelectAllProtocols} className="flex-1 px-3 py-1.5 text-xs font-medium bg-omni-blue text-white rounded hover:bg-blue-600 transition-colors">✓ All</button>
                  <button onClick={props.onDeselectAllProtocols} className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-600 text-white rounded hover:bg-slate-500 transition-colors">✗ None</button>
                </div>
                {PROTOCOLS.map(protocol => (
                  <label key={protocol.value} className="flex items-center px-3 py-2 hover:bg-slate-600 rounded cursor-pointer">
                    <input type="checkbox" checked={props.selectedProtocols.includes(protocol.value)} onChange={() => props.onToggleProtocol(protocol.value)} className="mr-3 w-4 h-4 text-omni-blue bg-slate-600 border-slate-500 rounded" />
                    <ProtocolIcon protocol={protocol.value} className="w-4 h-4 mr-2" />
                    <span className="text-white text-sm">{protocol.label}</span>
                  </label>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

// ─── Componente principal ────────────────────────────────────────────────────

export default function StrategyBuilder() {
  const [currentStep, setCurrentStep] = useState<Step>(1)
  const [availableAssets, setAvailableAssets] = useState<AssetInfo[]>([])
  const [assetsLoading, setAssetsLoading] = useState(true)
  
  // Supply filters
  const [supplyAssets, setSupplyAssets] = useState<string[]>([])
  const [supplyChains, setSupplyChains] = useState<Chain[]>(CHAINS.map(c => c.value))
  const [supplyProtocols, setSupplyProtocols] = useState<Protocol[]>(PROTOCOLS.map(p => p.value))
  const [supplyAssetCategory, setSupplyAssetCategory] = useState<AssetCategory | null>(null)
  const [supplyAssetOpen, setSupplyAssetOpen] = useState(false)
  const [supplyChainsOpen, setSupplyChainsOpen] = useState(false)
  const [supplyProtocolsOpen, setSupplyProtocolsOpen] = useState(false)
  
  // Borrow filters
  const [borrowAssets, setBorrowAssets] = useState<string[]>([])
  const [borrowChains, setBorrowChains] = useState<Chain[]>(CHAINS.map(c => c.value))
  const [borrowProtocols, setBorrowProtocols] = useState<Protocol[]>(PROTOCOLS.map(p => p.value))
  const [borrowAssetCategory, setBorrowAssetCategory] = useState<AssetCategory | null>(null)
  const [borrowAssetOpen, setBorrowAssetOpen] = useState(false)
  const [borrowChainsOpen, setBorrowChainsOpen] = useState(false)
  const [borrowProtocolsOpen, setBorrowProtocolsOpen] = useState(false)
  
  // Results
  const [pairs, setPairs] = useState<StrategyPair[]>([])
  const [selectedPairs, setSelectedPairs] = useState<StrategyPair[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetchAvailableAssets().then(assets => {
      if (assets.length > 0) {
        setAvailableAssets(assets)
        setSupplyAssets(assets.map(a => a.symbol))
        setBorrowAssets(assets.map(a => a.symbol))
      }
      setAssetsLoading(false)
    })
  }, [])

  const toParam = (arr: string[], total: number) =>
    arr.length > 0 && arr.length < total ? arr.join(',') : undefined

  const handleSearchPairs = async () => {
    setLoading(true)
    setError(null)
    setSupplyAssetOpen(false); setSupplyChainsOpen(false); setSupplyProtocolsOpen(false)
    setBorrowAssetOpen(false); setBorrowChainsOpen(false); setBorrowProtocolsOpen(false)

    try {
      const [supplyData, borrowData] = await Promise.all([
        searchRates({
          action: 'supply',
          assets: toParam(supplyAssets, availableAssets.length),
          chains: toParam(supplyChains, CHAINS.length),
          protocols: toParam(supplyProtocols, PROTOCOLS.length),
          asset_categories: supplyAssetCategory || undefined,
        }),
        searchRates({
          action: 'borrow',
          assets: toParam(borrowAssets, availableAssets.length),
          chains: toParam(borrowChains, CHAINS.length),
          protocols: toParam(borrowProtocols, PROTOCOLS.length),
          asset_categories: borrowAssetCategory || undefined,
        }),
      ])

      const topSupply = supplyData.results.slice(0, 10)
      const topBorrow = borrowData.results.slice(0, 10)

      const combined: StrategyPair[] = []
      for (const s of topSupply) {
        for (const b of topBorrow) {
          if (s.chain === b.chain && s.protocol === b.protocol) {
            combined.push({ supply: s, borrow: b, netApy: s.netApy - b.netApy })
          }
        }
      }
      combined.sort((a, b) => b.netApy - a.netApy)
      setPairs(combined)
      setCurrentStep(3)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch rates')
    } finally {
      setLoading(false)
    }
  }

  const handleRestart = () => {
    setCurrentStep(1)
    setPairs([])
    setSelectedPairs([])
    setError(null)
  }

  // Helper to create unique ID for a pair
  const getPairId = (pair: StrategyPair) => {
    const supplyId = `${pair.supply.protocol}-${pair.supply.chain}-${pair.supply.asset}-${pair.supply.vaultName || 'default'}`
    const borrowId = `${pair.borrow.protocol}-${pair.borrow.chain}-${pair.borrow.asset}-${pair.borrow.vaultName || 'default'}`
    return `${supplyId}__${borrowId}`
  }

  const formatApy = (v: number) => `${v >= 0 ? '+' : ''}${v.toFixed(2)}%`
  const formatNum = (n: number) => {
    if (n >= 1_000_000) return `$${(n / 1_000_000).toFixed(1)}M`
    if (n >= 1_000) return `$${(n / 1_000).toFixed(1)}K`
    return `$${n.toFixed(0)}`
  }

  return (
    <div className="space-y-8" id="strategy">
      {/* Header */}
      <div className="card">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-3 mb-2">
              <Zap className="w-6 h-6 text-omni-gold" />
              <h2 className="text-2xl font-bold text-white">Strategy Builder</h2>
            </div>
            <p className="text-omni-silver text-sm">
              Build your carry-trade strategy step by step
            </p>
          </div>
          {/* Stepper indicator */}
          <div className="flex items-center gap-2">
            {[1, 2, 3, 4].map(step => (
              <div key={step} className="flex items-center">
                <div className={`w-8 h-8 rounded-full flex items-center justify-center text-sm font-semibold transition-colors ${
                  currentStep === step ? 'bg-omni-gold text-slate-900' :
                  currentStep > step ? 'bg-green-600 text-white' :
                  'bg-slate-700 text-omni-silver'
                }`}>
                  {step}
                </div>
                {step < 4 && <div className={`w-8 h-0.5 ${currentStep > step ? 'bg-green-600' : 'bg-slate-700'}`} />}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="card border border-red-500/30 bg-red-900/10 text-red-400 text-sm">{error}</div>
      )}

      {/* Step 1: Supply Rules */}
      {currentStep === 1 && (
        <div className="space-y-6">
          <FilterPanel
            title="Supply Rules"
            icon={<TrendingUp className="w-5 h-5 text-omni-blue" />}
            accentColor="border-omni-blue"
            availableAssets={availableAssets}
            assetsLoading={assetsLoading}
            selectedAssets={supplyAssets}
            onToggleAsset={s => setSupplyAssets(prev => prev.includes(s) ? prev.filter(x => x !== s) : [...prev, s])}
            onSelectAllAssets={() => setSupplyAssets(availableAssets.map(a => a.symbol))}
            onDeselectAllAssets={() => setSupplyAssets([])}
            isAssetOpen={supplyAssetOpen}
            onToggleAssetOpen={() => setSupplyAssetOpen(o => !o)}
            selectedAssetCategory={supplyAssetCategory}
            onSetAssetCategory={setSupplyAssetCategory}
            selectedChains={supplyChains}
            onToggleChain={c => setSupplyChains(prev => prev.includes(c) ? prev.filter(x => x !== c) : [...prev, c])}
            onSelectAllChains={() => setSupplyChains(CHAINS.map(c => c.value))}
            onDeselectAllChains={() => setSupplyChains([])}
            isChainsOpen={supplyChainsOpen}
            onToggleChainsOpen={() => setSupplyChainsOpen(o => !o)}
            selectedProtocols={supplyProtocols}
            onToggleProtocol={p => setSupplyProtocols(prev => prev.includes(p) ? prev.filter(x => x !== p) : [...prev, p])}
            onSelectAllProtocols={() => setSupplyProtocols(PROTOCOLS.map(p => p.value))}
            onDeselectAllProtocols={() => setSupplyProtocols([])}
            isProtocolsOpen={supplyProtocolsOpen}
            onToggleProtocolsOpen={() => setSupplyProtocolsOpen(o => !o)}
          />
          <div className="flex justify-end">
            <button onClick={() => setCurrentStep(2)} className="btn-primary flex items-center gap-2">
              Next: Borrow Rules
              <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Step 2: Borrow Rules */}
      {currentStep === 2 && (
        <div className="space-y-6">
          <FilterPanel
            title="Borrow Rules"
            icon={<TrendingDown className="w-5 h-5 text-omni-red" />}
            accentColor="border-omni-red"
            availableAssets={availableAssets}
            assetsLoading={assetsLoading}
            selectedAssets={borrowAssets}
            onToggleAsset={s => setBorrowAssets(prev => prev.includes(s) ? prev.filter(x => x !== s) : [...prev, s])}
            onSelectAllAssets={() => setBorrowAssets(availableAssets.map(a => a.symbol))}
            onDeselectAllAssets={() => setBorrowAssets([])}
            isAssetOpen={borrowAssetOpen}
            onToggleAssetOpen={() => setBorrowAssetOpen(o => !o)}
            selectedAssetCategory={borrowAssetCategory}
            onSetAssetCategory={setBorrowAssetCategory}
            selectedChains={borrowChains}
            onToggleChain={c => setBorrowChains(prev => prev.includes(c) ? prev.filter(x => x !== c) : [...prev, c])}
            onSelectAllChains={() => setBorrowChains(CHAINS.map(c => c.value))}
            onDeselectAllChains={() => setBorrowChains([])}
            isChainsOpen={borrowChainsOpen}
            onToggleChainsOpen={() => setBorrowChainsOpen(o => !o)}
            selectedProtocols={borrowProtocols}
            onToggleProtocol={p => setBorrowProtocols(prev => prev.includes(p) ? prev.filter(x => x !== p) : [...prev, p])}
            onSelectAllProtocols={() => setBorrowProtocols(PROTOCOLS.map(p => p.value))}
            onDeselectAllProtocols={() => setBorrowProtocols([])}
            isProtocolsOpen={borrowProtocolsOpen}
            onToggleProtocolsOpen={() => setBorrowProtocolsOpen(o => !o)}
          />
          <div className="flex justify-between">
            <button onClick={() => setCurrentStep(1)} className="btn-secondary flex items-center gap-2">
              <ArrowLeft className="w-4 h-4" />
              Back
            </button>
            <button onClick={handleSearchPairs} disabled={loading} className="btn-primary flex items-center gap-2 disabled:opacity-60">
              {loading ? (
                <>
                  <svg className="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8z" />
                  </svg>
                  Searching...
                </>
              ) : (
                <>
                  Find Strategies
                  <ArrowRight className="w-4 h-4" />
                </>
              )}
            </button>
          </div>
        </div>
      )}

      {/* Step 3: Select Strategy */}
      {currentStep === 3 && (
        <div className="space-y-6">
          <div className="card">
            <h3 className="text-lg font-semibold text-white mb-2">Select Strategies (up to 3)</h3>
            <p className="text-omni-silver text-sm">
              Found {pairs.length} viable carry-trade {pairs.length === 1 ? 'pair' : 'pairs'}. 
              Select up to 3 strategies to compare and simulate. 
              {selectedPairs.length > 0 && <span className="text-omni-gold ml-2">({selectedPairs.length} selected)</span>}
            </p>
          </div>
          {pairs.length === 0 ? (
            <div className="card text-center text-omni-silver py-12">
              No strategy pairs found with the current filters.
              <div className="mt-4">
                <button onClick={() => setCurrentStep(1)} className="btn-secondary">
                  Adjust Filters
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
                {pairs.map((pair, i) => {
                  const pairId = getPairId(pair)
                  const isSelected = selectedPairs.some(p => getPairId(p) === pairId)
                  const canSelect = selectedPairs.length < 3 || isSelected
                  
                  return (
                    <button 
                      key={i}
                      onClick={() => {
                        if (isSelected) {
                          setSelectedPairs(prev => prev.filter(p => getPairId(p) !== pairId))
                        } else if (canSelect) {
                          setSelectedPairs(prev => [...prev, pair])
                        }
                      }}
                      disabled={!canSelect && !isSelected}
                      className={`text-left card transition-all border ${
                        isSelected 
                          ? 'border-omni-gold bg-omni-gold/10 shadow-lg' 
                          : canSelect
                            ? 'border-slate-700/50 hover:border-omni-gold'
                            : 'border-slate-700/30 opacity-50 cursor-not-allowed'
                      }`}
                    >
                      <div className="flex items-start gap-3 mb-3">
                        <div className={`w-5 h-5 rounded border-2 flex items-center justify-center flex-shrink-0 mt-0.5 ${
                          isSelected 
                            ? 'bg-omni-gold border-omni-gold' 
                            : 'border-slate-500'
                        }`}>
                          {isSelected && (
                            <svg className="w-3 h-3 text-slate-900" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                            </svg>
                          )}
                        </div>
                        <div className="flex-1">
                          <PairCard pair={pair} formatApy={formatApy} formatNum={formatNum} />
                        </div>
                      </div>
                    </button>
                  )
                })}
              </div>
              <div className="flex justify-between">
                <button onClick={() => setCurrentStep(2)} className="btn-secondary flex items-center gap-2">
                  <ArrowLeft className="w-4 h-4" />
                  Back
                </button>
                <button 
                  onClick={() => setCurrentStep(4)} 
                  disabled={selectedPairs.length === 0}
                  className="btn-primary flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Next: Simulate
                  <ArrowRight className="w-4 h-4" />
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {/* Step 4: Simulation */}
      {currentStep === 4 && selectedPairs.length > 0 && (
        <div className="space-y-6">
          <div className="card border-omni-gold border-2">
            <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
              <DollarSign className="w-5 h-5 text-omni-gold" />
              Strategy Simulation ({selectedPairs.length} {selectedPairs.length === 1 ? 'Strategy' : 'Strategies'})
            </h3>
            <SimulationView pairs={selectedPairs} formatApy={formatApy} formatNum={formatNum} />
          </div>
          <div className="flex justify-between">
            <button onClick={() => setCurrentStep(3)} className="btn-secondary flex items-center gap-2">
              <ArrowLeft className="w-4 h-4" />
              Back to Selection
            </button>
            <button onClick={handleRestart} className="btn-primary flex items-center gap-2">
              <RefreshCw className="w-4 h-4" />
              Start New Search
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

// ─── Pair Card (Step 3) ──────────────────────────────────────────────────────

function PairCard({ pair, formatApy, formatNum }: {
  pair: StrategyPair
  formatApy: (v: number) => string
  formatNum: (n: number) => string
}) {
  const { supply, borrow, netApy } = pair
  const isPositive = netApy >= 0

  return (
    <>
      <div className="flex items-center justify-between mb-4">
        <span className="text-xs font-medium text-omni-silver uppercase tracking-wider">NET APY</span>
        <span className={`text-2xl font-bold ${isPositive ? 'text-green-400' : 'text-red-400'}`}>
          {formatApy(netApy)}
        </span>
      </div>
      <div className="bg-slate-800/50 rounded-lg p-3 mb-2">
        <div className="flex items-center gap-1 mb-2">
          <TrendingUp className="w-3.5 h-3.5 text-omni-blue" />
          <span className="text-xs font-semibold text-omni-blue uppercase">Supply</span>
        </div>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <ProtocolIcon protocol={supply.protocol} className="w-5 h-5" />
            <div>
              <div className="text-sm font-medium text-white">
                {supply.asset}
              </div>
              <div className="flex items-center gap-1 mt-0.5">
                <ChainIcon chain={supply.chain} className="w-3 h-3" />
                <span className="text-xs text-omni-silver capitalize">{supply.chain}</span>
              </div>
            </div>
          </div>
          <div className="text-right">
            <div className="text-sm font-bold text-green-400">{supply.netApy.toFixed(2)}%</div>
            <div className="text-xs text-omni-silver">{formatNum(supply.liquidity)}</div>
          </div>
        </div>
      </div>
      <div className="bg-slate-800/50 rounded-lg p-3">
        <div className="flex items-center gap-1 mb-2">
          <TrendingDown className="w-3.5 h-3.5 text-omni-red" />
          <span className="text-xs font-semibold text-omni-red uppercase">Borrow</span>
        </div>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <ProtocolIcon protocol={borrow.protocol} className="w-5 h-5" />
            <div>
              <div className="text-sm font-medium text-white">
                {borrow.asset}
              </div>
              <div className="flex items-center gap-1 mt-0.5">
                <ChainIcon chain={borrow.chain} className="w-3 h-3" />
                <span className="text-xs text-omni-silver capitalize">{borrow.chain}</span>
              </div>
            </div>
          </div>
          <div className="text-right">
            <div className="text-sm font-bold text-red-400">{borrow.netApy.toFixed(2)}%</div>
            <div className="text-xs text-omni-silver">{formatNum(borrow.liquidity)}</div>
          </div>
        </div>
      </div>
    </>
  )
}

// ─── Simulation View (Step 4) ────────────────────────────────────────────────

function SimulationView({ pairs, formatApy, formatNum }: {
  pairs: StrategyPair[]
  formatApy: (v: number) => string
  formatNum: (n: number) => string
}) {
  // Calculate minimum LTV across all selected strategies
  const minLtv = Math.min(...pairs.map(p => p.supply.collateralLtv || 0.7))
  const initialBorrowPercent = Math.round(minLtv * 70) // 70% of max LTV as default
  
  const [supplyAmount, setSupplyAmount] = useState(1000)
  const [borrowPercent, setBorrowPercent] = useState(initialBorrowPercent)
  const [supplyInput, setSupplyInput] = useState('1000')
  const [borrowInput, setBorrowInput] = useState(initialBorrowPercent.toString())
  
  // Calculate actual borrow amount from percentage
  const maxBorrowPercent = minLtv * 100
  const borrowAmount = (supplyAmount * borrowPercent) / 100
  
  const handleSupplySliderChange = (value: number) => {
    setSupplyAmount(value)
    setSupplyInput(value.toString())
  }
  
  const handleSupplyInputChange = (value: string) => {
    setSupplyInput(value)
    const num = parseFloat(value)
    if (!isNaN(num) && num >= 100 && num <= 100000) {
      setSupplyAmount(num)
    }
  }
  
  const handleBorrowSliderChange = (value: number) => {
    setBorrowPercent(value)
    setBorrowInput(value.toString())
  }
  
  const handleBorrowInputChange = (value: string) => {
    setBorrowInput(value)
    const num = parseFloat(value)
    if (!isNaN(num) && num >= 0 && num <= maxBorrowPercent) {
      setBorrowPercent(num)
    }
  }
  
  // Calculate returns for each strategy
  const strategyReturns = pairs.map(pair => {
    const supplyReturn = supplyAmount * (pair.supply.netApy / 100)
    const borrowCost = borrowAmount * (pair.borrow.netApy / 100)
    const netReturn = supplyReturn - borrowCost
    const netApy = supplyAmount > 0 ? (netReturn / supplyAmount) * 100 : 0
    
    return {
      pair,
      supplyReturn,
      borrowCost,
      netReturn,
      netApy
    }
  })

  // Find best strategy by net APY
  const bestStrategy = strategyReturns.reduce((best, current) => 
    current.netApy > best.netApy ? current : best
  , strategyReturns[0])

  return (
    <div className="space-y-6">
      {/* Amount Controls */}
      <div className="card bg-slate-800/50">
        <h4 className="text-base font-semibold text-white mb-4 flex items-center gap-2">
          <DollarSign className="w-5 h-5 text-omni-gold" />
          Amount Simulation
        </h4>
        
        <div className="space-y-6">
          {/* Supply Amount Control */}
          <div className="bg-slate-700/30 rounded-lg p-4">
            <div className="flex items-center justify-between mb-3">
              <label className="text-sm font-medium text-white flex items-center gap-2">
                <TrendingUp className="w-4 h-4 text-omni-blue" />
                Supply Amount
              </label>
              <input
                type="text"
                value={supplyInput}
                onChange={(e) => handleSupplyInputChange(e.target.value)}
                onBlur={() => setSupplyInput(supplyAmount.toString())}
                className="w-32 px-3 py-1.5 bg-slate-600 border border-slate-500 rounded text-white text-right text-sm focus:outline-none focus:ring-2 focus:ring-omni-blue"
                placeholder="Amount"
              />
            </div>
            <input
              type="range"
              min="100"
              max="100000"
              step="100"
              value={supplyAmount}
              onChange={(e) => handleSupplySliderChange(Number(e.target.value))}
              className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer slider"
            />
            <div className="flex justify-between text-xs text-omni-silver mt-1">
              <span>$100</span>
              <span>$100K</span>
            </div>
          </div>

          {/* Borrow Percentage Control */}
          <div className="bg-slate-700/30 rounded-lg p-4">
            <div className="flex items-center justify-between mb-3">
              <label className="text-sm font-medium text-white flex items-center gap-2">
                <TrendingDown className="w-4 h-4 text-omni-red" />
                Borrow Percentage
              </label>
              <input
                type="text"
                value={borrowInput}
                onChange={(e) => handleBorrowInputChange(e.target.value)}
                onBlur={() => setBorrowInput(borrowPercent.toString())}
                className="w-32 px-3 py-1.5 bg-slate-600 border border-slate-500 rounded text-white text-right text-sm focus:outline-none focus:ring-2 focus:ring-omni-red"
                placeholder="Percent"
              />
            </div>
            <input
              type="range"
              min="0"
              max={maxBorrowPercent}
              step="1"
              value={borrowPercent}
              onChange={(e) => handleBorrowSliderChange(Number(e.target.value))}
              className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer slider"
            />
            <div className="flex justify-between text-xs text-omni-silver mt-1">
              <span>0%</span>
              <span>Max: {maxBorrowPercent.toFixed(0)}% (Min LTV)</span>
            </div>
            <div className="text-sm text-white mt-2">
              Borrow Amount: <span className="font-bold text-omni-gold">${borrowAmount.toFixed(0)}</span>
            </div>
          </div>
        </div>
      </div>

      {/* Strategy Comparison */}
      <div className="space-y-3">
        <h4 className="text-base font-semibold text-white">Strategy Comparison</h4>
        {strategyReturns.map((result, idx) => {
          const isBest = result === bestStrategy && pairs.length > 1
          
          return (
            <div 
              key={idx} 
              className={`card transition-all ${
                isBest 
                  ? 'border-2 border-omni-gold bg-gradient-to-r from-omni-gold/10 to-transparent' 
                  : 'border border-slate-700'
              }`}
            >
              <div className="flex items-start justify-between mb-3">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-medium text-omni-silver uppercase tracking-wider">
                    Strategy {idx + 1}
                  </span>
                  {isBest && (
                    <span className="px-2 py-0.5 bg-omni-gold text-slate-900 text-xs font-bold rounded">
                      BEST
                    </span>
                  )}
                </div>
                <div className="text-right">
                  <div className={`text-2xl font-bold ${result.netApy >= 0 ? 'text-green-400' : 'text-red-400'}`}>
                    {result.netApy.toFixed(2)}%
                  </div>
                  <div className="text-xs text-omni-silver">NET APY</div>
                </div>
              </div>

              {/* Strategy Details */}
              <div className="grid grid-cols-2 gap-3 mb-3">
                <div className="bg-slate-700/50 rounded p-3">
                  <div className="flex items-center gap-1 mb-2">
                    <TrendingUp className="w-3 h-3 text-omni-blue" />
                    <span className="text-xs text-omni-silver">Supply</span>
                  </div>
                  <div className="flex items-center gap-2 mb-2">
                    <ProtocolIcon protocol={result.pair.supply.protocol} className="w-5 h-5" />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium text-white">{result.pair.supply.asset}</div>
                      <div className="flex items-center gap-1 text-xs text-omni-silver">
                        <ChainIcon chain={result.pair.supply.chain} className="w-3 h-3" />
                        <span className="capitalize">{result.pair.supply.chain}</span>
                      </div>
                    </div>
                  </div>
                  <div className="text-green-400 font-semibold">{result.pair.supply.netApy.toFixed(2)}% APY</div>
                </div>

                <div className="bg-slate-700/50 rounded p-3">
                  <div className="flex items-center gap-1 mb-2">
                    <TrendingDown className="w-3 h-3 text-omni-red" />
                    <span className="text-xs text-omni-silver">Borrow</span>
                  </div>
                  <div className="flex items-center gap-2 mb-2">
                    <ProtocolIcon protocol={result.pair.borrow.protocol} className="w-5 h-5" />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium text-white">{result.pair.borrow.asset}</div>
                      <div className="flex items-center gap-1 text-xs text-omni-silver">
                        <ChainIcon chain={result.pair.borrow.chain} className="w-3 h-3" />
                        <span className="capitalize">{result.pair.borrow.chain}</span>
                      </div>
                    </div>
                  </div>
                  <div className="text-red-400 font-semibold">{result.pair.borrow.netApy.toFixed(2)}% APY</div>
                </div>
              </div>

              {/* Results */}
              <div className="grid grid-cols-4 gap-2 pt-3 border-t border-slate-700">
                <div>
                  <div className="text-xs text-omni-silver">Supply</div>
                  <div className="text-sm font-semibold text-white">${supplyAmount.toLocaleString()}</div>
                </div>
                <div>
                  <div className="text-xs text-omni-silver">Borrow</div>
                  <div className="text-sm font-semibold text-white">${borrowAmount.toFixed(0)}</div>
                </div>
                <div>
                  <div className="text-xs text-omni-silver">Yearly Return</div>
                  <div className={`text-sm font-semibold ${result.netReturn >= 0 ? 'text-green-400' : 'text-red-400'}`}>
                    ${result.netReturn.toFixed(2)}
                  </div>
                </div>
                <div>
                  <div className="text-xs text-omni-silver">Impact</div>
                  <div className="text-sm font-semibold text-white">
                    {result === bestStrategy ? '—' : 
                     `${result.netApy > bestStrategy.netApy ? '+' : ''}${(result.netApy - bestStrategy.netApy).toFixed(2)}%`
                    }
                  </div>
                </div>
              </div>
            </div>
          )
        })}
      </div>

      <div className="bg-blue-900/20 border border-blue-500/30 rounded-lg p-3 text-xs text-blue-300">
        <strong>Note:</strong> This simulation uses the minimum LTV of {(minLtv * 100).toFixed(0)}% across all selected strategies. 
        Each strategy shows independent results for the same investment of ${supplyAmount.toLocaleString()} (supply) and ${borrowAmount.toFixed(0)} ({borrowPercent}% borrow).
        {pairs.length > 1 && ' Compare the strategies to find the best NET APY and yearly return.'}
      </div>
    </div>
  )
}
