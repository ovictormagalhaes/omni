import Logo from './Logo'

export default function Footer() {
  return (
    <footer className="border-t border-slate-800/30 mt-auto">
      <div className="container mx-auto px-4 sm:px-6 py-8">
        <div className="flex flex-col md:flex-row items-center justify-between gap-6">
          <div className="flex items-center gap-3">
            <Logo size="small" iconOnly />
            <div>
              <p className="text-sm font-semibold text-slate-300">OMNI Protocol</p>
              <p className="text-[11px] text-slate-600">Omnichain Lending Intelligence</p>
            </div>
          </div>

          <div className="flex items-center gap-6">
            <a href="#rates" className="text-xs text-slate-500 hover:text-slate-300 transition-colors">Rates</a>
            <a href="#strategy" className="text-xs text-slate-500 hover:text-slate-300 transition-colors">Strategy</a>
            <a href="#docs" className="text-xs text-slate-500 hover:text-slate-300 transition-colors">Docs</a>
          </div>

          <p className="text-[10px] text-slate-700">
            Data is informational only. DYOR.
          </p>
        </div>
      </div>
    </footer>
  )
}
