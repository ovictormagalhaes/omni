import { useState, useRef, useEffect } from 'react'
import { ChevronDown, ArrowRight, Zap, TrendingUp, TrendingDown, RefreshCw, DollarSign, ArrowLeft, Search, Filter, Droplets, Repeat } from 'lucide-react'
import { searchRates, searchPools, RateResult, PoolResult } from '../lib/api'
import { ProtocolIcon } from './ProtocolIcon'
import { ChainIcon } from './ChainIcon'
import AssetCategoryFilter, { type AssetCategory } from './AssetCategoryFilter'

// ─── Types ───────────────────────────────────────────────────────────────────

type Chain = 'ethereum' | 'solana' | 'bsc' | 'bitcoin' | 'tron' | 'base' | 'arbitrum' | 'polygon' | 'optimism' | 'avalanche' | 'sui' | 'hyperliquid' | 'scroll' | 'mantle' | 'linea' | 'blast' | 'fantom' | 'zksync' | 'aptos' | 'celo'
type LendingProtocol = 'aave' | 'kamino' | 'morpho' | 'fluid' | 'sparklend' | 'justlend' | 'euler' | 'jupiter' | 'lido' | 'marinade' | 'jito' | 'rocketpool' | 'compound' | 'venus' | 'pendle' | 'ethena' | 'etherfi' | 'benqi' | 'radiant' | 'silo' | 'sky' | 'fraxeth' | 'aura' | 'convex' | 'yearn' | 'stargate' | 'gmx'
type PoolProtocol = 'uniswap' | 'uniswapv4' | 'raydium' | 'curve' | 'pancakeswap' | 'aerodrome' | 'velodrome' | 'orca' | 'meteora' | 'sushiswap' | 'camelot' | 'traderjoe' | 'balancer' | 'maverick'
type PoolChain = 'ethereum' | 'solana' | 'arbitrum' | 'base' | 'polygon' | 'optimism' | 'avalanche' | 'fantom' | 'bsc' | 'celo'
type Step = 1 | 2 | 3 | 4
type StrategyType = 'carry' | 'pool'
type TokenMode = 'category' | 'specific'

interface PairTokenConfig {
  mode: TokenMode
  category: AssetCategory | null
  specificToken: string
}

interface StrategyPair {
  supply: RateResult
  borrow: RateResult
  netApy: number
}

// ─── Constants ───────────────────────────────────────────────────────────────

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

const LENDING_PROTOCOLS: { value: LendingProtocol; label: string }[] = [
  { value: 'aave', label: 'Aave v3' },
  { value: 'aura', label: 'Aura' },
  { value: 'benqi', label: 'Benqi' },
  { value: 'compound', label: 'Compound V3' },
  { value: 'convex', label: 'Convex' },
  { value: 'ethena', label: 'Ethena' },
  { value: 'etherfi', label: 'EtherFi' },
  { value: 'euler', label: 'Euler' },
  { value: 'fluid', label: 'Fluid' },
  { value: 'fraxeth', label: 'Frax ETH' },
  { value: 'gmx', label: 'GMX' },
  { value: 'jito', label: 'Jito' },
  { value: 'jupiter', label: 'Jupiter' },
  { value: 'justlend', label: 'JustLend' },
  { value: 'kamino', label: 'Kamino' },
  { value: 'lido', label: 'Lido' },
  { value: 'marinade', label: 'Marinade' },
  { value: 'morpho', label: 'Morpho' },
  { value: 'pendle', label: 'Pendle' },
  { value: 'radiant', label: 'Radiant' },
  { value: 'rocketpool', label: 'Rocket Pool' },
  { value: 'silo', label: 'Silo' },
  { value: 'sky', label: 'Sky (Maker)' },
  { value: 'sparklend', label: 'SparkLend' },
  { value: 'stargate', label: 'Stargate' },
  { value: 'venus', label: 'Venus' },
  { value: 'yearn', label: 'Yearn' },
]

const POOL_PROTOCOLS: { value: PoolProtocol; label: string }[] = [
  { value: 'aerodrome', label: 'Aerodrome' },
  { value: 'balancer', label: 'Balancer' },
  { value: 'camelot', label: 'Camelot' },
  { value: 'curve', label: 'Curve' },
  { value: 'maverick', label: 'Maverick' },
  { value: 'meteora', label: 'Meteora' },
  { value: 'orca', label: 'Orca' },
  { value: 'pancakeswap', label: 'PancakeSwap' },
  { value: 'raydium', label: 'Raydium' },
  { value: 'sushiswap', label: 'SushiSwap' },
  { value: 'traderjoe', label: 'Trader Joe' },
  { value: 'uniswap', label: 'Uniswap V3' },
  { value: 'uniswapv4', label: 'Uniswap V4' },
  { value: 'velodrome', label: 'Velodrome' },
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

const CARRY_STEP_LABELS: Record<Step, string> = {
  1: 'Chains & Protocols',
  2: 'Asset Selection',
  3: 'Select Pools',
  4: 'Simulation',
}

const POOL_STEP_LABELS: Record<Step, string> = {
  1: 'Chains & Protocols',
  2: 'Token Selection',
  3: 'Select Pools',
  4: 'Simulation',
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

const formatApy = (v: number) => `${v >= 0 ? '+' : ''}${v.toFixed(2)}%`
const formatNum = (n: number) => {
  if (n >= 1_000_000_000) return `$${(n / 1_000_000_000).toFixed(2)}B`
  if (n >= 1_000_000) return `$${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `$${(n / 1_000).toFixed(1)}K`
  return `$${n.toFixed(0)}`
}

// ─── Shared: Token Config Panel ──────────────────────────────────────────────

function PairConfigPanel({
  label,
  subtitle,
  icon,
  accentColor,
  accentBg,
  config,
  onChange,
}: {
  label: string
  subtitle: string
  icon: React.ReactNode
  accentColor: string
  accentBg: string
  config: PairTokenConfig
  onChange: (c: PairTokenConfig) => void
}) {
  return (
    <div className={`card border-l-2 ${accentColor} flex-1`}>
      <div className="flex items-center gap-3 mb-5">
        <div className={`w-9 h-9 rounded-lg ${accentBg} flex items-center justify-center`}>
          {icon}
        </div>
        <div>
          <h3 className="text-base font-semibold text-white">{label}</h3>
          <p className="text-xs text-slate-500">{subtitle}</p>
        </div>
      </div>

      {/* Mode Toggle */}
      <div className="mb-4">
        <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Selection Mode</label>
        <div className="flex gap-2">
          <button
            onClick={() => onChange({ ...config, mode: 'category' })}
            className={`flex-1 px-4 py-2.5 rounded-lg text-sm font-medium transition-all duration-200 ${
              config.mode === 'category'
                ? 'bg-omni-gold/15 text-omni-gold-light border border-omni-gold/30'
                : 'bg-slate-800/60 text-slate-500 border border-slate-700/30 hover:text-slate-300 hover:border-slate-600'
            }`}
          >By Category</button>
          <button
            onClick={() => onChange({ ...config, mode: 'specific' })}
            className={`flex-1 px-4 py-2.5 rounded-lg text-sm font-medium transition-all duration-200 ${
              config.mode === 'specific'
                ? 'bg-omni-gold/15 text-omni-gold-light border border-omni-gold/30'
                : 'bg-slate-800/60 text-slate-500 border border-slate-700/30 hover:text-slate-300 hover:border-slate-600'
            }`}
          >Specific Token</button>
        </div>
      </div>

      {config.mode === 'category' ? (
        <AssetCategoryFilter
          selected={config.category}
          onSelect={(cat) => onChange({ ...config, category: cat })}
        />
      ) : (
        <div>
          <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">Token Symbol</label>
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
            <input
              type="text"
              value={config.specificToken}
              onChange={(e) => onChange({ ...config, specificToken: e.target.value.toUpperCase() })}
              placeholder="e.g. USDC, WETH, SOL..."
              className="input-field w-full pl-10 text-sm"
            />
          </div>
          <p className="text-[10px] text-slate-600 mt-1.5">Enter the exact token symbol to filter results</p>
        </div>
      )}
    </div>
  )
}

// ─── Shared: Dropdown Selector ───────────────────────────────────────────────

function DropdownSelector<T extends string>({
  label: fieldLabel,
  items,
  selected,
  onToggle,
  onSelectAll,
  onClear,
  renderIcon,
}: {
  label: string
  items: { value: T; label: string }[]
  selected: T[]
  onToggle: (v: T) => void
  onSelectAll: () => void
  onClear: () => void
  renderIcon?: (v: T) => React.ReactNode
}) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [open])
  const displayLabel = () => {
    if (selected.length === items.length) return `All ${fieldLabel}`
    if (selected.length === 0) return `Select ${fieldLabel.toLowerCase()}...`
    if (selected.length === 1) return items.find(i => i.value === selected[0])?.label ?? `1 ${fieldLabel.toLowerCase()}`
    return `${selected.length} ${fieldLabel.toLowerCase()}`
  }

  return (
    <div className="relative" ref={ref}>
      <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">{fieldLabel}</label>
      <button onClick={() => setOpen(o => !o)} className="input-field flex items-center justify-between w-full">
        <span className="text-sm">{displayLabel()}</span>
        <ChevronDown className={`w-4 h-4 text-slate-500 transition-transform ${open ? 'rotate-180' : ''}`} />
      </button>
      {open && (
        <div className="dropdown-menu max-h-80">
          <div className="p-2">
            <div className="flex gap-2 mb-2 pb-2 border-b border-slate-700/50">
              <button onClick={onSelectAll} className="flex-1 px-3 py-1.5 text-xs font-medium bg-omni-blue/10 text-omni-blue-light rounded-lg hover:bg-omni-blue/20 transition-colors">Select All</button>
              <button onClick={onClear} className="flex-1 px-3 py-1.5 text-xs font-medium bg-slate-700/50 text-slate-400 rounded-lg hover:bg-slate-700 transition-colors">Clear</button>
            </div>
            {items.map(item => (
              <label key={item.value} className="flex items-center px-3 py-2 hover:bg-slate-700/50 rounded-lg cursor-pointer transition-colors">
                <input type="checkbox" checked={selected.includes(item.value)} onChange={() => onToggle(item.value)} className="mr-3 w-3.5 h-3.5 text-omni-blue bg-slate-700 border-slate-600 rounded focus:ring-omni-blue" />
                {renderIcon?.(item.value)}
                <span className="text-sm text-slate-300">{item.label}</span>
              </label>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

// ═════════════════════════════════════════════════════════════════════════════
//  CARRY TRADE STRATEGY
// ═════════════════════════════════════════════════════════════════════════════

function CarryTradeStrategy() {
  const [currentStep, setCurrentStep] = useState<Step>(1)

  // Step 1
  const [selectedChains, setSelectedChains] = useState<Chain[]>(CHAINS.map(c => c.value))
  const [selectedProtocols, setSelectedProtocols] = useState<LendingProtocol[]>(LENDING_PROTOCOLS.map(p => p.value))

  // Step 2
  const [supplyConfig, setSupplyConfig] = useState<PairTokenConfig>({ mode: 'category', category: null, specificToken: '' })
  const [borrowConfig, setBorrowConfig] = useState<PairTokenConfig>({ mode: 'category', category: null, specificToken: '' })

  // Step 3 & 4
  const [pairs, setPairs] = useState<StrategyPair[]>([])
  const [selectedPairs, setSelectedPairs] = useState<StrategyPair[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const toParam = (arr: string[], total: number) =>
    arr.length > 0 && arr.length < total ? arr.join(',') : undefined

  const buildAssetParam = (config: PairTokenConfig) => {
    if (config.mode === 'specific' && config.specificToken.trim())
      return { assets: config.specificToken.trim(), asset_categories: undefined }
    return { assets: undefined, asset_categories: config.category || undefined }
  }

  const handleSearch = async () => {
    setLoading(true)
    setError(null)
    try {
      const supplyAsset = buildAssetParam(supplyConfig)
      const borrowAsset = buildAssetParam(borrowConfig)
      const [supplyData, borrowData] = await Promise.all([
        searchRates({
          action: 'supply',
          chains: toParam(selectedChains, CHAINS.length),
          protocols: toParam(selectedProtocols, LENDING_PROTOCOLS.length),
          asset_categories: supplyAsset.asset_categories as string | undefined,
          ...(supplyAsset.assets ? { assets: supplyAsset.assets } : {}),
        }),
        searchRates({
          action: 'borrow',
          chains: toParam(selectedChains, CHAINS.length),
          protocols: toParam(selectedProtocols, LENDING_PROTOCOLS.length),
          asset_categories: borrowAsset.asset_categories as string | undefined,
          ...(borrowAsset.assets ? { assets: borrowAsset.assets } : {}),
        }),
      ])
      const topSupply = supplyData.results.slice(0, 15)
      const topBorrow = borrowData.results.slice(0, 15)
      const combined: StrategyPair[] = []
      for (const s of topSupply) {
        for (const b of topBorrow) {
          combined.push({ supply: s, borrow: b, netApy: s.netApy - b.netApy })
        }
      }
      combined.sort((a, b) => b.netApy - a.netApy)
      setPairs(combined.slice(0, 30))
      setSelectedPairs([])
      setCurrentStep(3)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch rates')
    } finally {
      setLoading(false)
    }
  }

  const handleRestart = () => { setCurrentStep(1); setPairs([]); setSelectedPairs([]); setError(null) }

  const canAdvanceStep2 = () => {
    const supplyOk = supplyConfig.mode === 'category' || supplyConfig.specificToken.trim().length > 0
    const borrowOk = borrowConfig.mode === 'category' || borrowConfig.specificToken.trim().length > 0
    return supplyOk && borrowOk
  }

  return (
    <div className="space-y-6">
      {/* Stepper */}
      <StepIndicator current={currentStep} labels={CARRY_STEP_LABELS} />

      {error && <div className="card border border-red-500/20 bg-red-500/5 text-red-400 text-sm">{error}</div>}

      {/* Step 1: Chains & Protocols */}
      {currentStep === 1 && (
        <div className="space-y-6">
          <div className="card">
            <div className="flex items-center gap-3 mb-5">
              <div className="w-9 h-9 rounded-lg bg-omni-blue/10 border border-omni-blue/20 flex items-center justify-center">
                <Filter className="w-5 h-5 text-omni-blue" />
              </div>
              <div>
                <h3 className="text-lg font-semibold text-white">Chains & Protocols</h3>
                <p className="text-xs text-slate-500">Select where to look for carry-trade opportunities</p>
              </div>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <DropdownSelector
                label="Chains"
                items={CHAINS}
                selected={selectedChains}
                onToggle={c => setSelectedChains(prev => prev.includes(c) ? prev.filter(x => x !== c) : [...prev, c])}
                onSelectAll={() => setSelectedChains(CHAINS.map(c => c.value))}
                onClear={() => setSelectedChains([])}
                renderIcon={v => <ChainIcon chain={v} className="w-4 h-4 mr-2" />}
              />
              <DropdownSelector
                label="Protocols"
                items={LENDING_PROTOCOLS}
                selected={selectedProtocols}
                onToggle={p => setSelectedProtocols(prev => prev.includes(p) ? prev.filter(x => x !== p) : [...prev, p])}
                onSelectAll={() => setSelectedProtocols(LENDING_PROTOCOLS.map(p => p.value))}
                onClear={() => setSelectedProtocols([])}
                renderIcon={v => <ProtocolIcon protocol={v} className="w-4 h-4 mr-2" />}
              />
            </div>
            {/* Quick picks */}
            <div className="flex flex-wrap gap-2 mt-4 pt-4 border-t border-slate-700/30">
              <span className="text-xs text-slate-500 self-center mr-1">Quick:</span>
              {[
                { label: 'Ethereum Only', chains: ['ethereum'] as Chain[] },
                { label: 'Solana Only', chains: ['solana'] as Chain[] },
                { label: 'EVM Chains', chains: ['ethereum', 'arbitrum', 'base', 'polygon', 'optimism'] as Chain[] },
                { label: 'All Chains', chains: CHAINS.map(c => c.value) },
              ].map(q => (
                <button key={q.label} onClick={() => { setSelectedChains(q.chains); setSelectedProtocols(LENDING_PROTOCOLS.map(p => p.value)) }}
                  className="px-3 py-1 text-xs font-medium bg-slate-800/60 text-slate-400 border border-slate-700/30 rounded-lg hover:text-white hover:border-slate-600 transition-colors"
                >{q.label}</button>
              ))}
            </div>
          </div>
          <div className="flex justify-end">
            <button onClick={() => setCurrentStep(2)} disabled={selectedChains.length === 0 || selectedProtocols.length === 0}
              className="btn-primary flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed">
              Next: Asset Selection <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Step 2: Supply & Borrow asset config */}
      {currentStep === 2 && (
        <div className="space-y-6">
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <PairConfigPanel label="Supply (Pair 1)" subtitle="What asset do you want to earn yield on?"
              icon={<TrendingUp className="w-5 h-5 text-omni-blue" />} accentColor="border-omni-blue" accentBg="bg-omni-blue/10 border border-omni-blue/20"
              config={supplyConfig} onChange={setSupplyConfig} />
            <PairConfigPanel label="Borrow (Pair 2)" subtitle="What asset do you want to borrow against?"
              icon={<TrendingDown className="w-5 h-5 text-red-400" />} accentColor="border-red-500" accentBg="bg-red-500/10 border border-red-500/20"
              config={borrowConfig} onChange={setBorrowConfig} />
          </div>
          <StepNav onBack={() => setCurrentStep(1)} onNext={handleSearch} nextLabel="Find Pools" nextDisabled={loading || !canAdvanceStep2()} loading={loading} />
        </div>
      )}

      {/* Step 3: Select Pairs */}
      {currentStep === 3 && (
        <div className="space-y-6">
          <CarryPoolSelection pairs={pairs} selectedPairs={selectedPairs} setSelectedPairs={setSelectedPairs} />
          <StepNav onBack={() => setCurrentStep(2)} onNext={() => setCurrentStep(4)} nextLabel="Next: Simulate" nextDisabled={selectedPairs.length === 0} />
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
            <CarrySimulation pairs={selectedPairs} />
          </div>
          <StepNav onBack={() => setCurrentStep(3)} onNext={handleRestart} nextLabel="Start New Search" nextIcon={<RefreshCw className="w-4 h-4" />} backLabel="Back to Selection" />
        </div>
      )}
    </div>
  )
}

// ─── Carry: Pool Selection ───────────────────────────────────────────────────

function CarryPoolSelection({ pairs, selectedPairs, setSelectedPairs }: {
  pairs: StrategyPair[]; selectedPairs: StrategyPair[]; setSelectedPairs: (p: StrategyPair[]) => void
}) {
  const getPairId = (pair: StrategyPair) =>
    `${pair.supply.protocol}-${pair.supply.chain}-${pair.supply.asset}-${pair.supply.vaultName || 'd'}__${pair.borrow.protocol}-${pair.borrow.chain}-${pair.borrow.asset}-${pair.borrow.vaultName || 'd'}`

  return (
    <>
      <div className="card">
        <h3 className="text-lg font-semibold text-white mb-2">Select Strategies (up to 3)</h3>
        <p className="text-omni-silver text-sm">
          Found {pairs.length} viable carry-trade {pairs.length === 1 ? 'pair' : 'pairs'}.
          {selectedPairs.length > 0 && <span className="text-omni-gold ml-2">({selectedPairs.length} selected)</span>}
        </p>
      </div>
      {pairs.length === 0 ? (
        <div className="card text-center text-omni-silver py-12">No strategy pairs found. Try broadening your filters.</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {pairs.map((pair, i) => {
            const pairId = getPairId(pair)
            const isSelected = selectedPairs.some(p => getPairId(p) === pairId)
            const canSelect = selectedPairs.length < 3 || isSelected
            return (
              <button key={i}
                onClick={() => {
                  if (isSelected) setSelectedPairs(selectedPairs.filter(p => getPairId(p) !== pairId))
                  else if (canSelect) setSelectedPairs([...selectedPairs, pair])
                }}
                disabled={!canSelect && !isSelected}
                className={`text-left card transition-all border ${isSelected ? 'border-omni-gold bg-omni-gold/10 shadow-lg' : canSelect ? 'border-slate-700/50 hover:border-omni-gold' : 'border-slate-700/30 opacity-50 cursor-not-allowed'}`}
              >
                <div className="flex items-start gap-3">
                  <SelectionCheckbox checked={isSelected} />
                  <div className="flex-1">
                    <CarryPairCard pair={pair} />
                  </div>
                </div>
              </button>
            )
          })}
        </div>
      )}
    </>
  )
}

function CarryPairCard({ pair }: { pair: StrategyPair }) {
  const { supply, borrow, netApy } = pair
  return (
    <>
      <div className="flex items-center justify-between mb-4">
        <span className="text-xs font-medium text-omni-silver uppercase tracking-wider">NET APY</span>
        <span className={`text-2xl font-bold ${netApy >= 0 ? 'text-green-400' : 'text-red-400'}`}>{formatApy(netApy)}</span>
      </div>
      <RateRow label="Supply" color="omni-blue-light" icon={<TrendingUp className="w-3.5 h-3.5 text-omni-blue-light" />} rate={supply} valueColor="text-emerald-400" />
      <RateRow label="Borrow" color="omni-red-light" icon={<TrendingDown className="w-3.5 h-3.5 text-omni-red-light" />} rate={borrow} valueColor="text-red-400" />
    </>
  )
}

function RateRow({ label, color, icon, rate, valueColor }: {
  label: string; color: string; icon: React.ReactNode; rate: RateResult; valueColor: string
}) {
  return (
    <div className="bg-slate-800/40 border border-slate-700/20 rounded-xl p-3 mb-2 last:mb-0">
      <div className="flex items-center gap-1.5 mb-2">
        {icon}
        <span className={`text-[10px] font-semibold text-${color} uppercase tracking-wider`}>{label}</span>
      </div>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ProtocolIcon protocol={rate.protocol} className="w-5 h-5" />
          <div>
            <div className="text-sm font-medium text-white">{rate.asset}</div>
            <div className="flex items-center gap-1 mt-0.5">
              <ChainIcon chain={rate.chain} className="w-3 h-3" />
              <span className="text-[10px] text-slate-500 capitalize">{rate.chain}</span>
            </div>
          </div>
        </div>
        <div className="text-right">
          <div className={`text-sm font-bold font-mono ${valueColor}`}>{rate.netApy.toFixed(2)}%</div>
          <div className="text-[10px] text-slate-500">{formatNum(rate.liquidity)}</div>
        </div>
      </div>
    </div>
  )
}

// ─── Carry: Simulation ───────────────────────────────────────────────────────

function CarrySimulation({ pairs }: { pairs: StrategyPair[] }) {
  const minLtv = Math.min(...pairs.map(p => p.supply.collateralLtv || 0.7))
  const initBorrow = Math.round(minLtv * 70)
  const [supplyAmount, setSupplyAmount] = useState(1000)
  const [borrowPercent, setBorrowPercent] = useState(initBorrow)
  const [supplyInput, setSupplyInput] = useState('1000')
  const [borrowInput, setBorrowInput] = useState(initBorrow.toString())
  const maxBorrow = minLtv * 100
  const borrowAmount = (supplyAmount * borrowPercent) / 100

  const results = pairs.map(pair => {
    const supplyReturn = supplyAmount * (pair.supply.netApy / 100)
    const borrowCost = borrowAmount * (pair.borrow.netApy / 100)
    const netReturn = supplyReturn - borrowCost
    const netApy = supplyAmount > 0 ? (netReturn / supplyAmount) * 100 : 0
    return { pair, supplyReturn, borrowCost, netReturn, netApy }
  })
  const best = results.reduce((b, c) => c.netApy > b.netApy ? c : b, results[0])

  return (
    <div className="space-y-6">
      {/* Sliders */}
      <div className="card">
        <h4 className="text-base font-semibold text-white mb-5 flex items-center gap-2">
          <div className="w-8 h-8 rounded-lg bg-omni-gold/10 border border-omni-gold/20 flex items-center justify-center"><DollarSign className="w-4 h-4 text-omni-gold" /></div>
          Amount Simulation
        </h4>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
          <SliderControl label="Supply Amount" icon={<TrendingUp className="w-4 h-4 text-omni-blue" />} prefix="$" accent="blue"
            value={supplyAmount} input={supplyInput} min={100} max={100000} step={100}
            onSlider={v => { setSupplyAmount(v); setSupplyInput(v.toString()) }}
            onInput={v => { setSupplyInput(v); const n = parseFloat(v); if (!isNaN(n) && n >= 100 && n <= 100000) setSupplyAmount(n) }}
            onBlur={() => setSupplyInput(supplyAmount.toString())} />
          <SliderControl label="Borrow %" icon={<TrendingDown className="w-4 h-4 text-red-400" />} suffix="%" accent="blue"
            value={borrowPercent} input={borrowInput} min={0} max={maxBorrow} step={1}
            maxLabel={`Max: ${maxBorrow.toFixed(0)}% (LTV)`}
            onSlider={v => { setBorrowPercent(v); setBorrowInput(v.toString()) }}
            onInput={v => { setBorrowInput(v); const n = parseFloat(v); if (!isNaN(n) && n >= 0 && n <= maxBorrow) setBorrowPercent(n) }}
            onBlur={() => setBorrowInput(borrowPercent.toString())}
            extra={<div className="text-sm text-slate-400 mt-2">= <span className="font-semibold text-omni-gold-light font-mono">${borrowAmount.toFixed(0)}</span></div>} />
        </div>
      </div>

      {/* Strategy cards */}
      <div className="space-y-3">
        <h4 className="text-base font-semibold text-white">Strategy Results</h4>
        {results.map((r, idx) => {
          const isBest = r === best && pairs.length > 1
          return (
            <div key={idx} className={`card transition-all ${isBest ? 'border-2 border-omni-gold bg-gradient-to-r from-omni-gold/10 to-transparent' : 'border border-slate-700'}`}>
              <div className="flex items-start justify-between mb-3">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-medium text-omni-silver uppercase tracking-wider">Strategy {idx + 1}</span>
                  {isBest && <span className="px-2 py-0.5 bg-omni-gold text-slate-900 text-xs font-bold rounded">BEST</span>}
                </div>
                <div className="text-right">
                  <div className={`text-2xl font-bold ${r.netApy >= 0 ? 'text-green-400' : 'text-red-400'}`}>{r.netApy.toFixed(2)}%</div>
                  <div className="text-xs text-omni-silver">NET APY</div>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3 mb-3">
                <SimSideCard label="Supply" icon={<TrendingUp className="w-3 h-3 text-omni-blue-light" />} rate={r.pair.supply} valueColor="text-emerald-400" suffix="APY" />
                <SimSideCard label="Borrow" icon={<TrendingDown className="w-3 h-3 text-omni-red-light" />} rate={r.pair.borrow} valueColor="text-red-400" suffix="APR" />
              </div>
              <div className="grid grid-cols-4 gap-2 pt-3 border-t border-slate-700/30">
                <StatCell label="Supply" value={`$${supplyAmount.toLocaleString()}`} />
                <StatCell label="Borrow" value={`$${borrowAmount.toFixed(0)}`} />
                <StatCell label="Yearly Return" value={`$${r.netReturn.toFixed(2)}`} color={r.netReturn >= 0 ? 'text-green-400' : 'text-red-400'} />
                <StatCell label="Impact" value={r === best ? '—' : `${r.netApy > best.netApy ? '+' : ''}${(r.netApy - best.netApy).toFixed(2)}%`} />
              </div>
            </div>
          )
        })}
      </div>
      <div className="bg-omni-blue/5 border border-omni-blue/15 rounded-xl p-3 text-xs text-slate-400">
        <strong className="text-slate-300">Note:</strong> Min LTV {(minLtv * 100).toFixed(0)}%. Supply ${supplyAmount.toLocaleString()}, Borrow ${borrowAmount.toFixed(0)} ({borrowPercent}%).
      </div>
    </div>
  )
}

// ═════════════════════════════════════════════════════════════════════════════
//  LIQUIDITY POOL STRATEGY
// ═════════════════════════════════════════════════════════════════════════════

function PoolStrategy() {
  const [currentStep, setCurrentStep] = useState<Step>(1)

  // Step 1
  const [selectedChains, setSelectedChains] = useState<PoolChain[]>(POOL_CHAINS.map(c => c.value))
  const [selectedProtocols, setSelectedProtocols] = useState<PoolProtocol[]>(POOL_PROTOCOLS.map(p => p.value))

  // Step 2
  const [tokenAConfig, setTokenAConfig] = useState<PairTokenConfig>({ mode: 'category', category: null, specificToken: '' })
  const [tokenBConfig, setTokenBConfig] = useState<PairTokenConfig>({ mode: 'category', category: null, specificToken: '' })

  // Step 3 & 4
  const [pools, setPools] = useState<PoolResult[]>([])
  const [selectedPools, setSelectedPools] = useState<PoolResult[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleSearch = async () => {
    setLoading(true)
    setError(null)
    try {
      // Use high min_tvl to filter out low-quality pools, sort by TVL for stability
      const params: Record<string, string | number> = { min_tvl: 100000, sort_by: 'tvl' }

      if (selectedChains.length > 0 && selectedChains.length < POOL_CHAINS.length)
        params.chains = selectedChains.join(',')
      if (selectedProtocols.length > 0 && selectedProtocols.length < POOL_PROTOCOLS.length)
        params.protocols = selectedProtocols.join(',')

      // If both specific tokens, use pair filter
      const tokenA = tokenAConfig.mode === 'specific' ? tokenAConfig.specificToken.trim().toUpperCase() : ''
      const tokenB = tokenBConfig.mode === 'specific' ? tokenBConfig.specificToken.trim().toUpperCase() : ''
      const catA = tokenAConfig.mode === 'category' ? tokenAConfig.category : null
      const catB = tokenBConfig.mode === 'category' ? tokenBConfig.category : null

      if (tokenA && tokenB) {
        params.pair = `${tokenA}/${tokenB}`
      } else if (tokenA) {
        params.token = tokenA
      } else if (tokenB) {
        params.token = tokenB
      }

      // Send categories to narrow server-side results
      const cats: string[] = []
      if (catA) cats.push(catA)
      if (catB && !cats.includes(catB)) cats.push(catB)
      if (cats.length > 0) params.asset_categories = cats.join(',')

      const data = await searchPools(params)

      // ─── Client-side filtering for quality & relevance ───────────────
      let filtered = data.results

      // Filter by specific tokens
      if (tokenA) filtered = filtered.filter(p => p.token0.toUpperCase() === tokenA || p.token1.toUpperCase() === tokenA)
      if (tokenB) filtered = filtered.filter(p => p.token0.toUpperCase() === tokenB || p.token1.toUpperCase() === tokenB)

      // IMPORTANT: When both sides are categories, ensure BOTH tokens match
      // their respective categories (not just "any token matches any category")
      if (catA && catB) {
        filtered = filtered.filter(p => {
          const t0Cats = p.token0Categories
          const t1Cats = p.token1Categories
          // token0 matches catA AND token1 matches catB
          const matchAB = t0Cats.includes(catA) && t1Cats.includes(catB)
          // OR token0 matches catB AND token1 matches catA (reversed)
          const matchBA = t0Cats.includes(catB) && t1Cats.includes(catA)
          return matchAB || matchBA
        })
      } else if (catA && tokenB) {
        // One side is category, other is specific token - ensure the OTHER token matches the category
        filtered = filtered.filter(p => {
          const otherIsTok0 = p.token1.toUpperCase() === tokenB
          return otherIsTok0 ? p.token0Categories.includes(catA) : p.token1Categories.includes(catA)
        })
      } else if (catB && tokenA) {
        filtered = filtered.filter(p => {
          const otherIsTok0 = p.token1.toUpperCase() === tokenA
          return otherIsTok0 ? p.token0Categories.includes(catB) : p.token1Categories.includes(catB)
        })
      } else if (catA && !catB && !tokenB) {
        // Only catA selected, ensure at least one token matches
        filtered = filtered.filter(p => p.token0Categories.includes(catA) || p.token1Categories.includes(catA))
      } else if (catB && !catA && !tokenA) {
        filtered = filtered.filter(p => p.token0Categories.includes(catB) || p.token1Categories.includes(catB))
      }

      // Filter out unrealistic APRs (>500% 24h is almost always unsustainable meme pools)
      filtered = filtered.filter(p => p.feeApr7d < 500)

      // Sort: prefer pools with reasonable 7d APR and high TVL
      // Score = TVL weight + APR weight (balanced ranking)
      filtered.sort((a, b) => {
        // Primary: 7d fee APR (more stable than 24h)
        // Secondary: TVL as tiebreaker
        const scoreA = a.feeApr7d * 100 + Math.log10(Math.max(a.tvlUsd, 1))
        const scoreB = b.feeApr7d * 100 + Math.log10(Math.max(b.tvlUsd, 1))
        return scoreB - scoreA
      })

      setPools(filtered.slice(0, 30))
      setSelectedPools([])
      setCurrentStep(3)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch pools')
    } finally {
      setLoading(false)
    }
  }

  const handleRestart = () => { setCurrentStep(1); setPools([]); setSelectedPools([]); setError(null) }

  const canAdvanceStep2 = () => {
    const aOk = tokenAConfig.mode === 'category' || tokenAConfig.specificToken.trim().length > 0
    const bOk = tokenBConfig.mode === 'category' || tokenBConfig.specificToken.trim().length > 0
    return aOk && bOk
  }

  return (
    <div className="space-y-6">
      <StepIndicator current={currentStep} labels={POOL_STEP_LABELS} />

      {error && <div className="card border border-red-500/20 bg-red-500/5 text-red-400 text-sm">{error}</div>}

      {/* Step 1: Chains & Protocols */}
      {currentStep === 1 && (
        <div className="space-y-6">
          <div className="card">
            <div className="flex items-center gap-3 mb-5">
              <div className="w-9 h-9 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center">
                <Filter className="w-5 h-5 text-emerald-400" />
              </div>
              <div>
                <h3 className="text-lg font-semibold text-white">Chains & DEX Protocols</h3>
                <p className="text-xs text-slate-500">Select where to look for liquidity pools</p>
              </div>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <DropdownSelector
                label="Chains"
                items={POOL_CHAINS}
                selected={selectedChains}
                onToggle={c => setSelectedChains(prev => prev.includes(c) ? prev.filter(x => x !== c) : [...prev, c])}
                onSelectAll={() => setSelectedChains(POOL_CHAINS.map(c => c.value))}
                onClear={() => setSelectedChains([])}
                renderIcon={v => <ChainIcon chain={v} className="w-4 h-4 mr-2" />}
              />
              <DropdownSelector
                label="Protocols"
                items={POOL_PROTOCOLS}
                selected={selectedProtocols}
                onToggle={p => setSelectedProtocols(prev => prev.includes(p) ? prev.filter(x => x !== p) : [...prev, p])}
                onSelectAll={() => setSelectedProtocols(POOL_PROTOCOLS.map(p => p.value))}
                onClear={() => setSelectedProtocols([])}
                renderIcon={v => <ProtocolIcon protocol={v} className="w-4 h-4 mr-2" />}
              />
            </div>
            <div className="flex flex-wrap gap-2 mt-4 pt-4 border-t border-slate-700/30">
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
          </div>
          <div className="flex justify-end">
            <button onClick={() => setCurrentStep(2)} disabled={selectedChains.length === 0 || selectedProtocols.length === 0}
              className="btn-primary flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed">
              Next: Token Selection <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Step 2: Token A & Token B */}
      {currentStep === 2 && (
        <div className="space-y-6">
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <PairConfigPanel label="Token A" subtitle="First token of the liquidity pair"
              icon={<Droplets className="w-5 h-5 text-emerald-400" />} accentColor="border-emerald-500" accentBg="bg-emerald-500/10 border border-emerald-500/20"
              config={tokenAConfig} onChange={setTokenAConfig} />
            <PairConfigPanel label="Token B" subtitle="Second token of the liquidity pair"
              icon={<Droplets className="w-5 h-5 text-sky-400" />} accentColor="border-sky-500" accentBg="bg-sky-500/10 border border-sky-500/20"
              config={tokenBConfig} onChange={setTokenBConfig} />
          </div>
          <StepNav onBack={() => setCurrentStep(1)} onNext={handleSearch} nextLabel="Find Pools" nextDisabled={loading || !canAdvanceStep2()} loading={loading} />
        </div>
      )}

      {/* Step 3: Select Pools */}
      {currentStep === 3 && (
        <div className="space-y-6">
          <LPPoolSelection pools={pools} selectedPools={selectedPools} setSelectedPools={setSelectedPools} />
          <StepNav onBack={() => setCurrentStep(2)} onNext={() => setCurrentStep(4)} nextLabel="Next: Simulate" nextDisabled={selectedPools.length === 0} />
        </div>
      )}

      {/* Step 4: Simulation */}
      {currentStep === 4 && selectedPools.length > 0 && (
        <div className="space-y-6">
          <div className="card border-emerald-500 border-2">
            <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
              <Droplets className="w-5 h-5 text-emerald-400" />
              Pool Simulation ({selectedPools.length} {selectedPools.length === 1 ? 'Pool' : 'Pools'})
            </h3>
            <LPSimulation pools={selectedPools} />
          </div>
          <StepNav onBack={() => setCurrentStep(3)} onNext={handleRestart} nextLabel="Start New Search" nextIcon={<RefreshCw className="w-4 h-4" />} backLabel="Back to Selection" />
        </div>
      )}
    </div>
  )
}

// ─── LP: Pool Selection ──────────────────────────────────────────────────────

function LPPoolSelection({ pools, selectedPools, setSelectedPools }: {
  pools: PoolResult[]; selectedPools: PoolResult[]; setSelectedPools: (p: PoolResult[]) => void
}) {
  return (
    <>
      <div className="card">
        <h3 className="text-lg font-semibold text-white mb-2">Select Pools (up to 3)</h3>
        <p className="text-omni-silver text-sm">
          Found {pools.length} matching {pools.length === 1 ? 'pool' : 'pools'}.
          {selectedPools.length > 0 && <span className="text-emerald-400 ml-2">({selectedPools.length} selected)</span>}
        </p>
      </div>
      {pools.length === 0 ? (
        <div className="card text-center text-omni-silver py-12">No pools found. Try broadening your filters.</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {pools.map((pool, i) => {
            const isSelected = selectedPools.some(p => p.poolVaultId === pool.poolVaultId)
            const canSelect = selectedPools.length < 3 || isSelected
            return (
              <button key={i}
                onClick={() => {
                  if (isSelected) setSelectedPools(selectedPools.filter(p => p.poolVaultId !== pool.poolVaultId))
                  else if (canSelect) setSelectedPools([...selectedPools, pool])
                }}
                disabled={!canSelect && !isSelected}
                className={`text-left card transition-all border ${isSelected ? 'border-emerald-500 bg-emerald-500/10 shadow-lg' : canSelect ? 'border-slate-700/50 hover:border-emerald-500' : 'border-slate-700/30 opacity-50 cursor-not-allowed'}`}
              >
                <div className="flex items-start gap-3">
                  <SelectionCheckbox checked={isSelected} color="bg-emerald-500 border-emerald-500" />
                  <div className="flex-1">
                    <LPPoolCard pool={pool} />
                  </div>
                </div>
              </button>
            )
          })}
        </div>
      )}
    </>
  )
}

function LPPoolCard({ pool }: { pool: PoolResult }) {
  return (
    <>
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <ProtocolIcon protocol={pool.protocol} className="w-5 h-5" />
          <div>
            <div className="text-sm font-bold text-white">{pool.pair}</div>
            <div className="flex items-center gap-1.5 mt-0.5">
              <ChainIcon chain={pool.chain} className="w-3 h-3" />
              <span className="text-[10px] text-slate-500 capitalize">{pool.chain}</span>
              <span className="text-[10px] text-slate-600 mx-0.5">&middot;</span>
              <span className="text-[10px] text-slate-500">{pool.feeTier}</span>
              <span className={`text-[10px] px-1.5 py-0.5 rounded ${pool.poolType === 'concentrated' ? 'bg-purple-500/10 text-purple-400' : 'bg-slate-700/50 text-slate-400'}`}>
                {pool.poolType === 'concentrated' ? 'CLMM' : 'Standard'}
              </span>
            </div>
          </div>
        </div>
        <div className="text-right">
          <div className="text-xl font-bold text-emerald-400">{(pool.feeApr7d + pool.rewardsApr).toFixed(2)}%</div>
          <div className="text-[10px] text-slate-500">APR (7d)</div>
        </div>
      </div>

      <div className="grid grid-cols-4 gap-2 bg-slate-800/40 border border-slate-700/20 rounded-xl p-3">
        <div>
          <div className="text-[10px] text-slate-500 uppercase">TVL</div>
          <div className="text-sm font-semibold text-white">{formatNum(pool.tvlUsd)}</div>
        </div>
        <div>
          <div className="text-[10px] text-slate-500 uppercase">Vol 24h</div>
          <div className="text-sm font-semibold text-white">{formatNum(pool.volume24h)}</div>
        </div>
        <div>
          <div className="text-[10px] text-slate-500 uppercase">Fee 7d</div>
          <div className="text-sm font-semibold text-emerald-400">{pool.feeApr7d.toFixed(2)}%</div>
        </div>
        <div>
          <div className="text-[10px] text-slate-500 uppercase">Fee 24h</div>
          <div className="text-sm font-semibold text-slate-400 text-xs">{pool.feeApr24h.toFixed(2)}%</div>
        </div>
      </div>

      {pool.rewardsApr > 0 && (
        <div className="mt-2 flex items-center gap-1.5">
          <span className="text-[10px] px-2 py-0.5 bg-omni-gold/10 text-omni-gold-light rounded border border-omni-gold/20">
            +{pool.rewardsApr.toFixed(2)}% Rewards
          </span>
        </div>
      )}
    </>
  )
}

// ─── LP: Simulation ──────────────────────────────────────────────────────────

function LPSimulation({ pools }: { pools: PoolResult[] }) {
  const [depositAmount, setDepositAmount] = useState(1000)
  const [depositInput, setDepositInput] = useState('1000')

  const results = pools.map(pool => {
    // Use 7d APR for simulation (more stable/realistic than 24h)
    const feeReturn = depositAmount * (pool.feeApr7d / 100)
    const rewardReturn = depositAmount * (pool.rewardsApr / 100)
    const totalReturn = feeReturn + rewardReturn
    const totalApr = pool.feeApr7d + pool.rewardsApr
    return { pool, feeReturn, rewardReturn, totalReturn, totalApr }
  })
  const best = results.reduce((b, c) => c.totalReturn > b.totalReturn ? c : b, results[0])

  return (
    <div className="space-y-6">
      {/* Deposit slider */}
      <div className="card">
        <h4 className="text-base font-semibold text-white mb-5 flex items-center gap-2">
          <div className="w-8 h-8 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center">
            <DollarSign className="w-4 h-4 text-emerald-400" />
          </div>
          Deposit Simulation
        </h4>
        <SliderControl label="Deposit Amount" icon={<Droplets className="w-4 h-4 text-emerald-400" />} prefix="$" accent="emerald"
          value={depositAmount} input={depositInput} min={100} max={100000} step={100}
          onSlider={v => { setDepositAmount(v); setDepositInput(v.toString()) }}
          onInput={v => { setDepositInput(v); const n = parseFloat(v); if (!isNaN(n) && n >= 100 && n <= 100000) setDepositAmount(n) }}
          onBlur={() => setDepositInput(depositAmount.toString())} />
      </div>

      {/* Pool comparison */}
      <div className="space-y-3">
        <h4 className="text-base font-semibold text-white">Pool Results</h4>
        {results.map((r, idx) => {
          const isBest = r === best && pools.length > 1
          return (
            <div key={idx} className={`card transition-all ${isBest ? 'border-2 border-emerald-500 bg-gradient-to-r from-emerald-500/10 to-transparent' : 'border border-slate-700'}`}>
              <div className="flex items-start justify-between mb-3">
                <div className="flex items-center gap-2">
                  <ProtocolIcon protocol={r.pool.protocol} className="w-6 h-6" />
                  <div>
                    <div className="text-sm font-bold text-white flex items-center gap-2">
                      {r.pool.pair}
                      {isBest && <span className="px-2 py-0.5 bg-emerald-500 text-slate-900 text-xs font-bold rounded">BEST</span>}
                    </div>
                    <div className="flex items-center gap-1.5 mt-0.5">
                      <ChainIcon chain={r.pool.chain} className="w-3 h-3" />
                      <span className="text-[10px] text-slate-500 capitalize">{r.pool.chain}</span>
                      <span className="text-[10px] text-slate-600 mx-0.5">&middot;</span>
                      <span className="text-[10px] text-slate-500">{r.pool.feeTier}</span>
                      <span className={`text-[10px] px-1.5 py-0.5 rounded ${r.pool.poolType === 'concentrated' ? 'bg-purple-500/10 text-purple-400' : 'bg-slate-700/50 text-slate-400'}`}>
                        {r.pool.poolType === 'concentrated' ? 'CLMM' : 'Standard'}
                      </span>
                    </div>
                  </div>
                </div>
                <div className="text-right">
                  <div className="text-2xl font-bold text-emerald-400">{r.totalApr.toFixed(2)}%</div>
                  <div className="text-xs text-omni-silver">APR (7d)</div>
                </div>
              </div>

              <div className="grid grid-cols-3 gap-3 mb-3">
                <div className="bg-slate-800/40 border border-slate-700/20 rounded-xl p-3">
                  <div className="text-[10px] text-slate-500 uppercase tracking-wider mb-1">Fee APR (7d)</div>
                  <div className="text-lg font-bold font-mono text-emerald-400">{r.pool.feeApr7d.toFixed(2)}%</div>
                  <div className="text-xs text-slate-500 mt-1">24h: {r.pool.feeApr24h.toFixed(2)}%</div>
                </div>
                <div className="bg-slate-800/40 border border-slate-700/20 rounded-xl p-3">
                  <div className="text-[10px] text-slate-500 uppercase tracking-wider mb-1">Rewards APR</div>
                  <div className="text-lg font-bold font-mono text-omni-gold-light">{r.pool.rewardsApr.toFixed(2)}%</div>
                  <div className="text-xs text-slate-500 mt-1">Incentives</div>
                </div>
                <div className="bg-slate-800/40 border border-slate-700/20 rounded-xl p-3">
                  <div className="text-[10px] text-slate-500 uppercase tracking-wider mb-1">Turnover 24h</div>
                  <div className="text-lg font-bold font-mono text-sky-400">{r.pool.turnoverRatio24h < 1 ? `${(r.pool.turnoverRatio24h * 100).toFixed(1)}%` : `${r.pool.turnoverRatio24h.toFixed(2)}x`}</div>
                  <div className="text-xs text-slate-500 mt-1">Vol/TVL</div>
                </div>
              </div>

              <div className="grid grid-cols-5 gap-2 pt-3 border-t border-slate-700/30">
                <StatCell label="Deposit" value={`$${depositAmount.toLocaleString()}`} />
                <StatCell label="TVL" value={formatNum(r.pool.tvlUsd)} />
                <StatCell label="Fee Yield" value={`$${r.feeReturn.toFixed(2)}`} color="text-emerald-400" />
                <StatCell label="Reward Yield" value={`$${r.rewardReturn.toFixed(2)}`} color="text-omni-gold-light" />
                <StatCell label="Total Yearly" value={`$${r.totalReturn.toFixed(2)}`} color="text-green-400" />
              </div>
            </div>
          )
        })}
      </div>

      <div className="bg-emerald-500/5 border border-emerald-500/15 rounded-xl p-3 text-xs text-slate-400">
        <strong className="text-slate-300">Note:</strong> Simulation uses 7-day average fee APR (more stable than 24h spikes).
        Actual returns depend on price range, impermanent loss, and market conditions.
        Rewards are subject to program duration and may change. Pools with &gt;500% 7d APR are excluded as likely unsustainable.
      </div>
    </div>
  )
}

// ═════════════════════════════════════════════════════════════════════════════
//  SHARED UI COMPONENTS
// ═════════════════════════════════════════════════════════════════════════════

function StepIndicator({ current, labels }: { current: Step; labels: Record<Step, string> }) {
  return (
    <div className="flex items-center justify-center gap-1">
      {([1, 2, 3, 4] as Step[]).map(step => (
        <div key={step} className="flex items-center">
          <div className="flex flex-col items-center">
            <div className={`w-8 h-8 rounded-lg flex items-center justify-center text-xs font-semibold transition-all duration-200 ${
              current === step ? 'bg-omni-gold text-slate-900 shadow-lg shadow-omni-gold/20' :
              current > step ? 'bg-emerald-500/15 text-emerald-400 border border-emerald-500/20' :
              'bg-slate-800/60 text-slate-500 border border-slate-700/30'
            }`}>
              {current > step ? (
                <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" /></svg>
              ) : step}
            </div>
            <span className={`text-[9px] mt-1 whitespace-nowrap ${current === step ? 'text-omni-gold' : current > step ? 'text-emerald-400/70' : 'text-slate-600'}`}>
              {labels[step]}
            </span>
          </div>
          {step < 4 && <div className={`w-8 h-px mx-1 ${current > step ? 'bg-emerald-500/30' : 'bg-slate-700/50'}`} />}
        </div>
      ))}
    </div>
  )
}

function StepNav({ onBack, onNext, nextLabel, nextDisabled, loading, backLabel, nextIcon }: {
  onBack: () => void; onNext: () => void; nextLabel: string; nextDisabled?: boolean; loading?: boolean; backLabel?: string; nextIcon?: React.ReactNode
}) {
  return (
    <div className="flex justify-between">
      <button onClick={onBack} className="btn-secondary flex items-center gap-2">
        <ArrowLeft className="w-4 h-4" />
        {backLabel || 'Back'}
      </button>
      <button onClick={onNext} disabled={nextDisabled || loading} className="btn-primary flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed">
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
            {nextLabel}
            {nextIcon || <ArrowRight className="w-4 h-4" />}
          </>
        )}
      </button>
    </div>
  )
}

function SelectionCheckbox({ checked, color }: { checked: boolean; color?: string }) {
  const activeColor = color || 'bg-omni-gold border-omni-gold'
  return (
    <div className={`w-5 h-5 rounded border-2 flex items-center justify-center flex-shrink-0 mt-0.5 ${checked ? activeColor : 'border-slate-500'}`}>
      {checked && (
        <svg className="w-3 h-3 text-slate-900" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
        </svg>
      )}
    </div>
  )
}

function SimSideCard({ label, icon, rate, valueColor, suffix }: {
  label: string; icon: React.ReactNode; rate: RateResult; valueColor: string; suffix: string
}) {
  return (
    <div className="bg-slate-800/40 border border-slate-700/20 rounded-xl p-3">
      <div className="flex items-center gap-1.5 mb-2">
        {icon}
        <span className="text-[10px] text-slate-500 uppercase tracking-wider">{label}</span>
      </div>
      <div className="flex items-center gap-2 mb-2">
        <ProtocolIcon protocol={rate.protocol} className="w-5 h-5" />
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-white">{rate.asset}</div>
          <div className="flex items-center gap-1 text-xs text-omni-silver">
            <ChainIcon chain={rate.chain} className="w-3 h-3" />
            <span className="capitalize">{rate.chain}</span>
          </div>
        </div>
      </div>
      <div className={`${valueColor} font-semibold font-mono text-sm`}>{rate.netApy.toFixed(2)}% {suffix}</div>
    </div>
  )
}

function StatCell({ label, value, color }: { label: string; value: string; color?: string }) {
  return (
    <div>
      <div className="text-xs text-omni-silver">{label}</div>
      <div className={`text-sm font-semibold ${color || 'text-white'}`}>{value}</div>
    </div>
  )
}

function SliderControl({ label, icon, prefix, suffix, value, input, min, max, step, maxLabel, onSlider, onInput, onBlur, extra, accent }: {
  label: string; icon: React.ReactNode; prefix?: string; suffix?: string
  value: number; input: string; min: number; max: number; step: number; maxLabel?: string
  onSlider: (v: number) => void; onInput: (v: string) => void; onBlur: () => void; extra?: React.ReactNode
  accent?: 'blue' | 'emerald'
}) {
  const ringColor = accent === 'emerald' ? 'focus:ring-emerald-500/50' : 'focus:ring-omni-blue/50'
  const sliderClass = accent === 'emerald' ? 'w-full slider-emerald' : 'w-full slider-blue'
  return (
    <div className="bg-slate-800/40 border border-slate-700/30 rounded-xl p-4">
      <div className="flex items-center justify-between mb-3">
        <label className="text-sm font-medium text-slate-300 flex items-center gap-2">{icon} {label}</label>
        <div className="relative">
          {prefix && <span className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-500 text-sm">{prefix}</span>}
          <input type="text" value={input} onChange={e => onInput(e.target.value)} onBlur={onBlur}
            className={`w-28 ${prefix ? 'pl-7' : 'pl-3'} ${suffix ? 'pr-7' : 'pr-3'} py-1.5 bg-slate-800/80 border border-slate-600/50 rounded-lg text-white text-right text-sm font-mono focus:outline-none focus:ring-2 ${ringColor}`} />
          {suffix && <span className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-500 text-sm">{suffix}</span>}
        </div>
      </div>
      <input type="range" min={min} max={max} step={step} value={value} onChange={e => onSlider(Number(e.target.value))} className={sliderClass} />
      <div className="flex justify-between text-[10px] text-slate-400 mt-1.5">
        <span>{prefix}{min}</span><span>{maxLabel || `${prefix || ''}${max.toLocaleString()}${suffix || ''}`}</span>
      </div>
      {extra}
    </div>
  )
}

// ═════════════════════════════════════════════════════════════════════════════
//  MAIN EXPORT
// ═════════════════════════════════════════════════════════════════════════════

export default function StrategyBuilder() {
  const [strategyType, setStrategyType] = useState<StrategyType | null>(null)

  return (
    <div className="space-y-8" id="strategy">
      {/* Header */}
      <div className="card">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-omni-gold/10 border border-omni-gold/20 flex items-center justify-center">
            <Zap className="w-5 h-5 text-omni-gold" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-white">Strategy Builder</h2>
            <p className="text-slate-500 text-xs">Build and simulate DeFi strategies step by step</p>
          </div>
        </div>
      </div>

      {/* Step 0: Strategy Selection */}
      {strategyType === null ? (
        <div className="animate-fade-in space-y-4">
          <div className="text-center mb-2">
            <h3 className="text-lg font-semibold text-white">Choose a Strategy Type</h3>
            <p className="text-sm text-slate-500 mt-1">Select the DeFi strategy you want to build</p>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {/* Carry Trade Card */}
            <button
              onClick={() => setStrategyType('carry')}
              className="card group text-left border border-slate-700/30 hover:border-omni-gold/40 hover:bg-omni-gold/[0.03] transition-all duration-200 cursor-pointer"
            >
              <div className="flex items-center gap-3 mb-3">
                <div className="w-11 h-11 rounded-xl bg-omni-gold/10 border border-omni-gold/20 flex items-center justify-center group-hover:bg-omni-gold/20 transition-colors">
                  <Repeat className="w-5 h-5 text-omni-gold" />
                </div>
                <div>
                  <h4 className="text-base font-semibold text-white group-hover:text-omni-gold-light transition-colors">Carry Trade</h4>
                  <p className="text-[11px] text-slate-500">Lending & Borrowing</p>
                </div>
              </div>
              <p className="text-sm text-slate-400 leading-relaxed">
                Supply assets at high yield and borrow at low rates across protocols and chains to capture the spread.
              </p>
              <div className="flex items-center gap-1.5 mt-4 text-xs text-omni-gold/70 font-medium">
                <span>Get started</span>
                <ArrowRight className="w-3.5 h-3.5 group-hover:translate-x-0.5 transition-transform" />
              </div>
            </button>

            {/* Liquidity Pool Card */}
            <button
              onClick={() => setStrategyType('pool')}
              className="card group text-left border border-slate-700/30 hover:border-emerald-500/40 hover:bg-emerald-500/[0.03] transition-all duration-200 cursor-pointer"
            >
              <div className="flex items-center gap-3 mb-3">
                <div className="w-11 h-11 rounded-xl bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center group-hover:bg-emerald-500/20 transition-colors">
                  <Droplets className="w-5 h-5 text-emerald-400" />
                </div>
                <div>
                  <h4 className="text-base font-semibold text-white group-hover:text-emerald-300 transition-colors">Liquidity Pool</h4>
                  <p className="text-[11px] text-slate-500">AMM & DEX Pools</p>
                </div>
              </div>
              <p className="text-sm text-slate-400 leading-relaxed">
                Provide liquidity to DEX pools and earn trading fees plus incentive rewards from token pairs.
              </p>
              <div className="flex items-center gap-1.5 mt-4 text-xs text-emerald-400/70 font-medium">
                <span>Get started</span>
                <ArrowRight className="w-3.5 h-3.5 group-hover:translate-x-0.5 transition-transform" />
              </div>
            </button>
          </div>
        </div>
      ) : (
        /* Active strategy with back option */
        <div className="animate-fade-in space-y-6">
          <button
            onClick={() => setStrategyType(null)}
            className="btn-secondary flex items-center gap-2 text-sm"
          >
            <ArrowLeft className="w-4 h-4" />
            Change Strategy
          </button>
          {strategyType === 'carry' ? <CarryTradeStrategy /> : <PoolStrategy />}
        </div>
      )}
    </div>
  )
}
