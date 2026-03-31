import type { Tab } from '../App'

interface HeroProps {
  onNavigate: (tab: Tab) => void
}

export default function Hero({ onNavigate }: HeroProps) {
  return (
    <section className="relative overflow-hidden pt-16 pb-20 md:pt-24 md:pb-28">
      {/* Background gradient orbs */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute top-[-10%] left-[10%] w-[500px] h-[500px] bg-omni-blue/20 rounded-full blur-[120px] animate-pulse-slow" />
        <div className="absolute bottom-[-10%] right-[10%] w-[400px] h-[400px] bg-purple-600/15 rounded-full blur-[120px] animate-pulse-slow" style={{ animationDelay: '1.5s' }} />
        <div className="absolute top-[30%] right-[25%] w-[300px] h-[300px] bg-omni-red/10 rounded-full blur-[100px] animate-pulse-slow" style={{ animationDelay: '3s' }} />
      </div>

      {/* Grid pattern overlay */}
      <div
        className="absolute inset-0 opacity-[0.03]"
        style={{
          backgroundImage: 'radial-gradient(circle at 1px 1px, rgba(248,250,252,0.5) 1px, transparent 0)',
          backgroundSize: '40px 40px',
        }}
      />

      <div className="container mx-auto px-4 sm:px-6 relative z-10">
        <div className="max-w-3xl mx-auto text-center">
          {/* Tag */}
          <div className="inline-flex items-center gap-2 px-3 py-1.5 mb-6 rounded-full border border-slate-700/50 bg-slate-800/40 text-xs text-slate-400 animate-fade-in">
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
            DeFi Yield Intelligence
          </div>

          <h1 className="text-4xl sm:text-5xl md:text-6xl lg:text-7xl font-bold mb-6 tracking-tight leading-[1.1] animate-slide-up">
            <span className="text-white">Find the best</span>
            <br />
            <span className="bg-gradient-to-r from-omni-blue via-purple-500 to-omni-red bg-clip-text text-transparent">
              DeFi yields
            </span>
          </h1>

          <p className="text-base sm:text-lg md:text-xl text-slate-400 mb-10 max-w-xl mx-auto leading-relaxed animate-slide-up" style={{ animationDelay: '0.1s' }}>
            Compare DeFi yields across{' '}
            <span className="text-slate-200 font-medium">40+ protocols</span> and{' '}
            <span className="text-slate-200 font-medium">20+ chains</span> in real time.
          </p>

          <div className="flex flex-col sm:flex-row items-center justify-center gap-3 animate-slide-up" style={{ animationDelay: '0.2s' }}>
            <button onClick={() => onNavigate('rates')} className="btn-primary w-full sm:w-auto text-center">
              Earn Finder
            </button>
            <button onClick={() => onNavigate('pools')} className="btn-secondary w-full sm:w-auto text-center">
              Liquidity Finder
            </button>
            <button onClick={() => onNavigate('strategy')} className="btn-secondary w-full sm:w-auto text-center">
              Strategy Builder
            </button>
          </div>

          {/* Stats */}
          <div className="grid grid-cols-3 gap-6 mt-16 max-w-md mx-auto animate-slide-up" style={{ animationDelay: '0.3s' }}>
            {[
              { value: '40+', label: 'Protocols' },
              { value: '20+', label: 'Chains' },
              { value: '24/7', label: 'Monitoring' },
            ].map((stat) => (
              <div key={stat.label} className="text-center">
                <div className="text-2xl sm:text-3xl font-bold text-white mb-1">{stat.value}</div>
                <div className="text-xs text-slate-500 uppercase tracking-wider">{stat.label}</div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Bottom fade */}
      <div className="absolute bottom-0 left-0 right-0 h-24 bg-gradient-to-t from-omni-navy to-transparent" />
    </section>
  )
}
