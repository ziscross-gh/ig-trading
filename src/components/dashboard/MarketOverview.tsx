'use client';

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { TrendingUp, TrendingDown, Activity, BarChart3 } from 'lucide-react';
import type { Market } from '@/types/ig';
import { MARKET_NAMES } from '@/types/ig';

interface MarketOverviewProps {
  markets: Market[];
  selectedEpic: string;
  onSelectMarket: (epic: string) => void;
  loading?: boolean;
}

function MarketCard({ market, isSelected, onClick }: { market: Market; isSelected: boolean; onClick: () => void }) {
  const isPositive = market.change >= 0;
  const spread = market.offer - market.bid;

  return (
    <div onClick={onClick} className={`p-4 rounded-lg border cursor-pointer transition-all hover:shadow-md ${isSelected ? 'border-primary bg-primary/5 ring-2 ring-primary/20' : 'border-border hover:border-primary/50'}`}>
      <div className="flex items-center justify-between mb-2">
        <span className="font-semibold text-sm">{MARKET_NAMES[market.epic] || market.name}</span>
        <Badge variant={market.marketStatus === 'TRADEABLE' ? 'default' : 'secondary'}>{market.marketStatus}</Badge>
      </div>
      <div className="flex items-center gap-2 mb-1">
        <span className="text-2xl font-bold">{(market.bid || 0).toFixed((market.bid || 0) < 10 ? 4 : 2)}</span>
        <div className={`flex items-center gap-1 ${isPositive ? 'text-green-500' : 'text-red-500'}`}>
          {isPositive ? <TrendingUp className="h-4 w-4" /> : <TrendingDown className="h-4 w-4" />}
          <span className="text-sm font-medium">{isPositive ? '+' : ''}{(market.change || 0).toFixed(2)}</span>
          <span className="text-xs">({isPositive ? '+' : ''}{(market.changePercent || 0).toFixed(2)}%)</span>
        </div>
      </div>
      <div className="flex items-center gap-4 text-xs text-muted-foreground">
        <span>Spread: {spread.toFixed(Math.abs(spread) < 0.01 ? 4 : 2)}</span>
        <span>H: {(market.high || 0).toFixed((market.high || 0) < 10 ? 4 : 2)}</span>
        <span>L: {(market.low || 0).toFixed((market.low || 0) < 10 ? 4 : 2)}</span>
      </div>
    </div>
  );
}

export function MarketOverview({ markets, selectedEpic, onSelectMarket, loading }: MarketOverviewProps) {
  return (
    <Card className="w-full">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <BarChart3 className="h-5 w-5 text-primary" />
            <CardTitle className="text-lg">Market Overview</CardTitle>
          </div>
          {loading && <Activity className="h-4 w-4 animate-spin text-muted-foreground" />}
        </div>
        <CardDescription>Real-time prices for Gold and FX pairs</CardDescription>
      </CardHeader>
      <CardContent>
        {markets.length === 0 ? (
          <div className="text-center text-muted-foreground py-8">No market data available</div>
        ) : (
          <div className="space-y-3">
            {markets.filter(market => market && market.epic).map((market) => (
              <MarketCard key={market.epic} market={market} isSelected={selectedEpic === market.epic} onClick={() => onSelectMarket(market.epic)} />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
