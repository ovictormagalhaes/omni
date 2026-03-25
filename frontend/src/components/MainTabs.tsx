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
      <div className="flex gap-1 p-1 bg-slate-800/40 border border-slate-700/30 rounded-xl w-fit mb-8">
        <button
          onClick={() => setActiveTab('rates')}
          className={`flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-medium transition-all duration-200 ${
            activeTab === 'rates'
              ? 'bg-omni-blue text-white shadow-lg shadow-omni-blue/20'
              : 'text-slate-400 hover:text-white hover:bg-slate-700/50'
          }`}
        >
          <Search className="w-4 h-4" />
          Rate Finder
        </button>
        <button
          onClick={() => setActiveTab('strategy')}
          id="strategy"
          className={`flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-medium transition-all duration-200 ${
            activeTab === 'strategy'
              ? 'bg-omni-gold text-omni-navy shadow-lg shadow-omni-gold/20'
              : 'text-slate-400 hover:text-white hover:bg-slate-700/50'
          }`}
        >
          <Zap className="w-4 h-4" />
          Strategy Builder
        </button>
      </div>

      {/* Content */}
      <div className="animate-fade-in">
        {activeTab === 'rates' && <RateFinder />}
        {activeTab === 'strategy' && <StrategyBuilder />}
      </div>
    </div>
  )
}
