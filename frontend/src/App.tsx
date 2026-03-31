import { useState, useRef, useCallback } from 'react'
import Header from './components/Header'
import Hero from './components/Hero'
import MainTabs from './components/MainTabs'
import Footer from './components/Footer'

export type Tab = 'rates' | 'strategy' | 'pools'

export default function App() {
  const [activeTab, setActiveTab] = useState<Tab>('rates')
  const tabsRef = useRef<HTMLDivElement>(null)

  const navigateTo = useCallback((tab: Tab) => {
    setActiveTab(tab)
    tabsRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }, [])

  return (
    <div className="min-h-screen bg-omni-navy flex flex-col">
      <Header onNavigate={navigateTo} />
      <main className="flex-1">
        <Hero onNavigate={navigateTo} />
        <div ref={tabsRef} className="container mx-auto px-4 sm:px-6 pb-16 scroll-mt-20">
          <MainTabs activeTab={activeTab} onTabChange={setActiveTab} />
        </div>
      </main>
      <Footer onNavigate={navigateTo} />
    </div>
  )
}
