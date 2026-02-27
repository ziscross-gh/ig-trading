'use client';

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Brain, TrendingUp, TrendingDown, Minus, RefreshCw, Activity } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { EngineLearning, StrategyLearningEntry, WeightAdjustment } from '@/hooks/useEngine';

// ─── helpers ────────────────────────────────────────────────────────────────

const SESSIONS = ['Asia', 'London', 'US'] as const;

/** Strategy display names */
const STRATEGY_LABELS: Record<string, string> = {
  MA_Crossover: 'MA Crossover',
  RSI_Reversal: 'RSI Reversal',
  MACD_Momentum: 'MACD Momentum',
  Bollinger_Reversion: 'Bollinger',
};

function fmt(n: number, decimals = 2) {
  return n.toFixed(decimals);
}

function fmtPct(n: number) {
  return `${(n * 100).toFixed(1)}%`;
}

function multiplierColor(m: number) {
  if (m >= 1.4) return 'text-emerald-400';
  if (m >= 1.1) return 'text-green-400';
  if (m <= 0.6) return 'text-red-400';
  if (m <= 0.9) return 'text-orange-400';
  return 'text-slate-300';
}

function multiplierBadge(m: number) {
  if (m >= 1.2) return 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30';
  if (m >= 1.0) return 'bg-green-500/20 text-green-400 border-green-500/30';
  if (m <= 0.7) return 'bg-red-500/20 text-red-400 border-red-500/30';
  return 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30';
}

function heatColor(winRate: number) {
  if (winRate >= 0.7) return 'bg-emerald-500';
  if (winRate >= 0.55) return 'bg-green-500';
  if (winRate >= 0.45) return 'bg-yellow-500';
  if (winRate >= 0.35) return 'bg-orange-500';
  return 'bg-red-500';
}

function heatOpacity(winRate: number) {
  // Scale 0–1 win rate to 30–100% opacity
  return Math.round(30 + winRate * 70);
}

function timeSince(isoString: string): string {
  const delta = Date.now() - new Date(isoString).getTime();
  const mins = Math.floor(delta / 60_000);
  const hours = Math.floor(delta / 3_600_000);
  const days = Math.floor(delta / 86_400_000);
  if (days > 0) return `${days}d ago`;
  if (hours > 0) return `${hours}h ago`;
  if (mins > 0) return `${mins}m ago`;
  return 'just now';
}

// ─── sub-components ──────────────────────────────────────────────────────────

/** Rolling win-rate progress bar */
function WinRateBar({ rate }: { rate: number }) {
  const pct = rate * 100;
  const color =
    pct >= 60 ? 'bg-emerald-500' : pct >= 45 ? 'bg-yellow-500' : 'bg-red-500';
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 bg-slate-700 rounded-full h-2 overflow-hidden">
        <div
          className={`h-full rounded-full transition-all ${color}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-xs text-slate-300 w-10 text-right">{fmtPct(rate)}</span>
    </div>
  );
}

/** Per-strategy scorecard card */
function StrategyCard({ entry }: { entry: StrategyLearningEntry }) {
  const label = STRATEGY_LABELS[entry.name] ?? entry.name;
  const trendIcon =
    entry.current_multiplier >= 1.05 ? (
      <TrendingUp className="w-3.5 h-3.5 text-emerald-400" />
    ) : entry.current_multiplier <= 0.95 ? (
      <TrendingDown className="w-3.5 h-3.5 text-red-400" />
    ) : (
      <Minus className="w-3.5 h-3.5 text-slate-400" />
    );

  return (
    <div className="bg-slate-800/60 border border-slate-700/50 rounded-xl p-4 space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {trendIcon}
          <span className="text-sm font-medium text-slate-200">{label}</span>
        </div>
        <span
          className={`text-xs font-bold px-2 py-0.5 rounded-full border ${multiplierBadge(
            entry.current_multiplier
          )}`}
        >
          {fmt(entry.current_multiplier)}×
        </span>
      </div>

      {/* Win-rate bar */}
      <WinRateBar rate={entry.win_rate} />

      {/* Stats row */}
      <div className="grid grid-cols-3 gap-2 text-xs">
        <div className="text-center">
          <p className="text-slate-400">Prof. Factor</p>
          <p className={`font-semibold ${entry.profit_factor >= 1 ? 'text-green-400' : 'text-red-400'}`}>
            {fmt(entry.profit_factor)}
          </p>
        </div>
        <div className="text-center">
          <p className="text-slate-400">Eff. Weight</p>
          <p className={`font-semibold ${multiplierColor(entry.effective_weight)}`}>
            {fmt(entry.effective_weight)}
          </p>
        </div>
        <div className="text-center">
          <p className="text-slate-400">Max Consec.</p>
          <p className={`font-semibold ${entry.max_consecutive_losses >= 4 ? 'text-red-400' : 'text-slate-300'}`}>
            {entry.max_consecutive_losses}L
          </p>
        </div>
      </div>

      {/* Trades in window */}
      <p className="text-[10px] text-slate-500 text-right">
        {entry.trades_in_window} trades in window
      </p>
    </div>
  );
}

/** Session heatmap table */
function SessionHeatmap({ strategies }: { strategies: StrategyLearningEntry[] }) {
  if (strategies.length === 0) {
    return (
      <div className="text-center text-sm text-slate-500 py-6">
        No session data yet — trades are still accumulating.
      </div>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-xs">
        <thead>
          <tr>
            <th className="text-left text-slate-400 font-medium pb-2 pr-4">Strategy</th>
            {SESSIONS.map((s) => (
              <th key={s} className="text-center text-slate-400 font-medium pb-2 px-2 w-24">
                {s}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="space-y-1">
          {strategies.map((entry) => {
            const label = STRATEGY_LABELS[entry.name] ?? entry.name;
            return (
              <tr key={entry.name} className="border-t border-slate-700/30">
                <td className="py-2 pr-4 text-slate-300 font-medium whitespace-nowrap">
                  {label}
                </td>
                {SESSIONS.map((session) => {
                  const stat = entry.sessions[session];
                  if (!stat) {
                    return (
                      <td key={session} className="py-2 px-2 text-center">
                        <div className="inline-flex flex-col items-center gap-0.5">
                          <div className="w-14 h-7 bg-slate-700/30 rounded flex items-center justify-center">
                            <span className="text-slate-600">—</span>
                          </div>
                        </div>
                      </td>
                    );
                  }
                  return (
                    <td key={session} className="py-2 px-2 text-center">
                      <div className="inline-flex flex-col items-center gap-0.5">
                        <div
                          className={`w-14 h-7 rounded flex items-center justify-center ${heatColor(stat.win_rate)}`}
                          style={{ opacity: `${heatOpacity(stat.win_rate)}%` }}
                          title={`Win: ${fmtPct(stat.win_rate)} | PF: ${fmt(stat.profit_factor)}`}
                        >
                          <span className="text-white font-semibold text-[11px]">
                            {fmtPct(stat.win_rate)}
                          </span>
                        </div>
                        <span className="text-slate-500 text-[10px]">
                          PF {fmt(stat.profit_factor, 1)}
                        </span>
                      </div>
                    </td>
                  );
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** Adjustment log entry */
function AdjustmentRow({ adj }: { adj: WeightAdjustment }) {
  const label = STRATEGY_LABELS[adj.strategy] ?? adj.strategy;
  const up = adj.new_weight >= adj.old_weight;
  const pctChange = ((adj.new_weight - adj.old_weight) / Math.max(adj.old_weight, 0.001)) * 100;

  return (
    <div className="flex items-start gap-3 py-2.5 border-b border-slate-700/30 last:border-0">
      <div
        className={`mt-0.5 w-1.5 h-1.5 rounded-full flex-shrink-0 ${
          up ? 'bg-emerald-400' : 'bg-red-400'
        }`}
      />
      <div className="flex-1 min-w-0">
        <p className="text-sm text-slate-200">
          <span className={up ? 'text-emerald-400' : 'text-red-400'}>
            {up ? '↑ Boosted' : '↓ Reduced'}
          </span>{' '}
          {label} to{' '}
          <span className="font-semibold text-white">{fmt(adj.new_weight)}×</span>
          <span className="text-slate-400 text-xs ml-1">
            ({up ? '+' : ''}{fmt(pctChange, 1)}%)
          </span>
        </p>
        <p className="text-xs text-slate-500 mt-0.5">
          Win rate {fmtPct(adj.win_rate)} · PF {fmt(adj.profit_factor)} · {adj.trade_count} trades
        </p>
      </div>
      <span className="text-xs text-slate-500 flex-shrink-0 mt-0.5">
        {timeSince(adj.timestamp)}
      </span>
    </div>
  );
}

// ─── main component ──────────────────────────────────────────────────────────

interface LearningPanelProps {
  learning: EngineLearning | null;
  onRefresh?: () => void;
}

export function LearningPanel({ learning, onRefresh }: LearningPanelProps) {
  const hasData =
    learning && (learning.strategies.length > 0 || learning.total_trades_processed > 0);

  return (
    <Card className="bg-slate-900/60 border-slate-700/50 backdrop-blur-sm">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Brain className="w-5 h-5 text-violet-400" />
            <CardTitle className="text-slate-200 text-lg">Adaptive Learning</CardTitle>
            {hasData && (
              <Badge
                variant="outline"
                className="ml-1 text-[10px] bg-violet-500/10 text-violet-400 border-violet-500/30"
              >
                {learning!.total_trades_processed} trades
              </Badge>
            )}
          </div>
          {onRefresh && (
            <Button
              variant="ghost"
              size="sm"
              onClick={onRefresh}
              className="h-7 w-7 p-0 text-slate-400 hover:text-slate-200"
            >
              <RefreshCw className="w-3.5 h-3.5" />
            </Button>
          )}
        </div>
        <CardDescription className="text-slate-400 text-xs">
          Dynamic strategy weights adjusted by rolling win rate and profit factor
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-6">
        {/* ── Empty state ── */}
        {!hasData && (
          <div className="flex flex-col items-center gap-3 py-10 text-center">
            <Activity className="w-10 h-10 text-slate-600" />
            <p className="text-slate-400 text-sm">Waiting for trade data…</p>
            <p className="text-slate-500 text-xs max-w-xs">
              The adaptive learning system activates after{' '}
              <span className="text-slate-400">20+ closed trades</span> per
              strategy. Keep trading!
            </p>
          </div>
        )}

        {/* ── Strategy scorecards ── */}
        {hasData && learning!.strategies.length > 0 && (
          <div>
            <h3 className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-3">
              Strategy Scorecards
            </h3>
            <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-3">
              {learning!.strategies.map((entry) => (
                <StrategyCard key={entry.name} entry={entry} />
              ))}
            </div>
          </div>
        )}

        {/* ── Session heatmap ── */}
        {hasData && (
          <div>
            <h3 className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-3">
              Session Heatmap — Win Rate by Trading Session
            </h3>
            <div className="bg-slate-800/40 border border-slate-700/40 rounded-xl p-4">
              <SessionHeatmap strategies={learning!.strategies} />
            </div>
          </div>
        )}

        {/* ── Adaptation log ── */}
        {hasData && (
          <div>
            <h3 className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-3">
              Adaptation Log
            </h3>
            <div className="bg-slate-800/40 border border-slate-700/40 rounded-xl px-4 max-h-64 overflow-y-auto">
              {learning!.recent_adjustments.length === 0 ? (
                <p className="text-slate-500 text-xs py-4 text-center">
                  No weight adjustments yet — not enough trades in the rolling window.
                </p>
              ) : (
                [...learning!.recent_adjustments]
                  .reverse()
                  .map((adj, i) => <AdjustmentRow key={`${adj.strategy}-${adj.timestamp}-${i}`} adj={adj} />)
              )}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
