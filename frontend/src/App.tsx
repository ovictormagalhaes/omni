import Header from './components/Header'
import Hero from './components/Hero'
import MainTabs from './components/MainTabs'
import Footer from './components/Footer'

export default function App() {
  return (
    <div className="min-h-screen bg-omni-navy flex flex-col">
      <Header />
      <main className="flex-1">
        <Hero />
        <div className="container mx-auto px-4 sm:px-6 pb-16">
          <MainTabs />
        </div>
      </main>
      <Footer />
    </div>
  )
}
