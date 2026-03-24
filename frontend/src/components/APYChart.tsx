/**
 * APYChart — reusable APY time-series chart.
 *
 * Accepts two data series:
 *  - `historical` – past snapshots (solid area)
 *  - `projection` – future projections (dashed area, optional)
 *
 * This split is intentional: the backtest/projection feature will populate
 * `projection` in the future without changing this component's API.
 */

import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
  ReferenceLine,
} from 'recharts'
import { useMemo } from 'react'
import type { VaultHistoryPoint } from '../lib/api'

// ─── Types ────────────────────────────────────────────────────────────────────

export interface ProjectionPoint {
  /** ISO-8601 date string */
  date: string
  /** Projected net APY */
  net_apy: number
  /** Lower confidence bound (optional band) */
  lower_bound?: number
  /** Upper confidence bound (optional band) */
  upper_bound?: number
}

export type APYChartSeries = 'net' | 'base' | 'rewards'

export interface APYChartProps {
  historical: VaultHistoryPoint[]
  projection?: ProjectionPoint[]
  /** Which series to display (default: ['net']) */
  series?: APYChartSeries[]
  height?: number
  /** Reference line for the current/average APY */
  referenceApy?: number
  referenceLabel?: string
  showLegend?: boolean
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function formatDate(iso: string): string {
  const d = new Date(iso)
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

function formatApy(v: number): string {
  return `${v.toFixed(2)}%`
}

// ─── Custom Tooltip ───────────────────────────────────────────────────────────

interface CustomTooltipProps {
  active?: boolean
  payload?: Array<{ name: string; value: number; color: string }>
  label?: string
}

function CustomTooltip({ active, payload, label }: CustomTooltipProps) {
  if (!active || !payload?.length) return null
  return (
    <div className="bg-slate-800 border border-slate-600 rounded-lg p-3 shadow-xl text-xs">
      <p className="text-omni-silver mb-2 font-medium">{label}</p>
      {payload.map((entry) => (
        <div key={entry.name} className="flex items-center gap-2 mb-1">
          <span className="w-2 h-2 rounded-full" style={{ background: entry.color }} />
          <span className="text-slate-400">{entry.name}:</span>
          <span className="text-white font-mono font-semibold">{formatApy(entry.value)}</span>
        </div>
      ))}
    </div>
  )
}

// ─── Component ────────────────────────────────────────────────────────────────

export default function APYChart({
  historical,
  projection,
  series = ['net'],
  height = 220,
  referenceApy,
  referenceLabel,
  showLegend = false,
}: APYChartProps) {
  const hasProjection = !!projection?.length

  // Merge historical + projection into a single chart dataset
  const data = useMemo(() => {
    const hist = historical.map((p) => ({
      date: formatDate(p.date),
      rawDate: p.date,
      net_apy: p.net_apy,
      base_apy: p.base_apy,
      rewards_apy: p.rewards_apy,
      proj_net_apy: undefined as number | undefined,
      proj_lower: undefined as number | undefined,
      proj_upper: undefined as number | undefined,
      isProjection: false,
    }))

    if (!hasProjection) return hist

    const proj = (projection ?? []).map((p) => ({
      date: formatDate(p.date),
      rawDate: p.date,
      net_apy: undefined as number | undefined,
      base_apy: undefined as number | undefined,
      rewards_apy: undefined as number | undefined,
      proj_net_apy: p.net_apy,
      proj_lower: p.lower_bound,
      proj_upper: p.upper_bound,
      isProjection: true,
    }))

    return [...hist, ...proj]
  }, [historical, projection, hasProjection])

  if (!data.length) {
    return (
      <div
        className="flex items-center justify-center text-slate-500 text-sm"
        style={{ height }}
      >
        No historical data available yet
      </div>
    )
  }

  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={data} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
        <defs>
          {/* Historical net APY gradient */}
          <linearGradient id="gradNet" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.3} />
            <stop offset="95%" stopColor="#3b82f6" stopOpacity={0.02} />
          </linearGradient>
          {/* Historical base APY gradient */}
          <linearGradient id="gradBase" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="#8b5cf6" stopOpacity={0.25} />
            <stop offset="95%" stopColor="#8b5cf6" stopOpacity={0.02} />
          </linearGradient>
          {/* Rewards APY gradient */}
          <linearGradient id="gradRewards" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="#10b981" stopOpacity={0.25} />
            <stop offset="95%" stopColor="#10b981" stopOpacity={0.02} />
          </linearGradient>
          {/* Projection gradient */}
          <linearGradient id="gradProj" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="#f59e0b" stopOpacity={0.2} />
            <stop offset="95%" stopColor="#f59e0b" stopOpacity={0.02} />
          </linearGradient>
        </defs>

        <CartesianGrid strokeDasharray="3 3" stroke="#334155" vertical={false} />

        <XAxis
          dataKey="date"
          tick={{ fill: '#64748b', fontSize: 11 }}
          tickLine={false}
          axisLine={false}
          interval="preserveStartEnd"
        />
        <YAxis
          tickFormatter={(v) => `${v.toFixed(1)}%`}
          tick={{ fill: '#64748b', fontSize: 11 }}
          tickLine={false}
          axisLine={false}
          width={48}
        />

        <Tooltip content={<CustomTooltip />} />
        {showLegend && <Legend wrapperStyle={{ fontSize: 12, color: '#94a3b8' }} />}

        {/* Reference line (avg APY or any marker) */}
        {referenceApy !== undefined && (
          <ReferenceLine
            y={referenceApy}
            stroke="#64748b"
            strokeDasharray="4 4"
            label={{
              value: referenceLabel ?? formatApy(referenceApy),
              fill: '#64748b',
              fontSize: 10,
              position: 'insideTopRight',
            }}
          />
        )}

        {/* ── Historical series ── */}
        {series.includes('base') && (
          <Area
            type="monotone"
            dataKey="base_apy"
            name="Base APY"
            stroke="#8b5cf6"
            strokeWidth={1.5}
            fill="url(#gradBase)"
            dot={false}
            activeDot={{ r: 4 }}
            connectNulls
          />
        )}
        {series.includes('rewards') && (
          <Area
            type="monotone"
            dataKey="rewards_apy"
            name="Rewards APY"
            stroke="#10b981"
            strokeWidth={1.5}
            fill="url(#gradRewards)"
            dot={false}
            activeDot={{ r: 4 }}
            connectNulls
          />
        )}
        {series.includes('net') && (
          <Area
            type="monotone"
            dataKey="net_apy"
            name="Net APY"
            stroke="#3b82f6"
            strokeWidth={2}
            fill="url(#gradNet)"
            dot={false}
            activeDot={{ r: 5 }}
            connectNulls
          />
        )}

        {/* ── Projection series (future) ── */}
        {hasProjection && (
          <Area
            type="monotone"
            dataKey="proj_net_apy"
            name="Projection"
            stroke="#f59e0b"
            strokeWidth={2}
            strokeDasharray="6 3"
            fill="url(#gradProj)"
            dot={false}
            activeDot={{ r: 5 }}
            connectNulls
          />
        )}
      </AreaChart>
    </ResponsiveContainer>
  )
}
