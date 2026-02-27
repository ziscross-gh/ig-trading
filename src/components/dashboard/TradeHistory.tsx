'use client';

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { History, TrendingUp, TrendingDown, BarChart2, LineChart } from 'lucide-react';
import type { TradeSignal, PerformanceMetrics } from '@/types/ig';
import { MARKET_NAMES } from '@/types/ig';
import { formatTimeSGT, formatDateSGT } from '@/lib/utils';
import { LineChart as RechartsLineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';

interface TradeHistoryProps {
  signals?: TradeSignal[];
  performance?: PerformanceMetrics;
  equityCurve?: Array<{ timestamp: string; pnl: number; cumulative_pnl: number; epic: string; strategy: string }>;
  mockMode?: boolean;
}

function PerformanceCard({ label, value, subValue, trend }: { label: string; value: string | number; subValue?: string; trend?: 'up' | 'down' | 'neutral' }) {
  const trendColors = { up: 'text-green-500', down: 'text-red-500', neutral: 'text-foreground' };
  return (
    <div className="text-center p-3 bg-muted/30 rounded-lg">
      <div className="text-xs text-muted-foreground mb-1">{label}</div>
      <div className={`text-xl font-bold ${trend ? trendColors[trend] : ''}`}>{value}</div>
      {subValue && <div className="text-xs text-muted-foreground">{subValue}</div>}
    </div>
  );
}

function generateMockTradeHistory(): { trades: Array<{ id: string; epic: string; direction: 'BUY' | 'SELL'; entry: number; exit: number; size: number; pnl: number; timestamp: Date; status: 'CLOSED' | 'OPEN' }>; performance: PerformanceMetrics } {
  const trades = [
    { id: 'trade-1', epic: 'CS.D.CFIGOLD.CFI.IP', direction: 'BUY' as const, entry: 2345.50, exit: 2352.25, size: 0.5, pnl: 33.75, timestamp: new Date(Date.now() - 3600000), status: 'CLOSED' as const },
    { id: 'trade-2', epic: 'CS.D.EURUSD.CFD', direction: 'SELL' as const, entry: 1.0875, exit: 1.0855, size: 1.0, pnl: 20.00, timestamp: new Date(Date.now() - 7200000), status: 'CLOSED' as const },
    { id: 'trade-3', epic: 'CS.D.CFIGOLD.CFI.IP', direction: 'SELL' as const, entry: 2360.00, exit: 2365.50, size: 0.5, pnl: -27.50, timestamp: new Date(Date.now() - 10800000), status: 'CLOSED' as const },
    { id: 'trade-4', epic: 'CS.D.GBPUSD.CFD', direction: 'BUY' as const, entry: 1.2680, exit: 1.2715, size: 0.8, pnl: 28.00, timestamp: new Date(Date.now() - 14400000), status: 'CLOSED' as const },
    { id: 'trade-5', epic: 'CS.D.CFIGOLD.CFI.IP', direction: 'BUY' as const, entry: 2340.00, exit: 2335.00, size: 0.3, pnl: -15.00, timestamp: new Date(Date.now() - 18000000), status: 'CLOSED' as const },
  ];

  const performance: PerformanceMetrics = {
    totalTrades: 5,
    winningTrades: 3,
    losingTrades: 2,
    winRate: 60,
    totalPnl: 39.25,
    avgWin: 27.25,
    avgLoss: 21.25,
    profitFactor: 1.28,
    sharpeRatio: 0.85,
    maxDrawdown: 2.5,
    dailyPnl: [
      { date: new Date(Date.now() - 86400000 * 4).toDateString(), pnl: 15.50 },
      { date: new Date(Date.now() - 86400000 * 3).toDateString(), pnl: -12.00 },
      { date: new Date(Date.now() - 86400000 * 2).toDateString(), pnl: 28.75 },
      { date: new Date(Date.now() - 86400000).toDateString(), pnl: 7.00 },
      { date: new Date().toDateString(), pnl: 0 },
    ],
  };

  return { trades, performance };
}

export function TradeHistory({ performance, equityCurve, mockMode = true }: TradeHistoryProps) {
  const mockData = mockMode ? generateMockTradeHistory() : null;
  const trades = mockData?.trades || [];
  const perf = mockData?.performance || performance;

  // Format equity curve data for recharts
  const equityCurveData = equityCurve?.map((point) => ({
    time: new Date(point.timestamp).toLocaleTimeString('en-SG', { hour: '2-digit', minute: '2-digit' }),
    cumPnl: parseFloat(point.cumulative_pnl.toFixed(2)),
  })) || [];

  return (
    <Card className="w-full">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <History className="h-5 w-5 text-primary" />
            <CardTitle className="text-lg">Trade History</CardTitle>
          </div>
          {perf && (
            <Badge variant={perf.totalPnl >= 0 ? 'default' : 'destructive'}>
              Total P&L: {perf.totalPnl >= 0 ? '+' : ''}${perf.totalPnl.toFixed(2)}
            </Badge>
          )}
        </div>
        <CardDescription>Past trades and performance metrics</CardDescription>
      </CardHeader>
      <CardContent>
        <Tabs defaultValue="trades" className="w-full">
          <TabsList className="grid w-full grid-cols-3 mb-4">
            <TabsTrigger value="trades">Trades</TabsTrigger>
            <TabsTrigger value="performance">Performance</TabsTrigger>
            <TabsTrigger value="equity">Equity Curve</TabsTrigger>
          </TabsList>

          <TabsContent value="trades">
            {trades.length === 0 ? (
              <div className="text-center text-muted-foreground py-8">No trade history available</div>
            ) : (
              <div className="rounded-lg border overflow-hidden">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Market</TableHead>
                      <TableHead>Side</TableHead>
                      <TableHead>Entry</TableHead>
                      <TableHead>Exit</TableHead>
                      <TableHead>Size</TableHead>
                      <TableHead>P&L</TableHead>
                      <TableHead>Time</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {trades.map((trade) => (
                      <TableRow key={trade.id}>
                        <TableCell className="font-medium">{MARKET_NAMES[trade.epic] || trade.epic}</TableCell>
                        <TableCell>
                          <Badge variant={trade.direction === 'BUY' ? 'default' : 'secondary'} className={`flex items-center gap-1 w-fit ${trade.direction === 'BUY' ? 'bg-green-500/10 text-green-500' : 'bg-red-500/10 text-red-500'}`}>
                            {trade.direction === 'BUY' ? <TrendingUp className="h-3 w-3" /> : <TrendingDown className="h-3 w-3" />}
                            {trade.direction}
                          </Badge>
                        </TableCell>
                        <TableCell className="font-mono text-sm">{trade.entry.toFixed(trade.entry < 10 ? 4 : 2)}</TableCell>
                        <TableCell className="font-mono text-sm">{trade.exit.toFixed(trade.exit < 10 ? 4 : 2)}</TableCell>
                        <TableCell className="font-mono text-sm">{trade.size.toFixed(2)}</TableCell>
                        <TableCell className={`font-mono font-semibold ${trade.pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>{trade.pnl >= 0 ? '+' : ''}${trade.pnl.toFixed(2)}</TableCell>
                        <TableCell className="text-sm text-muted-foreground">{formatTimeSGT(trade.timestamp)}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            )}
          </TabsContent>

          <TabsContent value="performance">
            {perf ? (
              <div className="space-y-4">
                <div className="grid grid-cols-4 gap-3">
                  <PerformanceCard label="Win Rate" value={`${perf.winRate.toFixed(1)}%`} subValue={`${perf.winningTrades}/${perf.totalTrades} trades`} trend={perf.winRate >= 50 ? 'up' : 'down'} />
                  <PerformanceCard label="Profit Factor" value={perf.profitFactor.toFixed(2)} trend={perf.profitFactor >= 1 ? 'up' : 'down'} />
                  <PerformanceCard label="Avg Win" value={`$${perf.avgWin.toFixed(2)}`} trend="up" />
                  <PerformanceCard label="Avg Loss" value={`$${perf.avgLoss.toFixed(2)}`} trend="down" />
                </div>
                <div className="grid grid-cols-3 gap-3">
                  <PerformanceCard label="Total Trades" value={perf.totalTrades} />
                  <PerformanceCard label="Sharpe Ratio" value={perf.sharpeRatio.toFixed(2)} trend={perf.sharpeRatio >= 1 ? 'up' : perf.sharpeRatio >= 0.5 ? 'neutral' : 'down'} />
                  <PerformanceCard label="Max Drawdown" value={`${perf.maxDrawdown.toFixed(1)}%`} trend={perf.maxDrawdown <= 5 ? 'up' : perf.maxDrawdown <= 10 ? 'neutral' : 'down'} />
                </div>
                <div className="pt-4 border-t">
                  <div className="flex items-center gap-2 mb-3">
                    <BarChart2 className="h-4 w-4 text-muted-foreground" />
                    <span className="text-sm font-medium">Daily P&L</span>
                  </div>
                  <div className="flex items-end gap-1 h-20">
                    {perf.dailyPnl.map((day, index) => (
                      <div key={index} className="flex-1 flex flex-col items-center">
                        <div className={`w-full rounded-t transition-all ${day.pnl >= 0 ? 'bg-green-500' : 'bg-red-500'}`} style={{ height: `${Math.min(Math.abs(day.pnl) * 2, 60)}px` }} />
                        <span className="text-[10px] text-muted-foreground mt-1">{formatDateSGT(day.date)}</span>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            ) : (
              <div className="text-center text-muted-foreground py-8">No performance data available</div>
            )}
          </TabsContent>

          <TabsContent value="equity">
            {equityCurveData && equityCurveData.length > 0 ? (
              <div className="space-y-4">
                <div className="flex items-center gap-2">
                  <LineChart className="h-4 w-4 text-muted-foreground" />
                  <span className="text-sm font-medium">Cumulative P&L Over Time</span>
                </div>
                <div className="w-full h-64 bg-muted/10 rounded-lg p-4">
                  <ResponsiveContainer width="100%" height="100%">
                    <RechartsLineChart data={equityCurveData}>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis dataKey="time" />
                      <YAxis />
                      <Tooltip
                        contentStyle={{ backgroundColor: '#1a1a1a', border: '1px solid #333' }}
                        formatter={(value: number) => `$${value.toFixed(2)}`}
                        labelFormatter={(label) => `Time: ${label}`}
                      />
                      <Line
                        type="monotone"
                        dataKey="cumPnl"
                        stroke="#10b981"
                        dot={false}
                        strokeWidth={2}
                        isAnimationActive={false}
                      />
                    </RechartsLineChart>
                  </ResponsiveContainer>
                </div>
                <div className="grid grid-cols-3 gap-3 pt-2">
                  <div className="text-center p-3 bg-muted/30 rounded-lg">
                    <div className="text-xs text-muted-foreground mb-1">Starting P&L</div>
                    <div className="text-lg font-bold">$0.00</div>
                  </div>
                  <div className="text-center p-3 bg-muted/30 rounded-lg">
                    <div className="text-xs text-muted-foreground mb-1">Current P&L</div>
                    <div className={`text-lg font-bold ${equityCurveData[equityCurveData.length - 1]?.cumPnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                      ${equityCurveData[equityCurveData.length - 1]?.cumPnl.toFixed(2) || '0.00'}
                    </div>
                  </div>
                  <div className="text-center p-3 bg-muted/30 rounded-lg">
                    <div className="text-xs text-muted-foreground mb-1">Trades</div>
                    <div className="text-lg font-bold">{equityCurveData.length}</div>
                  </div>
                </div>
              </div>
            ) : (
              <div className="text-center text-muted-foreground py-8">No equity curve data available</div>
            )}
          </TabsContent>
        </Tabs>
      </CardContent>
    </Card>
  );
}
