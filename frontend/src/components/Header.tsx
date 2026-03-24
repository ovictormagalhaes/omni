import Logo from './Logo'

export default function Header() {
  return (
    <header className="sticky top-0 z-50 backdrop-blur-lg bg-omni-navy/80 border-b border-slate-800">
      <div className="container mx-auto px-4">
        <div className="flex items-center justify-between h-20">
          {/* Logo */}
          <a href="/" className="hover:opacity-80 transition-opacity">
            <Logo size="medium" />
          </a>

          {/* Navigation */}
          <nav className="hidden md:flex items-center space-x-8">
            <a href="#rates" className="text-omni-silver hover:text-white transition-colors">
              Rates
            </a>
            <a href="#strategy" className="text-omni-silver hover:text-white transition-colors">
              Strategy
            </a>
            <a href="#docs" className="text-omni-silver hover:text-white transition-colors">
              Docs
            </a>
          </nav>
        </div>
      </div>
    </header>
  )
}
