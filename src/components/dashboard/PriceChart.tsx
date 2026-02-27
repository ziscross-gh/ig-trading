'use client';

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Button } from '@/components/ui/button';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, ComposedChart, Area } from 'recharts';
import { BarChart3, RefreshCw } from 'lucide-react';
import type { Candle } from '@/types/ig';
import { MARKET_NAMES } from '@/types/ig';
import { useState } from 'react';

interface PriceChartProps {
  candles: Candle[];
  epic: string;
  loading?: boolean;
  onRefresh?: () => void;
  livePrice?: { bid: number; offer: number; timestamp?: string };
}

function formatPrice(value: number, epic: string): string {
  if (epic.includes('GOLD')) return value.toFixed(2);
  if (epic.includes('JPY')) return value.toFixed(3);
  return value.toFixed(5);
}

function formatTime(timestamp: string): string {
  return new Date(timestamp).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
}

export function PriceChart({ candles, epic, loading, onRefresh, livePrice }: PriceChartProps) {
  const [chartType, setChartType] = useState<'line' | 'area'>('line');

  // Merge historical candles with live tick so chart matches MarketOverview
  const mergedCandles = [...candles];
  if (livePrice && livePrice.bid > 0) {
    const mid = (livePrice.bid + livePrice.offer) / 2;
    const lastCandle = candles[candles.length - 1];
    const ts = livePrice.timestamp || new Date().toISOString();
    // If live tick is newer than last candle, append as virtual candle
    if (!lastCandle || new Date(ts).getTime() > new Date(lastCandle.timestamp).getTime()) {
      mergedCandles.push({
        open: lastCandle ? lastCandle.close : mid,
        high: Math.max(lastCandle ? lastCandle.high : mid, mid),
        low: Math.min(lastCandle ? lastCandle.low : mid, mid),
        close: mid,
        volume: 0,
        timestamp: ts,
      });
    }
  }

  const chartData = mergedCandles.map((candle) => ({
    time: formatTime(candle.timestamp),
    open: candle.open,
    high: candle.high,
    low: candle.low,
    close: candle.close,
    volume: candle.volume,
  }));

  const priceDomain = mergedCandles.length > 0 ? [Math.min(...mergedCandles.map((c) => c.low)) * 0.999, Math.max(...mergedCandles.map((c) => c.high)) * 1.001] : ['auto', 'auto'];
  const currentPrice = livePrice && livePrice.bid > 0 ? (livePrice.bid + livePrice.offer) / 2 : (mergedCandles.length > 0 ? mergedCandles[mergedCandles.length - 1]?.close || 0 : 0);
  const previousPrice = candles.length > 1 ? candles[candles.length - 2]?.close || currentPrice : (candles.length > 0 ? candles[0]?.close || currentPrice : currentPrice);
  const priceChange = currentPrice - previousPrice;
  const isPositive = priceChange >= 0;

  return (
    <Card className="w-full">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <BarChart3 className="h-5 w-5 text-primary" />
            <div>
              <CardTitle className="text-lg">{MARKET_NAMES[epic] || epic}</CardTitle>
              <div className="flex items-center gap-2 mt-1">
                <span className="text-2xl font-bold">{formatPrice(currentPrice, epic)}</span>
                <span className={`text-sm font-medium ${isPositive ? 'text-green-500' : 'text-red-500'}`}>{isPositive ? '+' : ''}{formatPrice(priceChange, epic)}</span>
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Tabs value={chartType} onValueChange={(v) => setChartType(v as typeof chartType)}>
              <TabsList className="grid grid-cols-2 h-8">
                <TabsTrigger value="line" className="text-xs px-2">Line</TabsTrigger>
                <TabsTrigger value="area" className="text-xs px-2">Area</TabsTrigger>
              </TabsList>
            </Tabs>
            {onRefresh && (
              <Button variant="ghost" size="icon" onClick={onRefresh} disabled={loading}>
                <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
              </Button>
            )}
          </div>
        </div>
        <CardDescription>{mergedCandles.length} data points • Last update: {mergedCandles.length > 0 ? new Date(mergedCandles[mergedCandles.length - 1].timestamp).toLocaleString() : 'N/A'}</CardDescription>
      </CardHeader>
      <CardContent>
        {mergedCandles.length === 0 ? (
          <div className="h-[300px] flex items-center justify-center text-muted-foreground">{loading ? 'Loading chart data...' : 'No data available'}</div>
        ) : (
          <div className="h-[300px] w-full">
            <ResponsiveContainer width="100%" height="100%">
              {chartType === 'line' ? (
                <LineChart data={chartData}>
                  <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                  <XAxis dataKey="time" stroke="hsl(var(--muted-foreground))" fontSize={10} tickLine={false} />
                  <YAxis domain={priceDomain} stroke="hsl(var(--muted-foreground))" fontSize={10} tickLine={false} tickFormatter={(value) => formatPrice(value, epic)} />
                  <Tooltip contentStyle={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: '8px' }} labelStyle={{ color: 'hsl(var(--foreground))' }} formatter={(value: number) => [formatPrice(value, epic), 'Price']} />
                  <Line type="monotone" dataKey="close" stroke="#3b82f6" strokeWidth={2} dot={false} activeDot={{ r: 4, fill: '#3b82f6' }} />
                </LineChart>
              ) : (
                <ComposedChart data={chartData}>
                  <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                  <XAxis dataKey="time" stroke="hsl(var(--muted-foreground))" fontSize={10} tickLine={false} />
                  <YAxis domain={priceDomain} stroke="hsl(var(--muted-foreground))" fontSize={10} tickLine={false} tickFormatter={(value) => formatPrice(value, epic)} />
                  <Tooltip contentStyle={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: '8px' }} labelStyle={{ color: 'hsl(var(--foreground))' }} formatter={(value: number) => [formatPrice(value, epic), 'Price']} />
                  <Area type="monotone" dataKey="close" stroke="#3b82f6" strokeWidth={2} fill="rgba(59, 130, 246, 0.1)" />
                </ComposedChart>
              )}
            </ResponsiveContainer>
          </div>
        )}
        {mergedCandles.length > 0 && mergedCandles[mergedCandles.length - 1] && (
          <div className="grid grid-cols-4 gap-4 mt-4 pt-4 border-t">
            <div className="text-center">
              <div className="text-xs text-muted-foreground">Open</div>
              <div className="font-semibold">{formatPrice(mergedCandles[mergedCandles.length - 1]?.open || 0, epic)}</div>
            </div>
            <div className="text-center">
              <div className="text-xs text-muted-foreground">High</div>
              <div className="font-semibold text-green-500">{formatPrice(mergedCandles[mergedCandles.length - 1]?.high || 0, epic)}</div>
            </div>
            <div className="text-center">
              <div className="text-xs text-muted-foreground">Low</div>
              <div className="font-semibold text-red-500">{formatPrice(mergedCandles[mergedCandles.length - 1]?.low || 0, epic)}</div>
            </div>
            <div className="text-center">
              <div className="text-xs text-muted-foreground">Close</div>
              <div className="font-semibold">{formatPrice(mergedCandles[mergedCandles.length - 1]?.close || 0, epic)}</div>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
