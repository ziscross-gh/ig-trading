'use client';

import { useState, useEffect, useCallback } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { TrendingUp, TrendingDown, Activity, Flame, Zap, RefreshCw, AlertTriangle } from 'lucide-react';
import { DEFAULT_MARKETS, MARKET_NAMES } from '@/types/ig';

interface MarketTrend {
  epic: string;
  name: string;
  trend: 'STRONG_BULLISH' | 'BULLISH' | 'NEUTRAL' | 'BEARISH' | 'STRONG_BEARISH';
  strength: number;
  changePercent: number;
  sentiment: 'GREED' | 'OPTIMISM' | 'NEUTRAL' | 'FEAR' | 'EXTREME_FEAR';
  sentimentScore: number;
  isTrending: boolean;
  signals: Array<{ name: string; value: string; signal: 'BUY' | 'SELL' | 'NEUTRAL' }>;
}

interface TrendingMarket {
  rank: number;
  epic: string;
  name: string;
  trend: MarketTrend['trend'];
  changePercent: number;
  hotness: number;
  reason: string;
}

function generateMockTrendData(): { trends: MarketTrend[]; sentimentScore: number; topMovers: TrendingMarket[] } {
  const trends: MarketTrend[] = [];
  const epics = Object.values(DEFAULT_MARKETS);
  
  epics.forEach((epic) => {
    const changePercent = (Math.random() - 0.5) * 2;
    const strength = Math.floor(Math.random() * 5) + 5;
    const sentimentScore = Math.random() * 100;
    
    let trend: MarketTrend['trend'];
    if (changePercent > 0.8) trend = 'STRONG_BULLISH';
    else if (changePercent > 0.3) trend = 'BULLISH';
    else if (changePercent < -0.8) trend = 'STRONG_BEARISH';
    else if (changePercent < -0.3) trend = 'BEARISH';
    else trend = 'NEUTRAL';

    let sentiment: MarketTrend['sentiment'];
    if (sentimentScore >= 70) sentiment = 'GREED';
    else if (sentimentScore >= 55) sentiment = 'OPTIMISM';
    else if (sentimentScore >= 45) sentiment = 'NEUTRAL';
    else if (sentimentScore >= 30) sentiment = 'FEAR';
    else sentiment = 'EXTREME_FEAR';

    trends.push({
      epic,
      name: MARKET_NAMES[epic] || epic,
      trend,
      strength,
      changePercent,
      sentiment,
      sentimentScore,
      isTrending: Math.abs(changePercent) > 0.3,
      signals: [
        { name: 'RSI', value: (30 + Math.random() * 40).toFixed(1), signal: Math.random() > 0.5 ? 'BUY' : 'SELL' },
        { name: 'MACD', value: (Math.random() * 0.01 - 0.005).toFixed(4), signal: Math.random() > 0.5 ? 'BUY' : 'SELL' },
      ],
    });
  });

  const topMovers: TrendingMarket[] = trends
    .filter((t) => t.isTrending)
    .sort((a, b) => Math.abs(b.changePercent) - Math.abs(a.changePercent))
    .slice(0, 5)
    .map((t, i) => ({
      rank: i + 1,
      epic: t.epic,
      name: t.name,
      trend: t.trend,
      changePercent: t.changePercent,
      hotness: Math.min(10, Math.abs(t.changePercent) * 3 + t.strength / 2),
      reason: t.trend.includes('BULLISH') ? 'Strong upward momentum' : 'Strong downward pressure',
    }));

  return { trends, sentimentScore: trends.reduce((sum, t) => sum + t.sentimentScore, 0) / trends.length, topMovers };
}

function SentimentGauge({ score }: { score: number }) {
  const getEmoji = () => {
    if (score >= 70) return '🚀';
    if (score >= 55) return '😊';
    if (score >= 45) return '😐';
    if (score >= 30) return '😰';
    return '😱';
  };
  const getColor = () => {
    if (score >= 70) return 'text-green-500';
    if (score >= 55) return 'text-emerald-500';
    if (score >= 45) return 'text-gray-500';
    if (score >= 30) return 'text-orange-500';
    return 'text-red-500';
  };
  return (
    <div className="text-center">
      <div className="text-3xl mb-1">{getEmoji()}</div>
      <div className={`text-2xl font-bold ${getColor()}`}>{Math.round(score)}</div>
      <div className="text-xs text-muted-foreground">Sentiment</div>
    </div>
  );
}

function TrendingItem({ item }: { item: TrendingMarket }) {
  const isUp = item.changePercent > 0;
  return (
    <div className="p-3 rounded-lg hover:bg-muted/50 cursor-pointer transition-all border border-transparent hover:border-primary/20">
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground text-xs">#{item.rank} Trending</span>
            {item.hotness >= 7 && <Flame className="h-3 w-3 text-orange-500" />}
          </div>
          <div className="font-semibold text-sm mt-0.5">{item.name}</div>
          <div className="text-xs text-muted-foreground mt-0.5">{item.reason}</div>
        </div>
        <div className="text-right">
          <div className={`flex items-center gap-1 text-sm font-semibold ${isUp ? 'text-green-500' : 'text-red-500'}`}>
            {isUp ? <TrendingUp className="h-4 w-4" /> : <TrendingDown className="h-4 w-4" />}
            {isUp ? '+' : ''}{item.changePercent.toFixed(2)}%
          </div>
        </div>
      </div>
      <div className="mt-2 flex items-center gap-2">
        <span className="text-xs text-muted-foreground">Heat</span>
        <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden">
          <div className={`h-full rounded-full ${item.hotness >= 7 ? 'bg-orange-500' : item.hotness >= 4 ? 'bg-yellow-500' : 'bg-blue-500'}`} style={{ width: `${item.hotness * 10}%` }} />
        </div>
        <span className="text-xs font-medium">{item.hotness.toFixed(1)}</span>
      </div>
    </div>
  );
}

export function TrendFilterPanel() {
  // Initialize with lazy initial state
  const [trends, setTrends] = useState<MarketTrend[]>(() => generateMockTrendData().trends);
  const [sentimentScore, setSentimentScore] = useState<number>(() => generateMockTrendData().sentimentScore);
  const [topMovers, setTopMovers] = useState<TrendingMarket[]>(() => generateMockTrendData().topMovers);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState<'all' | 'bullish' | 'bearish' | 'trending'>('all');

  const fetchTrends = useCallback(() => {
    setLoading(true);
    setTimeout(() => {
      const data = generateMockTrendData();
      setTrends(data.trends);
      setSentimentScore(data.sentimentScore);
      setTopMovers(data.topMovers);
      setLoading(false);
    }, 500);
  }, []);

  // Set up refresh interval only
  useEffect(() => {
    const interval = setInterval(() => {
      fetchTrends();
    }, 30000);
    return () => clearInterval(interval);
  }, [fetchTrends]);

  const filteredTrends = trends.filter((t) => {
    if (filter === 'all') return true;
    if (filter === 'bullish') return t.trend.includes('BULLISH');
    if (filter === 'bearish') return t.trend.includes('BEARISH');
    if (filter === 'trending') return t.isTrending;
    return true;
  });

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-bold flex items-center gap-2"><Zap className="h-5 w-5 text-yellow-500" />Real-Time Trends</h2>
          <p className="text-sm text-muted-foreground">Live market sentiment</p>
        </div>
        <Button variant="outline" size="sm" onClick={fetchTrends} disabled={loading}>
          <RefreshCw className={`h-4 w-4 mr-1 ${loading ? 'animate-spin' : ''}`} />Refresh
        </Button>
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        <div className="space-y-4">
          <Card>
            <CardHeader className="pb-2"><CardTitle className="text-sm">Market Sentiment</CardTitle></CardHeader>
            <CardContent>
              <SentimentGauge score={sentimentScore} />
              <div className="mt-4 space-y-2">
                <div className="flex justify-between text-xs"><span className="text-green-500">Bullish</span><span>{trends.filter((t) => t.trend.includes('BULLISH')).length}</span></div>
                <Progress value={(trends.filter((t) => t.trend.includes('BULLISH')).length / trends.length) * 100} className="h-2" />
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="pb-2">
              <div className="flex items-center justify-between">
                <CardTitle className="text-sm flex items-center gap-1"><Flame className="h-4 w-4 text-orange-500" />Trending Now</CardTitle>
                <Badge variant="outline" className="text-xs">Live</Badge>
              </div>
            </CardHeader>
            <CardContent className="p-0">
              <ScrollArea className="h-[300px]">{topMovers.map((item) => <TrendingItem key={item.epic} item={item} />)}</ScrollArea>
            </CardContent>
          </Card>
        </div>

        <div className="lg:col-span-2 space-y-4">
          <div className="flex gap-2">
            <Button variant={filter === 'all' ? 'default' : 'outline'} size="sm" onClick={() => setFilter('all')}>All</Button>
            <Button variant={filter === 'bullish' ? 'default' : 'outline'} size="sm" onClick={() => setFilter('bullish')} className="text-green-600"><TrendingUp className="h-3 w-3 mr-1" />Bullish</Button>
            <Button variant={filter === 'bearish' ? 'default' : 'outline'} size="sm" onClick={() => setFilter('bearish')} className="text-red-600"><TrendingDown className="h-3 w-3 mr-1" />Bearish</Button>
            <Button variant={filter === 'trending' ? 'default' : 'outline'} size="sm" onClick={() => setFilter('trending')}><Flame className="h-3 w-3 mr-1" />Hot</Button>
          </div>

          <div className="grid gap-4 md:grid-cols-2">
            {filteredTrends.map((trend) => (
              <Card key={trend.epic} className="p-4">
                <div className="flex items-start justify-between mb-3">
                  <div>
                    <div className="font-semibold">{trend.name}</div>
                    <div className={`text-sm font-semibold ${trend.changePercent >= 0 ? 'text-green-500' : 'text-red-500'}`}>{trend.changePercent >= 0 ? '+' : ''}{trend.changePercent.toFixed(2)}%</div>
                  </div>
                  <Badge variant={trend.trend.includes('BULLISH') ? 'default' : 'secondary'} className={trend.trend.includes('BULLISH') ? 'bg-green-500/10 text-green-600' : 'bg-red-500/10 text-red-500'}>
                    {trend.trend.replace('_', ' ')}
                  </Badge>
                </div>
                <div className="flex flex-wrap gap-1">
                  {trend.signals.map((s) => (
                    <Badge key={s.name} variant="outline" className={`text-xs ${s.signal === 'BUY' ? 'bg-green-500/10 text-green-600' : 'bg-red-500/10 text-red-500'}`}>{s.name}: {s.value}</Badge>
                  ))}
                </div>
              </Card>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
