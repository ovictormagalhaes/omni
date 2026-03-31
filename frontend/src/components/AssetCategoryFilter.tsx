import { Info } from 'lucide-react'

export type AssetCategory = 'usd-correlated' | 'stablecoin' | 'btc-correlated' | 'eth-correlated' | 'sol-correlated' | 'other'

export const ASSET_CATEGORIES: { value: AssetCategory; label: string; description: string }[] = [
  { value: 'usd-correlated', label: 'USD', description: 'Assets pegged or correlated to the US Dollar, including tokenized treasuries and yield-bearing USD products (e.g. USDC, USDT, sDAI)' },
  { value: 'stablecoin', label: 'Stablecoin', description: 'Traditional stablecoins that maintain a 1:1 peg to fiat currencies (e.g. USDC, USDT, DAI)' },
  { value: 'btc-correlated', label: 'BTC', description: 'Bitcoin and wrapped/synthetic Bitcoin assets that track BTC price (e.g. WBTC, tBTC, cbBTC)' },
  { value: 'eth-correlated', label: 'ETH', description: 'Ether and liquid staking derivatives that track ETH price (e.g. ETH, wstETH, rETH, cbETH)' },
  { value: 'sol-correlated', label: 'SOL', description: 'Solana and liquid staking tokens that track SOL price (e.g. SOL, mSOL, jitoSOL, bSOL)' },
  { value: 'other', label: 'Other', description: 'Assets that don\'t fit into the above categories, including altcoins, governance tokens, and other DeFi tokens' },
]

interface AssetCategoryFilterProps {
  selected: AssetCategory | null
  onSelect: (category: AssetCategory | null) => void
  label?: string
}

export default function AssetCategoryFilter({ selected, onSelect, label = 'Asset Category' }: AssetCategoryFilterProps) {
  return (
    <div>
      {label && <label className="block text-xs font-medium text-slate-500 uppercase tracking-wider mb-2">{label}</label>}
      <div className="flex flex-wrap gap-2">
        {ASSET_CATEGORIES.map((category) => (
          <div key={category.value} className="relative group/tooltip">
            <button
              onClick={() => onSelect(selected === category.value ? null : category.value)}
              className={`flex items-center gap-1.5 px-3.5 py-2 rounded-lg text-sm font-medium transition-all duration-200 ${
                selected === category.value
                  ? 'bg-omni-gold/15 text-omni-gold-light border border-omni-gold/30'
                  : 'bg-slate-800/60 text-slate-500 border border-slate-700/30 hover:text-slate-300 hover:border-slate-600'
              }`}
            >
              {category.label}
              <Info className="w-3 h-3 opacity-40 group-hover/tooltip:opacity-70 transition-opacity" />
            </button>
            <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-xs text-slate-300 w-56 opacity-0 invisible group-hover/tooltip:opacity-100 group-hover/tooltip:visible transition-all duration-200 z-50 pointer-events-none shadow-xl">
              {category.description}
              <div className="absolute top-full left-1/2 -translate-x-1/2 -mt-px w-2 h-2 bg-slate-900 border-r border-b border-slate-700 rotate-45" />
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
