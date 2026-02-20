'use client';

import { useState, useEffect, useCallback } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { ScrollArea } from '@/components/ui/scroll-area';
import { 
  TrendingUp, 
  TrendingDown, 
  Activity, 
  Flame, 
  RefreshCw, 
  Filter,
  Target,
  AlertTriangle,
  CheckCircle,
  Zap
} from 'lucide-react';

interface ScanResult {
  epic: string;
  name: string;
  timestamp: string;
  price: number;
  change: number;
  changePercent: number;
  trend: 'STRONG_BULLISH' | 'BULLISH' | 'NEUTRAL' | 'BEARISH' | 'STRONG_BEARISH';
  trendStrength: number;
  signal: 'STRONG_BUY' | 'BUY' | 'HOLD' | 'SELL' | 'STRONG_SELL';
  signalStrength: number;
  confidence: number;
  score: number;
  hotness: number;
  indicators: {
    rsi: { value: number; signal: string };
    macd: { value: number; signal: string };
    ma: { short: number; long: number; signal: string };
    bb: { upper: number; lower: number; position: number; signal: string };
    atr: number;
  };
  setup: {
    entry: number;
    stopLoss: number;
    takeProfit1: number;
    takeProfit2: number;
    riskReward: number;
  };
  reasons: string[];
  warnings: string[];
}

export function MarketScannerPanel() {
  const [results, setResults] = useState<ScanResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState<'all' | 'buy' | 'sell' | 'strong'>('all');
  const [lastScan, setLastScan] = useState<Date | null>(null);

  const runScan = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetch('/api/scanner');
      const data = await response.json();
      if (data.success) {
        setResults(data.results);
        setLastScan(new Date());
      }
    } catch (error) {
      console.error('Scan failed:', error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    runScan();
  }, [runScan]);

  const getSignalColor = (signal: string) => {
    switch (signal) {
      case 'STRONG_BUY': return 'bg-green-500 text-white';
      case 'BUY': return 'bg-green-100 text-green-700 border-green-300';
      case 'STRONG_SELL': return 'bg-red-500 text-white';
      case 'SELL': return 'bg-red-100 text-red-700 border-red-300';
      default: return 'bg-gray-100 text-gray-700';
    }
  };

  const getTrendIcon = (trend: string) => {
    if (trend.includes('BULLISH')) {
      return <TrendingUp className="h-4 w-4 text-green-500" />;
    } else if (trend.includes('BEARISH')) {
      return <TrendingDown className="h-4 w-4 text-red-500" />;
    }
    return <Activity className="h-4 w-4 text-gray-500" />;
  };

  const filteredResults = results.filter(r => {
    if (filter === 'all') return true;
    if (filter === 'buy') return r.signal.includes('BUY');
    if (filter === 'sell') return r.signal.includes('SELL');
    if (filter === 'strong') return r.signal.includes('STRONG');
    return true;
  });

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-bold flex items-center gap-2">
            <Target className="h-5 w-5 text-blue-500" />
            Market Scanner
          </h2>
          <p className="text-sm text-muted-foreground">
            {lastScan ? `Last scan: ${lastScan.toLocaleTimeString()}` : 'Click scan to analyze markets'}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={runScan}
            disabled={loading}
          >
            <RefreshCw className={`h-4 w-4 mr-1 ${loading ? 'animate-spin' : ''}`} />
            {loading ? 'Scanning...' : 'Scan Now'}
          </Button>
        </div>
      </div>

      {/* Filters */}
      <div className="flex gap-2">
        <Button
          variant={filter === 'all' ? 'default' : 'outline'}
          size="sm"
          onClick={() => setFilter('all')}
        >
          All ({results.length})
        </Button>
        <Button
          variant={filter === 'buy' ? 'default' : 'outline'}
          size="sm"
          onClick={() => setFilter('buy')}
          className="text-green-600"
        >
          <TrendingUp className="h-3 w-3 mr-1" />
          Buy ({results.filter(r => r.signal.includes('BUY')).length})
        </Button>
        <Button
          variant={filter === 'sell' ? 'default' : 'outline'}
          size="sm"
          onClick={() => setFilter('sell')}
          className="text-red-600"
        >
          <TrendingDown className="h-3 w-3 mr-1" />
          Sell ({results.filter(r => r.signal.includes('SELL')).length})
        </Button>
        <Button
          variant={filter === 'strong' ? 'default' : 'outline'}
          size="sm"
          onClick={() => setFilter('strong')}
        >
          <Zap className="h-3 w-3 mr-1" />
          Strong ({results.filter(r => r.signal.includes('STRONG')).length})
        </Button>
      </div>

      {/* Results Grid */}
      <div className="grid gap-4 md:grid-cols-2">
        {filteredResults.length === 0 ? (
          <Card className="col-span-2">
            <CardContent className="py-8 text-center text-muted-foreground">
              <Filter className="h-8 w-8 mx-auto mb-2 opacity-50" />
              <p>No markets match the current filter</p>
            </CardContent>
          </Card>
        ) : (
          filteredResults.map((result) => (
            <Card key={result.epic} className={`overflow-hidden ${
              result.signal === 'STRONG_BUY' ? 'border-l-4 border-l-green-500' :
              result.signal === 'STRONG_SELL' ? 'border-l-4 border-l-red-500' : ''
            }`}>
              <CardHeader className="pb-2">
                <div className="flex items-start justify-between">
                  <div>
                    <CardTitle className="text-lg">{result.name}</CardTitle>
                    <div className="flex items-center gap-2 mt-1">
                      <span className="text-xl font-bold">${result.price.toFixed(2)}</span>
                      <span className={`text-sm ${result.change >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                        {result.change >= 0 ? '+' : ''}{result.changePercent.toFixed(2)}%
                      </span>
                    </div>
                  </div>
                  <div className="text-right">
                    <Badge className={getSignalColor(result.signal)}>
                      {result.signal.replace('_', ' ')}
                    </Badge>
                    <div className="flex items-center gap-1 mt-1 text-xs text-muted-foreground">
                      {getTrendIcon(result.trend)}
                      {result.trend.replace('_', ' ')}
                    </div>
                  </div>
                </div>
              </CardHeader>
              
              <CardContent className="space-y-3">
                {/* Score & Confidence */}
                <div className="grid grid-cols-2 gap-2">
                  <div className="p-2 bg-muted/50 rounded-lg">
                    <div className="text-xs text-muted-foreground">Score</div>
                    <div className="flex items-center gap-2">
                      <span className="text-lg font-bold">{result.score}</span>
                      <Progress value={result.score} className="flex-1 h-2" />
                    </div>
                  </div>
                  <div className="p-2 bg-muted/50 rounded-lg">
                    <div className="text-xs text-muted-foreground">Confidence</div>
                    <div className="flex items-center gap-2">
                      <span className="text-lg font-bold">{result.confidence}%</span>
                      <Progress value={result.confidence} className="flex-1 h-2" />
                    </div>
                  </div>
                </div>

                {/* Hotness */}
                <div className="flex items-center gap-2">
                  <Flame className={`h-4 w-4 ${result.hotness >= 7 ? 'text-orange-500' : 'text-gray-400'}`} />
                  <span className="text-sm">Hotness: {result.hotness}/10</span>
                  <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden">
                    <div 
                      className={`h-full ${result.hotness >= 7 ? 'bg-orange-500' : result.hotness >= 4 ? 'bg-yellow-500' : 'bg-blue-500'}`}
                      style={{ width: `${result.hotness * 10}%` }}
                    />
                  </div>
                </div>

                {/* Indicators */}
                <div className="flex flex-wrap gap-1">
                  <Badge variant="outline" className="text-xs">
                    RSI: {result.indicators.rsi.value.toFixed(1)}
                  </Badge>
                  <Badge variant="outline" className="text-xs">
                    MACD: {result.indicators.macd.signal}
                  </Badge>
                  <Badge variant="outline" className="text-xs">
                    MA: {result.indicators.ma.signal}
                  </Badge>
                  <Badge variant="outline" className="text-xs">
                    BB: {result.indicators.bb.signal}
                  </Badge>
                </div>

                {/* Setup */}
                {result.signal !== 'HOLD' && (
                  <div className="p-2 bg-muted/30 rounded-lg text-xs">
                    <div className="font-medium mb-1">Trading Setup:</div>
                    <div className="grid grid-cols-2 gap-1 text-muted-foreground">
                      <div>Entry: ${result.setup.entry.toFixed(2)}</div>
                      <div>R:R: {result.setup.riskReward.toFixed(1)}</div>
                      <div>SL: ${result.setup.stopLoss.toFixed(2)}</div>
                      <div>TP: ${result.setup.takeProfit1.toFixed(2)}</div>
                    </div>
                  </div>
                )}

                {/* Reasons */}
                {result.reasons.length > 0 && (
                  <div className="text-xs text-muted-foreground">
                    {result.reasons.slice(0, 2).map((reason, i) => (
                      <div key={i} className="flex items-center gap-1">
                        <CheckCircle className="h-3 w-3 text-green-500" />
                        {reason}
                      </div>
                    ))}
                  </div>
                )}

                {/* Warnings */}
                {result.warnings.length > 0 && (
                  <div className="text-xs text-yellow-600">
                    {result.warnings.slice(0, 1).map((warning, i) => (
                      <div key={i} className="flex items-center gap-1">
                        <AlertTriangle className="h-3 w-3" />
                        {warning}
                      </div>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>
          ))
        )}
      </div>
    </div>
  );
}
