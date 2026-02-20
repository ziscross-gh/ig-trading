'use client';

import { useState, useCallback } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Play, Loader2, Activity, Target, AlertTriangle } from 'lucide-react';

interface BacktestResult {
  totalTrades: number;
  winningTrades: number;
  losingTrades: number;
  winRate: number;
  totalPnl: number;
  totalPnlPercent: number;
  profitFactor: number;
  maxDrawdownPercent: number;
  sharpeRatio: number;
}

const MARKET_OPTIONS = [
  { value: 'CS.D.GOLDUSD.CFD', label: 'Gold (XAU/USD)' },
  { value: 'CS.D.EURUSD.CFD', label: 'EUR/USD' },
];

export function BacktestingPanel() {
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<BacktestResult | null>(null);
  const [config, setConfig] = useState({
    epic: 'CS.D.GOLDUSD.CFD',
    initialCapital: 10000,
    riskPerTrade: 1,
    stopLossPercent: 1.5,
    takeProfitPercent: 3,
    dataPoints: 500,
  });

  const runBacktest = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetch('/api/backtest', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          epic: config.epic,
          generateMockData: true,
          mockDataCount: config.dataPoints,
          config: {
            initialCapital: config.initialCapital,
            positionSizeMode: 'risk_based',
            riskPerTrade: config.riskPerTrade,
            defaultStopLossPercent: config.stopLossPercent,
            defaultTakeProfitPercent: config.takeProfitPercent,
          },
        }),
      });
      const data = await response.json();
      if (data.success && data.result) {
        setResult({
          totalTrades: data.result.totalTrades,
          winningTrades: data.result.winningTrades,
          losingTrades: data.result.losingTrades,
          winRate: data.result.winRate,
          totalPnl: data.result.totalPnl,
          totalPnlPercent: data.result.totalPnlPercent,
          profitFactor: data.result.profitFactor,
          maxDrawdownPercent: data.result.maxDrawdownPercent,
          sharpeRatio: data.result.sharpeRatio,
        });
      }
    } catch (error) {
      console.error('Backtest failed:', error);
    } finally {
      setLoading(false);
    }
  }, [config]);

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Activity className="h-5 w-5" />Backtest Configuration</CardTitle>
          <CardDescription>Test strategies on historical data</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="space-y-2">
              <Label>Market</Label>
              <Select value={config.epic} onValueChange={(v) => setConfig((p) => ({ ...p, epic: v }))}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>{MARKET_OPTIONS.map((opt) => (<SelectItem key={opt.value} value={opt.value}>{opt.label}</SelectItem>))}</SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label>Initial Capital ($)</Label>
              <Input type="number" value={config.initialCapital} onChange={(e) => setConfig((p) => ({ ...p, initialCapital: Number(e.target.value) }))} />
            </div>
            <div className="space-y-2">
              <Label>Data Points</Label>
              <Input type="number" value={config.dataPoints} onChange={(e) => setConfig((p) => ({ ...p, dataPoints: Number(e.target.value) }))} />
            </div>
            <div className="space-y-2">
              <Label>Risk Per Trade (%)</Label>
              <Input type="number" step="0.1" value={config.riskPerTrade} onChange={(e) => setConfig((p) => ({ ...p, riskPerTrade: Number(e.target.value) }))} />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label>Stop Loss (%)</Label>
              <Input type="number" step="0.1" value={config.stopLossPercent} onChange={(e) => setConfig((p) => ({ ...p, stopLossPercent: Number(e.target.value) }))} />
            </div>
            <div className="space-y-2">
              <Label>Take Profit (%)</Label>
              <Input type="number" step="0.1" value={config.takeProfitPercent} onChange={(e) => setConfig((p) => ({ ...p, takeProfitPercent: Number(e.target.value) }))} />
            </div>
          </div>
          <Button onClick={runBacktest} disabled={loading} className="w-full">
            {loading ? <><Loader2 className="h-4 w-4 mr-2 animate-spin" />Running Backtest...</> : <><Play className="h-4 w-4 mr-2" />Run Backtest</>}
          </Button>
        </CardContent>
      </Card>

      {result && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <Card><CardContent className="pt-4"><div className="text-sm text-muted-foreground">Total P&L</div><div className={`text-2xl font-bold ${result.totalPnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>{result.totalPnl >= 0 ? '+' : ''}${result.totalPnl.toFixed(2)}</div><div className="text-xs text-muted-foreground">{result.totalPnlPercent >= 0 ? '+' : ''}{result.totalPnlPercent.toFixed(2)}%</div></CardContent></Card>
          <Card><CardContent className="pt-4"><div className="text-sm text-muted-foreground">Win Rate</div><div className="text-2xl font-bold">{result.winRate.toFixed(1)}%</div><div className="text-xs text-muted-foreground">{result.winningTrades}W / {result.losingTrades}L</div></CardContent></Card>
          <Card><CardContent className="pt-4"><div className="text-sm text-muted-foreground">Profit Factor</div><div className="text-2xl font-bold">{result.profitFactor.toFixed(2)}</div><div className="text-xs text-muted-foreground">Sharpe: {result.sharpeRatio.toFixed(2)}</div></CardContent></Card>
          <Card><CardContent className="pt-4"><div className="text-sm text-muted-foreground">Max Drawdown</div><div className="text-2xl font-bold text-red-500">-{result.maxDrawdownPercent.toFixed(1)}%</div></CardContent></Card>
        </div>
      )}
    </div>
  );
}
