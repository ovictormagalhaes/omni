import Logo from './Logo'
import { Menu, X } from 'lucide-react'
import { useState } from 'react'
import type { Tab } from '../App'

interface HeaderProps {
  onNavigate: (tab: Tab) => void
}

export default function Header({ onNavigate }: HeaderProps) {
  const [mobileOpen, setMobileOpen] = useState(false)

  const handleNav = (tab: Tab) => {
    onNavigate(tab)
    setMobileOpen(false)
  }

  return (
    <header className="sticky top-0 z-50 glass border-b border-slate-700/30">
      <div className="container mx-auto px-4 sm:px-6">
        <div className="flex items-center justify-between h-16">
          <a href="/" className="hover:opacity-90 transition-opacity">
            <Logo size="small" />
          </a>

          {/* Desktop nav */}
          <nav className="hidden md:flex items-center gap-1">
            <button onClick={() => handleNav('rates')} className="px-4 py-2 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all">
              Earn
            </button>
            <button onClick={() => handleNav('pools')} className="px-4 py-2 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all">
              Liquidity
            </button>
            <button onClick={() => handleNav('strategy')} className="px-4 py-2 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all">
              Strategy
            </button>
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
            <button onClick={() => handleNav('rates')} className="px-4 py-2.5 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all text-left">
              Earn
            </button>
            <button onClick={() => handleNav('pools')} className="px-4 py-2.5 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all text-left">
              Liquidity
            </button>
            <button onClick={() => handleNav('strategy')} className="px-4 py-2.5 text-sm text-slate-400 hover:text-white rounded-lg hover:bg-slate-800/60 transition-all text-left">
              Strategy
            </button>
          </nav>
        )}
      </div>
    </header>
  )
}
