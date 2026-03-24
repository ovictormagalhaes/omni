export default function Hero() {
  return (
    <section className="relative overflow-hidden py-20 md:py-32">
      {/* Background gradient */}
      <div className="absolute inset-0 bg-gradient-hero opacity-10"></div>
      
      {/* Animated circles */}
      <div className="absolute top-0 left-1/4 w-96 h-96 bg-omni-blue rounded-full blur-3xl opacity-20 animate-pulse-slow"></div>
      <div className="absolute bottom-0 right-1/4 w-96 h-96 bg-omni-red rounded-full blur-3xl opacity-20 animate-pulse-slow" style={{ animationDelay: '1s' }}></div>

      <div className="container mx-auto px-4 relative z-10">
        <div className="max-w-4xl mx-auto text-center">
          <h1 className="text-5xl md:text-7xl font-bold mb-6">
            <span className="bg-gradient-to-r from-omni-blue via-omni-red to-omni-gold bg-clip-text text-transparent">
              Omnichain Intelligence
            </span>
          </h1>
          
          <p className="text-xl md:text-2xl text-omni-silver mb-8 leading-relaxed">
            Find the best lending and borrowing rates across <span className="text-white font-semibold">Aave</span>, <span className="text-white font-semibold">Kamino</span>, and beyond.
            <br />
            Save time. Maximize yield.
          </p>

          <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
            <a href="#rates" className="btn-primary w-full sm:w-auto">
              Explore Rates
            </a>
            <button className="btn-secondary w-full sm:w-auto">
              View Docs
            </button>
          </div>

          {/* Stats */}
          <div className="grid grid-cols-3 gap-8 mt-16 max-w-2xl mx-auto">
            <div>
              <div className="text-3xl md:text-4xl font-bold text-white mb-2">2</div>
              <div className="text-sm text-omni-silver">Protocols</div>
            </div>
            <div>
              <div className="text-3xl md:text-4xl font-bold text-white mb-2">3</div>
              <div className="text-sm text-omni-silver">Chains</div>
            </div>
            <div>
              <div className="text-3xl md:text-4xl font-bold text-white mb-2">6</div>
              <div className="text-sm text-omni-silver">Assets</div>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}
