import { useEffect, useState } from 'react'
import { X, ExternalLink, TrendingUp, BarChart2, Droplets } from 'lucide-react'
import {
  AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer,
} from 'recharts'
import { fetchPoolHistory, type PoolHistoryData, type PoolResult } from '../lib/api'
import { ProtocolIcon } from './ProtocolIcon'
import { ChainIcon } from './ChainIcon'

interface PoolDetailDrawerProps {
  pool: PoolResult | null
  onClose: () => void
}

const DAY_OPTIONS = [
  { label: '7D', days: 7 },
  { label: '30D', days: 30 },
  { label: '90D', days: 90 },
]

type ChartMetric = 'fee_apr' | 'volume' | 'tvl' | 'turnover'

const METRIC_OPTIONS: { key: ChartMetric; label: string; dataKey: string; color: string; gradientId: string; formatValue: (v: number) => string; formatAxis: (v: number) => string; tooltipLabel: string }[] = [
  {
    key: 'fee_apr', label: 'Fee APR', dataKey: 'fee_apr_24h', color: '#10b981', gradientId: 'feeAprGradient',
    formatValue: (v) => `${v.toFixed(2)}%`, formatAxis: (v) => `${v.toFixed(0)}%`, tooltipLabel: 'Fee APR 24h',
  },
  {
    key: 'volume', label: 'Volume', dataKey: 'volume_24h_usd', color: '#3b82f6', gradientId: 'volumeGradient',
    formatValue: (v) => formatUsd(v), formatAxis: (v) => formatUsd(v), tooltipLabel: 'Volume 24h',
  },
  {
    key: 'tvl', label: 'TVL', dataKey: 'tvl_usd', color: '#8b5cf6', gradientId: 'tvlGradient',
    formatValue: (v) => formatUsd(v), formatAxis: (v) => formatUsd(v), tooltipLabel: 'TVL',
  },
  {
    key: 'turnover', label: 'Turnover', dataKey: 'turnover_ratio_24h', color: '#f59e0b', gradientId: 'turnoverGradient',
    formatValue: (v) => formatTurnover(v), formatAxis: (v) => formatTurnover(v), tooltipLabel: 'Turnover 24h',
  },
]

function formatUsd(value: number): string {
  if (value >= 1e9) return `$${(value / 1e9).toFixed(2)}B`
  if (value >= 1e6) return `$${(value / 1e6).toFixed(2)}M`
  if (value >= 1e3) return `$${(value / 1e3).toFixed(1)}K`
  return `$${value.toFixed(0)}`
}

function formatApr(value: number): string {
  return `${value.toFixed(2)}%`
}

function formatTurnover(value: number): string {
  if (value >= 1) return `${value.toFixed(2)}x`
  return `${(value * 100).toFixed(1)}%`
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

export default function PoolDetailDrawer({ pool, onClose }: PoolDetailDrawerProps) {
  const [history, setHistory] = useState<PoolHistoryData | null>(null)
  const [loadingHistory, setLoadingHistory] = useState(false)
  const [selectedDays, setSelectedDays] = useState(30)
  const [selectedMetric, setSelectedMetric] = useState<ChartMetric>('fee_apr')
  const [feePeriod, setFeePeriod] = useState<'24h' | '7d'>('24h')

  useEffect(() => {
    if (!pool) {
      setHistory(null)
      return
    }

    const load = async () => {
      setLoadingHistory(true)
      try {
        const data = await fetchPoolHistory({ pool_vault_id: pool.poolVaultId })
        setHistory(data)
      } catch {
        setHistory(null)
      } finally {
        setLoadingHistory(false)
      }
    }
    load()
  }, [pool?.poolVaultId])

  if (!pool) return null

  const filteredPoints = history?.points
    ? (() => {
        const cutoff = new Date()
        cutoff.setDate(cutoff.getDate() - selectedDays)
        return history.points.filter(p => new Date(p.date) >= cutoff)
      })()
    : []

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/50 z-40 transition-opacity"
        onClick={onClose}
      />

      {/* Drawer */}
      <div className="fixed right-0 top-0 h-full w-full max-w-lg bg-slate-900 border-l border-slate-700/50 z-50 overflow-y-auto shadow-2xl animate-slide-in">
        {/* Header */}
        <div className="sticky top-0 bg-slate-900/95 backdrop-blur border-b border-slate-800 px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <ProtocolIcon protocol={pool.protocol} className="w-6 h-6" />
            <div>
              <h2 className="text-lg font-bold text-white">{pool.pair}</h2>
              <div className="flex items-center gap-2 text-xs text-slate-400">
                <ChainIcon chain={pool.chain} className="w-3.5 h-3.5" />
                <span className="capitalize">{pool.chain}</span>
                <span className="text-slate-600">|</span>
                <span className="capitalize">{pool.protocol}</span>
                <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                  pool.poolType === 'concentrated'
                    ? 'bg-purple-500/20 text-purple-400'
                    : 'bg-slate-700/50 text-slate-400'
                }`}>
                  {pool.poolType === 'concentrated' ? 'CLMM' : 'AMM'}
                </span>
                <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-slate-700/50 text-slate-400">
                  {pool.feeTier}
                </span>
              </div>
            </div>
          </div>
          <button onClick={onClose} className="p-2 hover:bg-slate-800 rounded-lg transition-colors">
            <X className="w-5 h-5 text-slate-400" />
          </button>
        </div>

        <div className="p-6 space-y-6">
          {/* Fee APR — primary metric */}
          <div className="card p-5 bg-emerald-500/5 border-emerald-500/20">
            <div className="flex items-center gap-2 text-xs text-slate-400 mb-1">
              <TrendingUp className="w-3.5 h-3.5" /> Fee APR ({feePeriod})
            </div>
            <p className="text-3xl font-bold text-emerald-400">{formatApr(feePeriod === '24h' ? pool.feeApr24h : pool.feeApr7d)}</p>
            <p className="text-xs text-slate-500 mt-1">
              = {formatTurnover(feePeriod === '24h' ? pool.turnoverRatio24h : pool.turnoverRatio7d)} turnover &times; {pool.feeTier} fee &times; 365
            </p>
          </div>

          {/* Period toggle + KPIs */}
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-medium text-slate-400">Metrics</span>
            <div className="flex gap-0.5 p-0.5 bg-slate-800/60 rounded-lg">
              {(['24h', '7d'] as const).map(p => (
                <button
                  key={p}
                  onClick={() => setFeePeriod(p)}
                  className={`px-2.5 py-1 rounded text-xs font-medium transition-all ${
                    feePeriod === p
                      ? 'bg-emerald-500/20 text-emerald-400'
                      : 'text-slate-500 hover:text-slate-300'
                  }`}
                >
                  {p}
                </button>
              ))}
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="card p-4">
              <div className="flex items-center gap-2 text-xs text-slate-500 mb-1">
                <BarChart2 className="w-3.5 h-3.5" /> Volume {feePeriod}
              </div>
              <p className="text-lg font-semibold text-white">{formatUsd(feePeriod === '24h' ? pool.volume24h : pool.volume7d)}</p>
            </div>
            <div className="card p-4">
              <div className="flex items-center gap-2 text-xs text-slate-500 mb-1">
                <Droplets className="w-3.5 h-3.5" /> TVL
              </div>
              <p className="text-lg font-semibold text-white">{formatUsd(pool.tvlUsd)}</p>
            </div>
            <div className="card p-4 bg-emerald-500/5 border-emerald-500/10">
              <div className="text-xs text-slate-500 mb-1">Fees {feePeriod}</div>
              <p className="text-lg font-semibold text-emerald-400">{formatUsd(feePeriod === '24h' ? pool.fees24h : pool.fees7d)}</p>
            </div>
            <div className="card p-4">
              <div className="text-xs text-slate-500 mb-1">Fee APR {feePeriod}</div>
              <p className="text-lg font-semibold text-slate-300">{formatApr(feePeriod === '24h' ? pool.feeApr24h : pool.feeApr7d)}</p>
            </div>
            <div className="card p-4">
              <div className="text-xs text-slate-500 mb-1">Fee Tier</div>
              <p className="text-lg font-semibold text-slate-300">{pool.feeTier}</p>
            </div>
            <div className="card p-4">
              <div className="text-xs text-slate-500 mb-1">Turnover {feePeriod}</div>
              <p className="text-lg font-semibold text-slate-300">{formatTurnover(feePeriod === '24h' ? pool.turnoverRatio24h : pool.turnoverRatio7d)}</p>
            </div>
            {pool.rewardsApr > 0 && (
              <div className="card p-4 col-span-2">
                <div className="text-xs text-slate-500 mb-1">Rewards APR</div>
                <p className="text-lg font-semibold text-omni-gold">{formatApr(pool.rewardsApr)}</p>
              </div>
            )}
          </div>

          {/* Historical stats */}
          {history?.data_available && (
            <div className="card p-4">
              <div className="flex items-center justify-between mb-1">
                <h3 className="text-sm font-medium text-slate-300">Historical Stats</h3>
              </div>
              <div className="grid grid-cols-3 gap-3 text-sm">
                <div>
                  <p className="text-xs text-slate-500">Avg Fee APR</p>
                  <p className="text-slate-300">{formatApr(history.avg_fee_apr)}</p>
                </div>
                <div>
                  <p className="text-xs text-slate-500">Min Fee APR</p>
                  <p className="text-slate-300">{formatApr(history.min_fee_apr)}</p>
                </div>
                <div>
                  <p className="text-xs text-slate-500">Max Fee APR</p>
                  <p className="text-slate-300">{formatApr(history.max_fee_apr)}</p>
                </div>
              </div>
            </div>
          )}

          {/* Chart */}
          <div className="card p-4">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-sm font-medium text-slate-300">History</h3>
              <div className="flex gap-1 p-0.5 bg-slate-800/60 rounded-lg">
                {DAY_OPTIONS.map(opt => (
                  <button
                    key={opt.days}
                    onClick={() => setSelectedDays(opt.days)}
                    className={`px-2.5 py-1 rounded text-xs font-medium transition-all ${
                      selectedDays === opt.days
                        ? 'bg-emerald-500/20 text-emerald-400'
                        : 'text-slate-500 hover:text-slate-300'
                    }`}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Metric selector */}
            <div className="flex gap-1 p-0.5 bg-slate-800/40 rounded-lg mb-4">
              {METRIC_OPTIONS.map(opt => (
                <button
                  key={opt.key}
                  onClick={() => setSelectedMetric(opt.key)}
                  className={`flex-1 px-2 py-1.5 rounded text-xs font-medium transition-all ${
                    selectedMetric === opt.key
                      ? 'bg-slate-700/80 text-white'
                      : 'text-slate-500 hover:text-slate-300'
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>

            {loadingHistory ? (
              <div className="h-48 flex items-center justify-center">
                <div className="w-6 h-6 border-2 border-emerald-500/30 border-t-emerald-500 rounded-full animate-spin" />
              </div>
            ) : filteredPoints.length > 0 ? (
              (() => {
                const metric = METRIC_OPTIONS.find(m => m.key === selectedMetric)!
                return (
                  <ResponsiveContainer width="100%" height={200}>
                    <AreaChart data={filteredPoints}>
                      <defs>
                        <linearGradient id={metric.gradientId} x1="0" y1="0" x2="0" y2="1">
                          <stop offset="5%" stopColor={metric.color} stopOpacity={0.3} />
                          <stop offset="95%" stopColor={metric.color} stopOpacity={0} />
                        </linearGradient>
                      </defs>
                      <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                      <XAxis dataKey="date" tickFormatter={formatDate} tick={{ fontSize: 10, fill: '#64748b' }} />
                      <YAxis tickFormatter={metric.formatAxis} tick={{ fontSize: 10, fill: '#64748b' }} width={60} />
                      <Tooltip
                        contentStyle={{ background: '#0f172a', border: '1px solid #334155', borderRadius: 8 }}
                        labelFormatter={(label) => formatDate(String(label))}
                        formatter={(value: number) => [metric.formatValue(value), metric.tooltipLabel]}
                      />
                      <Area
                        type="monotone"
                        dataKey={metric.dataKey}
                        stroke={metric.color}
                        fill={`url(#${metric.gradientId})`}
                        strokeWidth={2}
                        dot={false}
                      />
                    </AreaChart>
                  </ResponsiveContainer>
                )
              })()
            ) : (
              <div className="h-48 flex items-center justify-center text-slate-500 text-sm">
                No historical data available yet
              </div>
            )}
          </div>

          {/* Action */}
          <a
            href={pool.url}
            target="_blank"
            rel="noopener noreferrer"
            className="btn-primary w-full flex items-center justify-center gap-2 py-3"
          >
            Add Liquidity on {pool.protocol.charAt(0).toUpperCase() + pool.protocol.slice(1)}
            <ExternalLink className="w-4 h-4" />
          </a>
        </div>
      </div>
    </>
  )
}
