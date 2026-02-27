'use client';

import { useMemo } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { TrendingUp, TrendingDown } from 'lucide-react';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import type { EngineTrade } from '@/hooks/engine/types';

interface EquityCurvePanelProps {
  trades: EngineTrade[];
  initialBalance?: number;
}

export function EquityCurvePanel({
  trades,
  initialBalance = 10000,
}: EquityCurvePanelProps) {
  // Compute running equity and statistics
  const { equityData, currentEquity, totalPnL, pnlPercent, maxDrawdown, stats } = useMemo(() => {
    if (!trades || trades.length === 0) {
      return {
        equityData: [],
        currentEquity: initialBalance,
        totalPnL: 0,
        pnlPercent: 0,
        maxDrawdown: 0,
        stats: {
          totalTrades: 0,
          winRate: 0,
          bestTrade: 0,
          worstTrade: 0,
        },
      };
    }

    // Sort trades by close time
    const sortedTrades = [...trades].sort((a, b) => {
      const aTime = new Date(a.closed_at || a.opened_at).getTime();
      const bTime = new Date(b.closed_at || b.opened_at).getTime();
      return aTime - bTime;
    });

    // Build equity curve
    let runningEquity = initialBalance;
    const equityArray: { timestamp: string; equity: number }[] = [];
    let peakEquity = initialBalance; // running high-water mark for drawdown calculation
    let maxDrawdownVal = 0;
    let winCount = 0;
    let bestTrade = -Infinity;
    let worstTrade = Infinity;

    const closedTrades = sortedTrades.filter((t) => t.status === 'closed' && t.pnl !== null);

    closedTrades.forEach((trade) => {
      const pnl = trade.pnl || 0;
      runningEquity += pnl;

      // Update high-water mark, then measure drawdown from it (correct peak-to-trough method)
      if (runningEquity > peakEquity) peakEquity = runningEquity;
      const drawdown = peakEquity > 0 ? ((peakEquity - runningEquity) / peakEquity) * 100 : 0;
      if (drawdown > maxDrawdownVal) {
        maxDrawdownVal = drawdown;
      }

      // Track best/worst
      if (pnl > bestTrade) bestTrade = pnl;
      if (pnl < worstTrade) worstTrade = pnl;

      // Count wins
      if (pnl > 0) winCount++;

      equityArray.push({
        timestamp: new Date(trade.closed_at || trade.opened_at).toLocaleTimeString('en-SG', {
          hour: '2-digit',
          minute: '2-digit',
        }),
        equity: parseFloat(runningEquity.toFixed(2)),
      });
    });

    const totalPnLVal = runningEquity - initialBalance;
    const totalPnLPct = (totalPnLVal / initialBalance) * 100;
    const winRateVal = closedTrades.length > 0 ? (winCount / closedTrades.length) * 100 : 0;

    return {
      equityData: equityArray,
      currentEquity: runningEquity,
      totalPnL: totalPnLVal,
      pnlPercent: totalPnLPct,
      maxDrawdown: maxDrawdownVal,
      stats: {
        totalTrades: closedTrades.length,
        winRate: winRateVal,
        bestTrade: bestTrade === -Infinity ? 0 : bestTrade,
        worstTrade: worstTrade === Infinity ? 0 : worstTrade,
      },
    };
  }, [trades, initialBalance]);

  if (trades.length === 0) {
    return (
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="flex items-center gap-2 text-lg">
            <TrendingUp className="h-5 w-5 text-primary" />
            Equity Curve
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-center text-muted-foreground py-8">No closed trades yet</div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <CardTitle className="flex items-center gap-2 text-lg">
            <TrendingUp className="h-5 w-5 text-primary" />
            Equity Curve
          </CardTitle>
          <Badge variant={totalPnL >= 0 ? 'default' : 'destructive'}>
            {totalPnL >= 0 ? '+' : ''}${totalPnL.toFixed(2)} ({pnlPercent >= 0 ? '+' : ''}
            {pnlPercent.toFixed(1)}%)
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Chart */}
        <div className="w-full h-64 bg-muted/10 rounded-lg p-4">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={equityData}>
              <defs>
                <linearGradient id="colorEquity" x1="0" y1="0" x2="0" y2="1">
                  <stop
                    offset="5%"
                    stopColor={currentEquity >= initialBalance ? '#10b981' : '#ef4444'}
                    stopOpacity={0.3}
                  />
                  <stop
                    offset="95%"
                    stopColor={currentEquity >= initialBalance ? '#10b981' : '#ef4444'}
                    stopOpacity={0.05}
                  />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#333" />
              <XAxis dataKey="timestamp" tick={{ fontSize: 12 }} />
              <YAxis tick={{ fontSize: 12 }} />
              <Tooltip
                contentStyle={{ backgroundColor: '#1a1a1a', border: '1px solid #333' }}
                formatter={(value: number) => `$${value.toFixed(2)}`}
                labelFormatter={(label) => `Time: ${label}`}
              />
              <Area
                type="monotone"
                dataKey="equity"
                stroke={currentEquity >= initialBalance ? '#10b981' : '#ef4444'}
                fill="url(#colorEquity)"
                strokeWidth={2}
                dot={false}
                isAnimationActive={false}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Stats Grid */}
        <div className="grid grid-cols-2 gap-3">
          <StatCard label="Current Equity" value={`$${currentEquity.toFixed(2)}`} trend={currentEquity >= initialBalance ? 'up' : 'down'} />
          <StatCard label="Total P&L" value={`$${totalPnL.toFixed(2)}`} trend={totalPnL >= 0 ? 'up' : 'down'} />
          <StatCard label="Win Rate" value={`${stats.winRate.toFixed(1)}%`} trend={stats.winRate >= 50 ? 'up' : 'down'} />
          <StatCard label="Max Drawdown" value={`${maxDrawdown.toFixed(1)}%`} trend={maxDrawdown <= 10 ? 'up' : 'down'} />
        </div>

        {/* Trade Stats */}
        <div className="pt-2 border-t space-y-3">
          <div className="text-sm font-medium text-muted-foreground">Trade Statistics</div>
          <div className="grid grid-cols-4 gap-2">
            <div className="text-center p-2 bg-muted/30 rounded">
              <div className="text-xs text-muted-foreground">Trades</div>
              <div className="text-sm font-bold">{stats.totalTrades}</div>
            </div>
            <div className="text-center p-2 bg-muted/30 rounded">
              <div className="text-xs text-muted-foreground">Win Rate</div>
              <div className="text-sm font-bold">{stats.winRate.toFixed(0)}%</div>
            </div>
            <div className="text-center p-2 bg-muted/30 rounded">
              <div className="text-xs text-muted-foreground">Best Trade</div>
              <div className="text-sm font-bold text-green-500">+${stats.bestTrade.toFixed(2)}</div>
            </div>
            <div className="text-center p-2 bg-muted/30 rounded">
              <div className="text-xs text-muted-foreground">Worst Trade</div>
              <div className="text-sm font-bold text-red-500">${stats.worstTrade.toFixed(2)}</div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function StatCard({
  label,
  value,
  trend,
}: {
  label: string;
  value: string;
  trend?: 'up' | 'down';
}) {
  const trendIcon = trend === 'up' ? <TrendingUp className="h-4 w-4" /> : <TrendingDown className="h-4 w-4" />;
  const trendColor = trend === 'up' ? 'text-green-500' : 'text-red-500';

  return (
    <div className="text-center p-3 bg-muted/30 rounded-lg space-y-1">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className={`text-lg font-bold flex items-center justify-center gap-1 ${trendColor}`}>
        {value}
        {trendIcon}
      </div>
    </div>
  );
}
