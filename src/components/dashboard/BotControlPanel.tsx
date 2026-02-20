'use client';

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Slider } from '@/components/ui/slider';
import { Play, Square, Settings, Activity, AlertTriangle, CheckCircle2, Clock, Zap } from 'lucide-react';
import type { BotConfig, RiskConfig } from '@/types/ig';

interface BotControlPanelProps {
  status: 'STOPPED' | 'STARTING' | 'RUNNING' | 'STOPPING' | 'ERROR';
  config: BotConfig | null;
  stats: { signalsGenerated: number; tradesExecuted: number; uptime: number };
  loading?: boolean;
  onStart: () => Promise<{ success: boolean; error?: string }>;
  onStop: () => Promise<{ success: boolean; error?: string }>;
  onUpdateRiskConfig?: (config: Partial<RiskConfig>) => Promise<{ success: boolean }>;
}

function StatusBadge({ status }: { status: BotControlPanelProps['status'] }) {
  const variants: Record<typeof status, { color: string; icon: React.ReactNode }> = {
    STOPPED: { color: 'secondary', icon: <Square className="h-3 w-3" /> },
    STARTING: { color: 'default', icon: <Activity className="h-3 w-3 animate-pulse" /> },
    RUNNING: { color: 'default', icon: <CheckCircle2 className="h-3 w-3" /> },
    STOPPING: { color: 'secondary', icon: <Square className="h-3 w-3" /> },
    ERROR: { color: 'destructive', icon: <AlertTriangle className="h-3 w-3" /> },
  };
  const { color, icon } = variants[status];
  return (
    <Badge variant={color as 'default' | 'secondary' | 'destructive'} className="flex items-center gap-1">
      {icon}
      {status}
    </Badge>
  );
}

export function BotControlPanel({ status, config, stats, loading, onStart, onStop, onUpdateRiskConfig }: BotControlPanelProps) {
  const isRunning = status === 'RUNNING';
  const isStarting = status === 'STARTING';
  const isStopping = status === 'STOPPING';
  const isBusy = isStarting || isStopping || loading;

  const formatUptime = (ms: number) => {
    if (!ms) return '0m';
    const seconds = Math.floor((Date.now() - ms) / 1000);
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    if (hours > 0) return `${hours}h ${minutes}m`;
    return `${minutes}m`;
  };

  return (
    <Card className="w-full">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Zap className="h-5 w-5 text-primary" />
            <CardTitle className="text-lg">Bot Control</CardTitle>
          </div>
          <StatusBadge status={status} />
        </div>
        <CardDescription>Start, stop, and configure the trading bot</CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="flex gap-3">
          <Button onClick={onStart} disabled={isRunning || isBusy} className="flex-1" variant={isRunning ? 'outline' : 'default'}>
            {isStarting ? <Activity className="h-4 w-4 mr-2 animate-pulse" /> : <Play className="h-4 w-4 mr-2" />}
            {isStarting ? 'Starting...' : 'Start Bot'}
          </Button>
          <Button onClick={onStop} disabled={!isRunning || isBusy} variant="destructive" className="flex-1">
            {isStopping ? <Activity className="h-4 w-4 mr-2 animate-pulse" /> : <Square className="h-4 w-4 mr-2" />}
            {isStopping ? 'Stopping...' : 'Stop Bot'}
          </Button>
        </div>

        <div className="grid grid-cols-3 gap-4 p-4 bg-muted/50 rounded-lg">
          <div className="text-center">
            <div className="text-2xl font-bold text-primary">{stats.signalsGenerated}</div>
            <div className="text-xs text-muted-foreground">Signals</div>
          </div>
          <div className="text-center">
            <div className="text-2xl font-bold text-green-500">{stats.tradesExecuted}</div>
            <div className="text-xs text-muted-foreground">Trades</div>
          </div>
          <div className="text-center">
            <div className="text-2xl font-bold">{isRunning ? formatUptime(stats.uptime) : '-'}</div>
            <div className="text-xs text-muted-foreground">Uptime</div>
          </div>
        </div>

        {config && onUpdateRiskConfig && (
          <div className="space-y-4">
            <div className="flex items-center gap-2 text-sm font-medium">
              <Settings className="h-4 w-4" />
              Risk Settings
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor="maxPositionSize" className="text-xs">Max Position Size</Label>
                <Input id="maxPositionSize" type="number" step="0.1" min="0.1" max="10" value={config.riskConfig.maxPositionSize} onChange={(e) => onUpdateRiskConfig({ maxPositionSize: parseFloat(e.target.value) })} disabled={isRunning} className="h-8" />
              </div>
              <div className="space-y-2">
                <Label htmlFor="maxDailyTrades" className="text-xs">Max Daily Trades</Label>
                <Input id="maxDailyTrades" type="number" min="1" max="50" value={config.riskConfig.maxDailyTrades} onChange={(e) => onUpdateRiskConfig({ maxDailyTrades: parseInt(e.target.value) })} disabled={isRunning} className="h-8" />
              </div>
              <div className="space-y-2">
                <Label htmlFor="maxDailyLoss" className="text-xs">Max Daily Loss ($)</Label>
                <Input id="maxDailyLoss" type="number" min="10" step="10" value={config.riskConfig.maxDailyLoss} onChange={(e) => onUpdateRiskConfig({ maxDailyLoss: parseFloat(e.target.value) })} disabled={isRunning} className="h-8" />
              </div>
              <div className="space-y-2">
                <Label htmlFor="riskPerTrade" className="text-xs">Risk Per Trade (%)</Label>
                <Input id="riskPerTrade" type="number" step="0.1" min="0.1" max="5" value={config.riskConfig.riskPerTrade} onChange={(e) => onUpdateRiskConfig({ riskPerTrade: parseFloat(e.target.value) })} disabled={isRunning} className="h-8" />
              </div>
            </div>
          </div>
        )}

        {config && (
          <div className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
            <div className="flex items-center gap-2">
              <Clock className="h-4 w-4 text-muted-foreground" />
              <span className="text-sm">Trading Hours</span>
            </div>
            <span className="text-sm font-medium">{config.tradingHours.start} - {config.tradingHours.end}</span>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
