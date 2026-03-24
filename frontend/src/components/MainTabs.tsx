import { useState } from 'react'
import { Search, Zap } from 'lucide-react'
import RateFinder from './RateFinder'
import StrategyBuilder from './StrategyBuilder'

type Tab = 'rates' | 'strategy'

export default function MainTabs() {
  const [activeTab, setActiveTab] = useState<Tab>('rates')

  return (
    <div id="rates">
      {/* Tab bar */}
      <div className="flex gap-1 bg-slate-800/60 p-1 rounded-xl w-fit mb-8">
        <button
          onClick={() => setActiveTab('rates')}
          className={`flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-medium transition-all ${
            activeTab === 'rates'
              ? 'bg-omni-blue text-white shadow'
              : 'text-omni-silver hover:text-white'
          }`}
        >
          <Search className="w-4 h-4" />
          Rate Finder
        </button>
        <button
          onClick={() => setActiveTab('strategy')}
          id="strategy"
          className={`flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-medium transition-all ${
            activeTab === 'strategy'
              ? 'bg-omni-gold text-omni-navy shadow'
              : 'text-omni-silver hover:text-white'
          }`}
        >
          <Zap className="w-4 h-4" />
          Strategy Builder
        </button>
      </div>

      {/* Content */}
      {activeTab === 'rates' && <RateFinder />}
      {activeTab === 'strategy' && <StrategyBuilder />}
    </div>
  )
}
