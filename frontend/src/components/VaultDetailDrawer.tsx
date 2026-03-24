/**
 * VaultDetailDrawer
 *
 * Slide-in panel from the right that shows:
 *  - Vault identity (protocol, chain, asset, operation type, name)
 *  - Key metrics (current APY, avg, min, max, liquidity, utilisation)
 *  - APY history chart (last 30 / 7 / 90 days toggle)
 *  - "Open in Protocol" CTA
 *
 * The <APYChart> component is intentionally separated so we can plug in
 * projection data in the future without touching this drawer.
 */

import { useEffect, useState, useCallback } from 'react'
import { X, ExternalLink, TrendingUp, TrendingDown, BarChart2, Droplets, Activity } from 'lucide-react'
import { fetchVaultHistory, type VaultHistoryData, type RateResult } from '../lib/api'
import APYChart, { type APYChartSeries } from './APYChart'
import { ProtocolIcon } from './ProtocolIcon'
import { ChainIcon } from './ChainIcon'

// ─── Types ────────────────────────────────────────────────────────────────────

interface VaultDetailDrawerProps {
  vault: RateResult | null
  action: 'supply' | 'borrow'
  onClose: () => void
}

const DAY_OPTIONS: { label: string; days: number }[] = [
  { label: '7D', days: 7 },
  { label: '30D', days: 30 },
  { label: '90D', days: 90 },
]

const SERIES_OPTIONS: { label: string; value: APYChartSeries }[] = [
  { label: 'Net', value: 'net' },
  { label: 'Base', value: 'base' },
  { label: 'Rewards', value: 'rewards' },
]

// ─── Helpers ─────────────────────────────────────────────────────────────────

function formatMoney(n: number): string {
  if (n >= 1_000_000_000) return `$${(n / 1_000_000_000).toFixed(2)}B`
  if (n >= 1_000_000) return `$${(n / 1_000_000).toFixed(2)}M`
  if (n >= 1_000) return `$${(n / 1_000).toFixed(0)}K`
  return `$${n.toFixed(0)}`
}

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1)
}

// ─── Stat tile ────────────────────────────────────────────────────────────────

interface StatTileProps {
  label: string
  value: string
  sub?: string
  highlight?: boolean
  icon?: React.ReactNode
}

function StatTile({ label, value, sub, highlight, icon }: StatTileProps) {
  return (
    <div className="bg-slate-900/60 border border-slate-700 rounded-lg p-3 flex flex-col gap-1">
      <div className="flex items-center gap-1.5 text-xs text-slate-500">
        {icon}
        {label}
      </div>
      <span
        className={`font-mono font-bold text-lg leading-tight ${
          highlight ? 'text-emerald-400' : 'text-white'
        }`}
      >
        {value}
      </span>
      {sub && <span className="text-xs text-slate-500">{sub}</span>}
    </div>
  )
}

// ─── Component ────────────────────────────────────────────────────────────────

export default function VaultDetailDrawer({ vault, action, onClose }: VaultDetailDrawerProps) {
  const [historyData, setHistoryData] = useState<VaultHistoryData | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [days, setDays] = useState(30)
  const [activeSeries, setActiveSeries] = useState<APYChartSeries[]>(['net'])

  const loadHistory = useCallback(
    async (d: number) => {
      if (!vault) return
      setLoading(true)
      setError(null)
      try {
        const data = await fetchVaultHistory({
          vault_id: vault.vaultId ?? undefined,
          protocol: vault.protocol,
          chain: vault.chain,
          asset: vault.asset,
          days: d,
        })
        setHistoryData(data)
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : 'Failed to load history'
        setError(msg)
      } finally {
        setLoading(false)
      }
    },
    [vault],
  )

  useEffect(() => {
    if (vault) {
      setHistoryData(null)
      loadHistory(days)
    }
  }, [vault, days, loadHistory])

  // Close on Escape key
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [onClose])

  if (!vault) return null

  const currentApy = action === 'supply' ? vault.netApy : vault.apy
  const label = action === 'supply' ? 'APY' : 'APR'

  const toggleSeries = (s: APYChartSeries) => {
    setActiveSeries((prev) =>
      prev.includes(s)
        ? prev.length > 1 ? prev.filter((x) => x !== s) : prev  // keep at least 1
        : [...prev, s],
    )
  }

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/60 backdrop-blur-sm z-40"
        onClick={onClose}
      />

      {/* Drawer panel */}
      <aside className="fixed top-0 right-0 h-full w-full max-w-lg bg-slate-900 border-l border-slate-700 z-50 flex flex-col shadow-2xl overflow-hidden">
        {/* ── Header ──────────────────────────────────────────────────────── */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-700 shrink-0">
          <div className="flex items-center gap-3">
            <ProtocolIcon protocol={vault.protocol} className="w-7 h-7" />
            <div>
              <h2 className="text-white font-bold text-base leading-tight capitalize">
                {vault.protocol}
              </h2>
              {vault.vaultName && (
                <p className="text-slate-400 text-xs mt-0.5 truncate max-w-[240px]">
                  {vault.vaultName}
                </p>
              )}
            </div>
            <div className="flex items-center gap-1.5 ml-1">
              <ChainIcon chain={vault.chain} className="w-4 h-4" />
              <span className="text-slate-400 text-xs capitalize">{vault.chain}</span>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white transition-colors p-1 rounded-lg hover:bg-slate-800"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* ── Scrollable body ──────────────────────────────────────────────── */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-5">

          {/* Asset + type badge */}
          <div className="flex items-center gap-3">
            <span className="text-3xl font-mono font-extrabold text-white">{vault.asset}</span>
            <span className={`px-2.5 py-1 rounded-full text-xs font-semibold border ${
              vault.operationType === 'vault'
                ? 'bg-orange-500/15 text-orange-400 border-orange-500/30'
                : vault.operationType === 'staking'
                ? 'bg-blue-500/15 text-blue-400 border-blue-500/30'
                : 'bg-green-500/15 text-green-400 border-green-500/30'
            }`}>
              {capitalize(vault.operationType)}
            </span>
          </div>

          {/* ── Key metrics grid ─────────────────────────────────────────── */}
          <div className="grid grid-cols-2 gap-3">
            <StatTile
              label={`Current Net ${label}`}
              value={`${currentApy.toFixed(2)}%`}
              sub={`Base: ${vault.apy.toFixed(2)}%`}
              highlight
              icon={<TrendingUp className="w-3 h-3" />}
            />
            <StatTile
              label="Rewards APY"
              value={vault.rewards > 0 ? `+${vault.rewards.toFixed(2)}%` : '—'}
              sub={vault.rewards > 0 ? 'Protocol incentives' : 'No active rewards'}
              icon={<BarChart2 className="w-3 h-3" />}
            />
            <StatTile
              label="Available Liquidity"
              value={formatMoney(vault.liquidity)}
              sub="Immediately deployable"
              icon={<Droplets className="w-3 h-3" />}
            />
            <StatTile
              label="Utilisation"
              value={`${vault.utilizationRate.toFixed(1)}%`}
              sub={formatMoney(vault.totalLiquidity ?? vault.liquidity) + ' total'}
              icon={<Activity className="w-3 h-3" />}
            />
          </div>

          {/* History stats (from loaded data) */}
          {historyData && historyData.points.length > 0 && (
            <div className="grid grid-cols-3 gap-3">
              <StatTile
                label={`Avg ${label} (${days}d)`}
                value={`${historyData.avg_apy.toFixed(2)}%`}
              />
              <StatTile
                label={`Min ${label}`}
                value={`${historyData.min_apy.toFixed(2)}%`}
                icon={<TrendingDown className="w-3 h-3" />}
              />
              <StatTile
                label={`Max ${label}`}
                value={`${historyData.max_apy.toFixed(2)}%`}
                icon={<TrendingUp className="w-3 h-3" />}
              />
            </div>
          )}

          {/* ── APY Chart section ─────────────────────────────────────────── */}
          <div className="space-y-3">
            {/* Chart controls */}
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-semibold text-white flex items-center gap-2">
                <BarChart2 className="w-4 h-4 text-omni-blue" />
                APY History
              </h3>

              <div className="flex items-center gap-2">
                {/* Series toggle */}
                <div className="flex gap-1">
                  {SERIES_OPTIONS.map((s) => (
                    <button
                      key={s.value}
                      onClick={() => toggleSeries(s.value)}
                      className={`px-2 py-1 rounded text-xs font-medium transition-colors ${
                        activeSeries.includes(s.value)
                          ? s.value === 'net'
                            ? 'bg-blue-500/20 text-blue-400 border border-blue-500/40'
                            : s.value === 'base'
                            ? 'bg-purple-500/20 text-purple-400 border border-purple-500/40'
                            : 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/40'
                          : 'bg-slate-800 text-slate-500 border border-slate-700 hover:text-slate-300'
                      }`}
                    >
                      {s.label}
                    </button>
                  ))}
                </div>

                {/* Days toggle */}
                <div className="flex rounded-lg overflow-hidden border border-slate-700">
                  {DAY_OPTIONS.map((opt) => (
                    <button
                      key={opt.days}
                      onClick={() => setDays(opt.days)}
                      className={`px-2.5 py-1 text-xs font-medium transition-colors ${
                        days === opt.days
                          ? 'bg-omni-blue text-white'
                          : 'bg-slate-800 text-slate-400 hover:text-white hover:bg-slate-700'
                      }`}
                    >
                      {opt.label}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            {/* Chart area */}
            <div className="bg-slate-800/50 border border-slate-700 rounded-xl p-3">
              {loading && (
                <div className="flex items-center justify-center text-slate-500 text-sm" style={{ height: 220 }}>
                  <div className="flex items-center gap-2">
                    <div className="w-4 h-4 border-2 border-omni-blue border-t-transparent rounded-full animate-spin" />
                    Loading history…
                  </div>
                </div>
              )}
              {!loading && error && (
                <div className="flex items-center justify-center text-slate-500 text-sm" style={{ height: 220 }}>
                  <span className="text-slate-500">
                    {error.includes('No historical data') || error.includes('500')
                      ? 'No historical snapshots collected yet for this vault.'
                      : `Error: ${error}`}
                  </span>
                </div>
              )}
              {!loading && !error && historyData && !historyData.data_available && (
                <div className="flex flex-col items-center justify-center text-slate-500 text-sm gap-2" style={{ height: 220 }}>
                  <BarChart2 className="w-8 h-8 text-slate-700" />
                  <span>No snapshots collected yet for this vault.</span>
                  <span className="text-xs text-slate-600">Data accumulates daily after the collection worker runs.</span>
                </div>
              )}
              {!loading && !error && historyData && historyData.data_available && (
                <APYChart
                  historical={historyData.points}
                  series={activeSeries}
                  height={220}
                  referenceApy={historyData.avg_apy > 0 ? historyData.avg_apy : undefined}
                  referenceLabel={`Avg ${historyData.avg_apy.toFixed(2)}%`}
                />
              )}
            </div>

            {/* Future projection placeholder hint */}
            <p className="text-xs text-slate-600 text-center">
              Backtest &amp; projection ─ coming soon
            </p>
          </div>
        </div>

        {/* ── Footer CTA ───────────────────────────────────────────────────── */}
        <div className="px-5 py-4 border-t border-slate-700 shrink-0">
          <a
            href={vault.url}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center justify-center gap-2 w-full py-3 bg-omni-blue hover:bg-blue-600 text-white rounded-xl font-semibold text-sm transition-colors"
          >
            {action === 'supply' ? 'Supply' : 'Borrow'} on{' '}
            {capitalize(vault.protocol)}
            <ExternalLink className="w-4 h-4" />
          </a>
        </div>
      </aside>
    </>
  )
}
