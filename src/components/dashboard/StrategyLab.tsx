'use client';

import { useState, useEffect } from 'react';
import { useEngine } from '@/hooks/useEngine';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Slider } from '@/components/ui/slider';
import { Brain, Play, RefreshCcw, TrendingUp, Shield, FlaskConical, CheckCircle2, History, BarChart3 } from 'lucide-react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import type { BacktestResultFull } from '@/hooks/useEngine';

export function StrategyLab() {
    const engine = useEngine();
    const [epic, setEpic] = useState('CS.D.CFIGOLD.CFI.IP');

    // Optimization State
    const [shortRange, setShortRange] = useState<[number, number]>([5, 20]);
    const [longRange, setLongRange] = useState<[number, number]>([20, 50]);
    const [isOptimizing, setIsOptimizing] = useState(false);
    const [pollInterval, setPollInterval] = useState<NodeJS.Timeout | null>(null);

    // Backtest State
    const [backtestStrategy, setBacktestStrategy] = useState('ma_crossover');
    const [backtestBalance, setBacktestBalance] = useState(10000);
    const [backtestRisk, setBacktestRisk] = useState(1.0);
    const [isBacktesting, setIsBacktesting] = useState(false);
    const [backtestResult, setBacktestResult] = useState<BacktestResultFull | null>(null);

    const startOptimization = async () => {
        setIsOptimizing(true);
        try {
            await engine.startOptimization({
                epic,
                short_range: shortRange,
                long_range: longRange,
            });

            // Start polling for results
            const interval = setInterval(async () => {
                const data = await engine.fetchOptimizationResults();
                if (data.success && data.result) {
                    setIsOptimizing(false);
                    // clearInterval(interval);
                }
            }, 2000);
            setPollInterval(interval);
        } catch {
            setIsOptimizing(false);
        }
    };

    const runBacktest = async () => {
        setIsBacktesting(true);
        setBacktestResult(null);
        try {
            const data = await engine.runBacktest({
                epic,
                strategy_name: backtestStrategy,
                initial_balance: backtestBalance,
                risk_pct: backtestRisk
            });
            if (data.success && 'result' in data) {
                setBacktestResult(data.result);
            } else {
                const errorMsg = 'error' in data ? data.error : 'Backtest failed';
                alert(errorMsg);
            }
        } catch (err: unknown) {
            console.error(err);
        } finally {
            setIsBacktesting(false);
        }
    };

    useEffect(() => {
        return () => {
            if (pollInterval) clearInterval(pollInterval);
        };
    }, [pollInterval]);

    const applyConfiguration = async (params: string) => {
        // Extract values from string like "Short: 12, Long: 26, ADX: 25.0"
        const shortMatch = params.match(/Short: (\d+)/);
        const longMatch = params.match(/Long: (\d+)/);
        const adxMatch = params.match(/ADX: ([\d.]+)/);

        if (shortMatch && longMatch && adxMatch) {
            const short_period = parseInt(shortMatch[1]);
            const long_period = parseInt(longMatch[1]);
            const require_adx_above = parseFloat(adxMatch[1]);

            console.log('Applying optimization:', { short_period, long_period, require_adx_above });

            const result = await engine.updateStrategy('ma_crossover', {
                short_period,
                long_period,
                require_adx_above
            });

            if (result.success) {
                alert(`Successfully applied configuration: ${params}`);
            } else {
                alert(`Failed to apply configuration: ${result.error}`);
            }
        }
    };

    return (
        <div className="space-y-6">
            <Tabs defaultValue="optimize" className="w-full">
                <TabsList className="grid w-full grid-cols-2 max-w-[400px]">
                    <TabsTrigger value="optimize" className="flex items-center gap-2">
                        <Brain className="h-4 w-4" />
                        Optimization
                    </TabsTrigger>
                    <TabsTrigger value="backtest" className="flex items-center gap-2">
                        <History className="h-4 w-4" />
                        Backtesting
                    </TabsTrigger>
                </TabsList>

                <TabsContent value="optimize" className="space-y-6 mt-6">
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                        {/* Configuration Panel */}
                        <Card className="md:col-span-1 border-blue-100 shadow-lg shadow-blue-500/5">
                            <CardHeader>
                                <CardTitle className="flex items-center gap-2 text-blue-700">
                                    <FlaskConical className="h-5 w-5" />
                                    Optimization Setup
                                </CardTitle>
                                <CardDescription>Define parameter ranges for grid search</CardDescription>
                            </CardHeader>
                            <CardContent className="space-y-6">
                                <div className="space-y-2">
                                    <Label>Market</Label>
                                    <select
                                        value={epic}
                                        onChange={(e) => setEpic(e.target.value)}
                                        className="w-full p-2 border rounded-md text-sm bg-white"
                                    >
                                        <option value="CS.D.CFIGOLD.CFI.IP">Gold (XAU/USD)</option>
                                        <option value="CS.D.EURUSD.CSD.IP">EUR/USD</option>
                                        <option value="CS.D.GBPUSD.CSD.IP">GBP/USD</option>
                                    </select>
                                </div>

                                <div className="space-y-4">
                                    <div className="flex justify-between items-center">
                                        <Label className="text-gray-600">Short EMA Range ({shortRange[0]} - {shortRange[1]})</Label>
                                    </div>
                                    <Slider
                                        min={2}
                                        max={30}
                                        step={1}
                                        value={shortRange}
                                        onValueChange={(val) => setShortRange(val as [number, number])}
                                    />
                                </div>

                                <div className="space-y-4">
                                    <div className="flex justify-between items-center">
                                        <Label className="text-gray-600">Long EMA Range ({longRange[0]} - {longRange[1]})</Label>
                                    </div>
                                    <Slider
                                        min={10}
                                        max={100}
                                        step={1}
                                        value={longRange}
                                        onValueChange={(val) => setLongRange(val as [number, number])}
                                    />
                                </div>

                                <Button
                                    className="w-full bg-blue-600 hover:bg-blue-700 shadow-md shadow-blue-200"
                                    onClick={startOptimization}
                                    disabled={isOptimizing}
                                >
                                    {isOptimizing ? (
                                        <>
                                            <RefreshCcw className="h-4 w-4 mr-2 animate-spin" />
                                            Optimizing...
                                        </>
                                    ) : (
                                        <>
                                            <Brain className="h-4 w-4 mr-2" />
                                            Run Optimizer
                                        </>
                                    )}
                                </Button>
                            </CardContent>
                        </Card>

                        {/* Results Panel */}
                        <Card className="md:col-span-2 border-emerald-100 shadow-lg shadow-emerald-500/5">
                            <CardHeader>
                                <CardTitle className="flex items-center gap-2 text-emerald-700">
                                    <TrendingUp className="h-5 w-5" />
                                    Optimization Results
                                </CardTitle>
                                <CardDescription>Top performing strategy configurations</CardDescription>
                            </CardHeader>
                            <CardContent>
                                {!engine.optimizationResult && !isOptimizing && (
                                    <div className="text-center py-12 text-gray-400">
                                        <Brain className="h-12 w-12 mx-auto mb-3 opacity-20" />
                                        <p>No results yet. Run the optimizer to see ideal settings.</p>
                                    </div>
                                )}

                                {isOptimizing && (
                                    <div className="space-y-4 py-8 text-center text-blue-600">
                                        <div className="relative w-16 h-16 mx-auto">
                                            <div className="absolute inset-0 border-4 border-blue-100 rounded-full"></div>
                                            <div className="absolute inset-0 border-4 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
                                        </div>
                                        <p className="text-sm font-medium">Simulating thousands of trades...</p>
                                    </div>
                                )}

                                {engine.optimizationResult && (
                                    <div className="space-y-4">
                                        <div className="p-4 bg-emerald-50 border border-emerald-100 rounded-xl flex items-center justify-between">
                                            <div className="flex items-center gap-3">
                                                <div className="bg-emerald-500 p-2 rounded-lg shadow-sm">
                                                    <Shield className="h-5 w-5 text-white" />
                                                </div>
                                                <div>
                                                    <p className="text-xs font-bold text-emerald-600 uppercase">Recommended config</p>
                                                    <p className="text-sm font-mono font-bold text-emerald-900">{engine.optimizationResult.best_parameters}</p>
                                                </div>
                                            </div>
                                            <div className="text-right">
                                                <p className="text-xs text-emerald-600 font-medium">PnL / Sharpe</p>
                                                <p className="text-lg font-bold text-emerald-700">
                                                    +${engine.optimizationResult.best_pnl.toFixed(2)}
                                                    <span className="text-xs ml-2 opacity-60">SR: {engine.optimizationResult.top_runs[0].result.sharpe_ratio.toFixed(2)}</span>
                                                </p>
                                            </div>
                                        </div>

                                        <div className="border rounded-lg overflow-hidden border-gray-100">
                                            <table className="w-full text-sm">
                                                <thead className="bg-gray-50 border-b">
                                                    <tr>
                                                        <th className="px-4 py-2 text-left text-gray-500">Parameters</th>
                                                        <th className="px-4 py-2 text-center text-gray-500">Win Rate</th>
                                                        <th className="px-4 py-2 text-center text-gray-500">PnL %</th>
                                                        <th className="px-4 py-2 text-center text-gray-500">Sharpe</th>
                                                        <th className="px-4 py-2 text-right text-gray-500">Action</th>
                                                    </tr>
                                                </thead>
                                                <tbody className="divide-y divide-gray-50">
                                                    {engine.optimizationResult.top_runs.map((run, i) => (
                                                        <tr key={i} className={i === 0 ? 'bg-emerald-50/30' : 'hover:bg-gray-50 transition-colors'}>
                                                            <td className="px-4 py-3 font-mono text-xs text-gray-700">{run.parameters}</td>
                                                            <td className="px-4 py-3 text-center">
                                                                <Badge variant="outline" className="font-bold border-gray-200">
                                                                    {run.result.win_rate.toFixed(1)}%
                                                                </Badge>
                                                            </td>
                                                            <td className={`px-4 py-3 text-center font-bold ${run.result.total_pnl >= 0 ? 'text-emerald-600' : 'text-red-600'}`}>
                                                                {run.result.total_pnl_pct.toFixed(2)}%
                                                            </td>
                                                            <td className="px-4 py-3 text-center text-gray-600">
                                                                {run.result.sharpe_ratio.toFixed(2)}
                                                            </td>
                                                            <td className="px-4 py-3 text-right">
                                                                <Button
                                                                    size="sm"
                                                                    variant="ghost"
                                                                    className="h-8 text-blue-600 hover:text-blue-700 hover:bg-blue-50"
                                                                    onClick={() => applyConfiguration(run.parameters)}
                                                                >
                                                                    <CheckCircle2 className="h-3 w-3 mr-1" />
                                                                    Apply
                                                                </Button>
                                                            </td>
                                                        </tr>
                                                    ))}
                                                </tbody>
                                            </table>
                                        </div>
                                    </div>
                                )}
                            </CardContent>
                        </Card>
                    </div>
                </TabsContent>

                <TabsContent value="backtest" className="space-y-6 mt-6">
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                        {/* Control Panel */}
                        <Card className="md:col-span-1 border-indigo-100 shadow-lg shadow-indigo-500/5">
                            <CardHeader>
                                <CardTitle className="flex items-center gap-2 text-indigo-700">
                                    <History className="h-5 w-5" />
                                    Backtest Run
                                </CardTitle>
                                <CardDescription>Run historical sim on current data</CardDescription>
                            </CardHeader>
                            <CardContent className="space-y-6">
                                <div className="space-y-2">
                                    <Label>Market</Label>
                                    <select
                                        value={epic}
                                        onChange={(e) => setEpic(e.target.value)}
                                        className="w-full p-2 border rounded-md text-sm bg-white"
                                    >
                                        <option value="CS.D.CFIGOLD.CFI.IP">Gold (XAU/USD)</option>
                                        <option value="CS.D.EURUSD.CSD.IP">EUR/USD</option>
                                        <option value="CS.D.GBPUSD.CSD.IP">GBP/USD</option>
                                    </select>
                                </div>

                                <div className="space-y-2">
                                    <Label>Strategy</Label>
                                    <select
                                        value={backtestStrategy}
                                        onChange={(e) => setBacktestStrategy(e.target.value)}
                                        className="w-full p-2 border rounded-md text-sm bg-white"
                                    >
                                        <option value="ma_crossover">MA Crossover</option>
                                        <option value="rsi_reversal">RSI Reversal</option>
                                        <option value="macd_momentum">MACD Momentum</option>
                                        <option value="bollinger">Bollinger Bands</option>
                                    </select>
                                </div>

                                <div className="grid grid-cols-2 gap-4">
                                    <div className="space-y-2">
                                        <Label>Balance ($)</Label>
                                        <Input
                                            type="number"
                                            value={backtestBalance}
                                            onChange={(e) => setBacktestBalance(Number(e.target.value))}
                                            className="h-9"
                                        />
                                    </div>
                                    <div className="space-y-2">
                                        <Label>Risk per Trade (%)</Label>
                                        <Input
                                            type="number"
                                            value={backtestRisk}
                                            onChange={(e) => setBacktestRisk(Number(e.target.value))}
                                            className="h-9"
                                        />
                                    </div>
                                </div>

                                <Button
                                    className="w-full bg-indigo-600 hover:bg-indigo-700 shadow-md shadow-indigo-200"
                                    onClick={runBacktest}
                                    disabled={isBacktesting}
                                >
                                    {isBacktesting ? (
                                        <>
                                            <RefreshCcw className="h-4 w-4 mr-2 animate-spin" />
                                            Running Backtest...
                                        </>
                                    ) : (
                                        <>
                                            <Play className="h-4 w-4 mr-2" />
                                            Start Backtest
                                        </>
                                    )}
                                </Button>
                            </CardContent>
                        </Card>

                        {/* Results Panel */}
                        <Card className="md:col-span-2 border-slate-100 shadow-lg shadow-slate-500/5">
                            <CardHeader>
                                <CardTitle className="flex items-center gap-2 text-slate-700">
                                    <BarChart3 className="h-5 w-5" />
                                    Backtest Results
                                </CardTitle>
                                <CardDescription>Performance metrics and trade history</CardDescription>
                            </CardHeader>
                            <CardContent>
                                {!backtestResult && !isBacktesting && (
                                    <div className="text-center py-12 text-gray-400">
                                        <History className="h-12 w-12 mx-auto mb-3 opacity-20" />
                                        <p>Select parameters and start backtest to see performance.</p>
                                    </div>
                                )}

                                {isBacktesting && (
                                    <div className="space-y-4 py-8 text-center text-indigo-600">
                                        <div className="relative w-16 h-16 mx-auto">
                                            <div className="absolute inset-0 border-4 border-indigo-100 rounded-full"></div>
                                            <div className="absolute inset-0 border-4 border-indigo-500 border-t-transparent rounded-full animate-spin"></div>
                                        </div>
                                        <p className="text-sm font-medium">Processing historical data...</p>
                                    </div>
                                )}

                                {backtestResult && (
                                    <div className="space-y-6">
                                        {/* Metrics Grid */}
                                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                                            <div className="p-3 bg-slate-50 rounded-lg border border-slate-100">
                                                <p className="text-xs text-slate-500 font-medium uppercase">Net PnL</p>
                                                <p className={`text-lg font-bold ${backtestResult.total_pnl >= 0 ? 'text-emerald-600' : 'text-red-600'}`}>
                                                    ${backtestResult.total_pnl.toFixed(2)}
                                                </p>
                                                <p className="text-[10px] text-slate-400">{backtestResult.total_pnl_pct.toFixed(2)}% return</p>
                                            </div>
                                            <div className="p-3 bg-slate-50 rounded-lg border border-slate-100">
                                                <p className="text-xs text-slate-500 font-medium uppercase">Win Rate</p>
                                                <p className="text-lg font-bold text-slate-800">{backtestResult.win_rate.toFixed(1)}%</p>
                                                <p className="text-[10px] text-slate-400">{backtestResult.winning_trades}W / {backtestResult.losing_trades}L</p>
                                            </div>
                                            <div className="p-3 bg-slate-50 rounded-lg border border-slate-100">
                                                <p className="text-xs text-slate-500 font-medium uppercase">Profit Factor</p>
                                                <p className="text-lg font-bold text-slate-800">{backtestResult.profit_factor.toFixed(2)}</p>
                                                <p className="text-[10px] text-slate-400">Risk/Reward efficiency</p>
                                            </div>
                                            <div className="p-3 bg-slate-50 rounded-lg border border-slate-100">
                                                <p className="text-xs text-slate-500 font-medium uppercase">Max DD</p>
                                                <p className="text-lg font-bold text-red-500">{backtestResult.max_drawdown_pct.toFixed(2)}%</p>
                                                <p className="text-[10px] text-slate-400">Peak-to-valley risk</p>
                                            </div>
                                        </div>

                                        {/* Trade List */}
                                        <div className="border rounded-lg overflow-hidden border-gray-100">
                                            <div className="bg-gray-50 px-4 py-2 border-b text-xs font-bold text-gray-500 flex justify-between">
                                                <span>SIMULATED TRADES</span>
                                                <span>{backtestResult.trades.length} EXECUTIONS</span>
                                            </div>
                                            <div className="max-h-[300px] overflow-y-auto">
                                                <table className="w-full text-sm">
                                                    <thead className="bg-gray-50/50 text-[10px] text-gray-400 sticky top-0 bg-white">
                                                        <tr>
                                                            <th className="px-4 py-2 text-left">TIME</th>
                                                            <th className="px-4 py-2 text-center">DIR</th>
                                                            <th className="px-4 py-2 text-center">ENTRY</th>
                                                            <th className="px-4 py-2 text-center">EXIT</th>
                                                            <th className="px-4 py-2 text-right">PNL</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody className="divide-y divide-gray-50">
                                                        {backtestResult.trades.map((trade, i) => (
                                                            <tr key={i} className="hover:bg-gray-50 transition-colors">
                                                                <td className="px-4 py-2 text-xs text-gray-400">
                                                                    {new Date(trade.entry_time * 1000).toLocaleDateString()}
                                                                </td>
                                                                <td className="px-4 py-2 text-center">
                                                                    <Badge className={trade.direction === 'BUY' ? 'bg-blue-100 text-blue-700' : 'bg-red-100 text-red-700'}>
                                                                        {trade.direction}
                                                                    </Badge>
                                                                </td>
                                                                <td className="px-4 py-2 text-center font-mono text-xs">{trade.entry_price.toFixed(5)}</td>
                                                                <td className="px-4 py-2 text-center font-mono text-xs">{trade.exit_price?.toFixed(5)}</td>
                                                                <td className={`px-4 py-2 text-right font-bold ${trade.pnl >= 0 ? 'text-emerald-600' : 'text-red-600'}`}>
                                                                    {trade.pnl >= 0 ? '+' : ''}{trade.pnl.toFixed(2)}
                                                                </td>
                                                            </tr>
                                                        ))}
                                                    </tbody>
                                                </table>
                                            </div>
                                        </div>
                                    </div>
                                )}
                            </CardContent>
                        </Card>
                    </div>
                </TabsContent>
            </Tabs>
        </div>
    );
}
