'use client';

import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import {
  Activity,
  Play,
  Square,
  Pause,
  RefreshCw,
  Shield,
  TrendingUp,
  TrendingDown,
  Cpu,
  Timer,
  Gauge,
} from 'lucide-react';
import type {
  EngineStatus,
  EnginePosition,
  EngineSignal,
} from '@/hooks/useEngine';

// ============================================
// Engine Control Panel
// ============================================
interface EngineControlProps {
  status: EngineStatus | null;
  connected: boolean;
  loading: boolean;
  onStart: () => Promise<{ success: boolean }>;
  onStop: () => Promise<{ success: boolean }>;
  onPause: () => Promise<{ success: boolean }>;
}

export function EngineControlPanel({
  status,
  connected,
  loading,
  onStart,
  onStop,
  onPause,
}: EngineControlProps) {
  const isRunning = status?.status === 'running';
  const isPaused = status?.status === 'paused';
  const isStopped = !status || status.status === 'stopped';

  const formatUptime = (secs: number) => {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return h > 0 ? `${h}h ${m}m` : `${m}m`;
  };

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-sm font-medium flex items-center justify-between">
          <span className="flex items-center gap-2">
            <Cpu className="h-4 w-4" />
            Rust Engine
          </span>
          <Badge
            variant={connected ? 'default' : 'destructive'}
            className={
              isRunning
                ? 'bg-green-500 hover:bg-green-600'
                : isPaused
                  ? 'bg-yellow-500 hover:bg-yellow-600'
                  : ''
            }
          >
            <Activity
              className={`h-3 w-3 mr-1 ${isRunning ? 'animate-pulse' : ''}`}
            />
            {!connected
              ? 'Offline'
              : status?.status?.toUpperCase() || 'UNKNOWN'}
          </Badge>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Controls */}
        <div className="flex gap-2">
          {isStopped ? (
            <Button
              onClick={onStart}
              disabled={loading || !connected}
              className="flex-1 bg-green-600 hover:bg-green-700"
              size="sm"
            >
              <Play className="h-4 w-4 mr-1" />
              Start
            </Button>
          ) : (
            <>
              <Button
                onClick={onPause}
                disabled={loading || isPaused}
                variant="outline"
                className="flex-1"
                size="sm"
              >
                <Pause className="h-4 w-4 mr-1" />
                Pause
              </Button>
              <Button
                onClick={onStop}
                disabled={loading}
                variant="destructive"
                className="flex-1"
                size="sm"
              >
                <Square className="h-4 w-4 mr-1" />
                Stop
              </Button>
            </>
          )}
        </div>

        {/* Stats */}
        {status && (
          <>
            <Separator />
            <div className="grid grid-cols-2 gap-3 text-sm">
              <div>
                <div className="text-muted-foreground text-xs">Mode</div>
                <div className="font-medium capitalize">
                  <Badge variant="outline" className="text-xs">
                    {status.mode}
                  </Badge>
                </div>
              </div>
              <div>
                <div className="text-muted-foreground text-xs">Uptime</div>
                <div className="font-medium flex items-center gap-1">
                  <Timer className="h-3 w-3" />
                  {formatUptime(status.uptime_secs)}
                </div>
              </div>
              <div>
                <div className="text-muted-foreground text-xs">
                  Trades Today
                </div>
                <div className="font-medium">
                  {status.daily_stats.trades_today}
                </div>
              </div>
              <div>
                <div className="text-muted-foreground text-xs">Win Rate</div>
                <div className="font-medium">
                  {status.daily_stats.trades_today > 0
                    ? (
                      (status.daily_stats.winning /
                        status.daily_stats.trades_today) *
                      100
                    ).toFixed(0)
                    : 0}
                  %
                </div>
              </div>
              <div>
                <div className="text-muted-foreground text-xs">Daily P&L</div>
                <div
                  className={`font-medium ${status.daily_stats.net_pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}
                >
                  {status.daily_stats.net_pnl >= 0 ? '+' : ''}$
                  {status.daily_stats.net_pnl.toFixed(2)}
                </div>
              </div>
              <div>
                <div className="text-muted-foreground text-xs">Positions</div>
                <div className="font-medium">{status.open_positions}</div>
              </div>
            </div>

            {/* Circuit Breaker */}
            {status.circuit_breaker.consecutive_losses > 0 && (
              <>
                <Separator />
                <div className="flex items-center gap-2 text-xs">
                  <Shield
                    className={`h-4 w-4 ${status.circuit_breaker.is_paused ? 'text-red-500' : 'text-yellow-500'}`}
                  />
                  <span>
                    {status.circuit_breaker.is_paused
                      ? 'Circuit breaker ACTIVE — trading paused'
                      : `${status.circuit_breaker.consecutive_losses} consecutive losses (size: ${(status.circuit_breaker.size_multiplier * 100).toFixed(0)}%)`}
                  </span>
                </div>
              </>
            )}

            {/* Account */}
            {status.account && (
              <>
                <Separator />
                <div className="grid grid-cols-2 gap-3 text-sm">
                  <div>
                    <div className="text-muted-foreground text-xs">Balance</div>
                    <div className="font-semibold">
                      ${status.account.balance.toFixed(2)}
                    </div>
                  </div>
                  <div>
                    <div className="text-muted-foreground text-xs">
                      Available
                    </div>
                    <div className="font-semibold">
                      ${status.account.available.toFixed(2)}
                    </div>
                  </div>
                  <div>
                    <div className="text-muted-foreground text-xs">Margin</div>
                    <div className="font-semibold">
                      ${status.account.margin_used.toFixed(2)}
                    </div>
                  </div>
                  <div>
                    <div className="text-muted-foreground text-xs">P&L</div>
                    <div
                      className={`font-semibold ${status.account.pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}
                    >
                      {status.account.pnl >= 0 ? '+' : ''}$
                      {status.account.pnl.toFixed(2)}
                    </div>
                  </div>
                </div>
              </>
            )}
          </>
        )}
      </CardContent>
    </Card>
  );
}

// ============================================
// Engine Positions Panel
// ============================================
interface EnginePositionsPanelProps {
  positions: EnginePosition[];
  loading: boolean;
  onRefresh: () => void;
}

export function EnginePositionsPanel({
  positions,
  loading,
  onRefresh,
}: EnginePositionsPanelProps) {
  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-sm font-medium flex items-center justify-between">
          <span>Open Positions ({positions.length})</span>
          <Button
            variant="ghost"
            size="sm"
            onClick={onRefresh}
            disabled={loading}
          >
            <RefreshCw
              className={`h-3 w-3 ${loading ? 'animate-spin' : ''}`}
            />
          </Button>
        </CardTitle>
      </CardHeader>
      <CardContent>
        {positions.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">
            No open positions
          </p>
        ) : (
          <div className="space-y-3">
            {positions.map((pos) => (
              <div
                key={pos.deal_id}
                className="flex items-center justify-between p-2 rounded-lg border text-sm"
              >
                <div className="flex items-center gap-2">
                  {pos.direction === 'buy' ? (
                    <TrendingUp className="h-4 w-4 text-green-500" />
                  ) : (
                    <TrendingDown className="h-4 w-4 text-red-500" />
                  )}
                  <div>
                    <div className="font-medium">{pos.name || pos.epic}</div>
                    <div className="text-xs text-muted-foreground">
                      {pos.strategy} • {pos.size} lots @ {pos.entry_price}
                    </div>
                  </div>
                </div>
                <div className="text-right">
                  <div
                    className={`font-medium ${pos.unrealised_pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}
                  >
                    {pos.unrealised_pnl >= 0 ? '+' : ''}$
                    {pos.unrealised_pnl.toFixed(2)}
                  </div>
                  <div className="text-xs text-muted-foreground">
                    SL: {pos.stop_loss} | TP: {pos.take_profit || '—'}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

// ============================================
// Engine Signals Panel
// ============================================
interface EngineSignalsPanelProps {
  signals: EngineSignal[];
  onRefresh: () => void;
}

export function EngineSignalsPanel({
  signals,
  onRefresh,
}: EngineSignalsPanelProps) {
  const [showAll, setShowAll] = useState(false);
  const displayed = showAll ? signals : signals.slice(0, 10);

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-sm font-medium flex items-center justify-between">
          <span>Recent Signals ({signals.length})</span>
          <Button variant="ghost" size="sm" onClick={onRefresh}>
            <RefreshCw className="h-3 w-3" />
          </Button>
        </CardTitle>
      </CardHeader>
      <CardContent>
        {signals.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">
            No signals yet
          </p>
        ) : (
          <div className="space-y-2">
            {displayed.map((sig) => (
              <div
                key={`${sig.timestamp}-${sig.epic}-${sig.strategy}`}
                className="flex items-center justify-between p-2 rounded border text-xs"
              >
                <div className="flex items-center gap-2">
                  <Badge
                    variant={sig.direction === 'buy' ? 'default' : 'destructive'}
                    className="text-[10px] px-1.5"
                  >
                    {sig.direction.toUpperCase()}
                  </Badge>
                  <span className="font-medium">{sig.name || sig.epic}</span>
                  <span className="text-muted-foreground">{sig.strategy}</span>
                </div>
                <div className="flex items-center gap-2">
                  <Gauge className="h-3 w-3" />
                  <span>{sig.strength.toFixed(1)}</span>
                  {sig.was_executed ? (
                    <Badge
                      variant="outline"
                      className="text-[10px] text-green-600"
                    >
                      Executed
                    </Badge>
                  ) : sig.rejection_reason ? (
                    <Badge
                      variant="outline"
                      className="text-[10px] text-red-600"
                    >
                      {sig.rejection_reason}
                    </Badge>
                  ) : null}
                </div>
              </div>
            ))}
            {signals.length > 10 && (
              <Button
                variant="ghost"
                size="sm"
                className="w-full text-xs"
                onClick={() => setShowAll(!showAll)}
              >
                {showAll ? 'Show less' : `Show all ${signals.length} signals`}
              </Button>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
