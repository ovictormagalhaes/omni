import Logo from './Logo'
import { Menu, X } from 'lucide-react'
import { useState } from 'react'

export default function Header() {
  const [mobileOpen, setMobileOpen] = useState(false)

  return (
    <header className="sticky top-0 z-50 glass border-b border-slate-700/30">
      <div className="container mx-auto px-4 sm:px-6">
        <div className="flex items-center justify-between h-16">
          <a href="/" className="hover:opacity-90 transition-opacity">
            <Logo size="small" />
          </a>

          {/* Desktop nav */}
          <nav className="hidden md:flex items-center gap-1">
            <a href="#rates" className="px-4 py-2 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all">
              Rates
            </a>
            <a href="#strategy" className="px-4 py-2 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all">
              Strategy
            </a>
            <a href="#docs" className="px-4 py-2 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all">
              Docs
            </a>
            <div className="w-px h-5 bg-slate-700 mx-2" />
            <a
              href="#rates"
              className="ml-1 px-4 py-2 text-sm font-medium bg-omni-blue/10 text-omni-blue-light border border-omni-blue/20 rounded-lg hover:bg-omni-blue/20 transition-all"
            >
              Launch App
            </a>
          </nav>

          {/* Mobile toggle */}
          <button
            onClick={() => setMobileOpen(!mobileOpen)}
            className="md:hidden p-2 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all"
          >
            {mobileOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
          </button>
        </div>

        {/* Mobile nav */}
        {mobileOpen && (
          <nav className="md:hidden pb-4 flex flex-col gap-1 animate-fade-in">
            <a href="#rates" onClick={() => setMobileOpen(false)} className="px-4 py-2.5 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all">
              Rates
            </a>
            <a href="#strategy" onClick={() => setMobileOpen(false)} className="px-4 py-2.5 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all">
              Strategy
            </a>
            <a href="#docs" onClick={() => setMobileOpen(false)} className="px-4 py-2.5 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all">
              Docs
            </a>
            <a
              href="#rates"
              onClick={() => setMobileOpen(false)}
              className="mt-2 px-4 py-2.5 text-sm font-medium bg-omni-blue/10 text-omni-blue-light border border-omni-blue/20 rounded-lg text-center"
            >
              Launch App
            </a>
          </nav>
        )}
      </div>
    </header>
  )
}
