import Header from './components/Header'
import Hero from './components/Hero'
import MainTabs from './components/MainTabs'

export default function App() {
  return (
    <div className="min-h-screen bg-omni-navy">
      <main>
        <Header />
        <Hero />
        <div className="container mx-auto px-4 py-12">
          <MainTabs />
        </div>
      </main>
    </div>
  )
}
