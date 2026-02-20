'use client';

import { useState, useEffect, useCallback } from 'react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Progress } from '@/components/ui/progress';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Separator } from '@/components/ui/separator';
import {
  Brain,
  TrendingUp,
  TrendingDown,
  Minus,
  AlertTriangle,
  CheckCircle,
  Clock,
  Target,
  Zap,
  RefreshCw,
  Sparkles,
  Info,
  AlertCircle,
  ChevronUp,
  ChevronDown,
  Activity,
  BarChart3
} from 'lucide-react';
import type { AITradeSuggestion, SentimentAnalysis, AIInsight } from '@/lib/ai-analysis';

interface AIInsightsPanelProps {
  selectedSymbol?: string;
  onTradeSignal?: (signal: AITradeSuggestion) => void;
}

export function AIInsightsPanel({ selectedSymbol = 'GOLD', onTradeSignal }: AIInsightsPanelProps) {
  const [loading, setLoading] = useState(false);
  const [suggestion, setSuggestion] = useState<AITradeSuggestion | null>(null);
  const [sentiment, setSentiment] = useState<SentimentAnalysis | null>(null);
  const [insights, setInsights] = useState<AIInsight[]>([]);
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(true);

  const fetchAIAnalysis = useCallback(async () => {
    setLoading(true);
    try {
      // Fetch AI suggestions
      const response = await fetch('/api/ai-analysis', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'suggestion',
          context: {
            symbol: selectedSymbol,
            name: selectedSymbol === 'GOLD' ? 'Gold Spot' : selectedSymbol,
            currentPrice: selectedSymbol === 'GOLD' ? 2340.50 : 1.0850,
            dayChange: 5.20,
            dayChangePercent: 0.22,
            volume: 1250000,
            high24h: selectedSymbol === 'GOLD' ? 2350.00 : 1.0900,
            low24h: selectedSymbol === 'GOLD' ? 2330.00 : 1.0800,
            trend: 'bullish',
            volatility: 'medium'
          },
          indicators: {
            rsi: 58,
            macd: { value: 2.5, signal: 1.8, histogram: 0.7 },
            ma20: selectedSymbol === 'GOLD' ? 2335.00 : 1.0825,
            ma50: selectedSymbol === 'GOLD' ? 2320.00 : 1.0790,
            ma200: selectedSymbol === 'GOLD' ? 2280.00 : 1.0650,
            bollingerBands: {
              upper: selectedSymbol === 'GOLD' ? 2360.00 : 1.0920,
              middle: selectedSymbol === 'GOLD' ? 2340.00 : 1.0850,
              lower: selectedSymbol === 'GOLD' ? 2320.00 : 1.0780
            },
            atr: selectedSymbol === 'GOLD' ? 8.5 : 0.0045,
            adx: 32,
            stochastic: { k: 65, d: 58 },
            williamsR: -35
          },
          historicalData: generateMockHistoricalData(selectedSymbol)
        })
      });

      const data = await response.json();
      if (data.success) {
        setSuggestion(data.suggestion);
      }

      // Fetch sentiment analysis
      const sentimentResponse = await fetch('/api/ai-analysis', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'sentiment',
          context: {
            symbol: selectedSymbol,
            name: selectedSymbol === 'GOLD' ? 'Gold Spot' : selectedSymbol,
            currentPrice: selectedSymbol === 'GOLD' ? 2340.50 : 1.0850,
            dayChange: 5.20,
            dayChangePercent: 0.22,
            volume: 1250000,
            high24h: selectedSymbol === 'GOLD' ? 2350.00 : 1.0900,
            low24h: selectedSymbol === 'GOLD' ? 2330.00 : 1.0800,
            trend: 'bullish',
            volatility: 'medium'
          },
          indicators: {
            rsi: 58,
            macd: { value: 2.5, signal: 1.8, histogram: 0.7 },
            ma20: selectedSymbol === 'GOLD' ? 2335.00 : 1.0825,
            ma50: selectedSymbol === 'GOLD' ? 2320.00 : 1.0790,
            ma200: selectedSymbol === 'GOLD' ? 2280.00 : 1.0650,
            bollingerBands: {
              upper: selectedSymbol === 'GOLD' ? 2360.00 : 1.0920,
              middle: selectedSymbol === 'GOLD' ? 2340.00 : 1.0850,
              lower: selectedSymbol === 'GOLD' ? 2320.00 : 1.0780
            },
            atr: selectedSymbol === 'GOLD' ? 8.5 : 0.0045,
            adx: 32,
            stochastic: { k: 65, d: 58 },
            williamsR: -35
          },
          historicalData: generateMockHistoricalData(selectedSymbol)
        })
      });

      const sentimentData = await sentimentResponse.json();
      if (sentimentData.success) {
        setSentiment(sentimentData.sentiment);
      }

      setLastUpdate(new Date());
    } catch (error) {
      console.error('Failed to fetch AI analysis:', error);
    } finally {
      setLoading(false);
    }
  }, [selectedSymbol]);

  useEffect(() => {
    fetchAIAnalysis();
  }, [fetchAIAnalysis]);

  useEffect(() => {
    if (!autoRefresh) return;
    
    const interval = setInterval(fetchAIAnalysis, 60000); // Refresh every minute
    return () => clearInterval(interval);
  }, [autoRefresh, fetchAIAnalysis]);

  const getActionIcon = (action: string) => {
    switch (action) {
      case 'BUY': return <TrendingUp className="h-4 w-4 text-green-500" />;
      case 'SELL': return <TrendingDown className="h-4 w-4 text-red-500" />;
      default: return <Minus className="h-4 w-4 text-yellow-500" />;
    }
  };

  const getActionColor = (action: string) => {
    switch (action) {
      case 'BUY': return 'text-green-500 bg-green-500/10 border-green-500/20';
      case 'SELL': return 'text-red-500 bg-red-500/10 border-red-500/20';
      default: return 'text-yellow-500 bg-yellow-500/10 border-yellow-500/20';
    }
  };

  const getSentimentColor = (sentiment: string) => {
    switch (sentiment) {
      case 'very_bullish': return 'text-green-600';
      case 'bullish': return 'text-green-500';
      case 'neutral': return 'text-yellow-500';
      case 'bearish': return 'text-red-500';
      case 'very_bearish': return 'text-red-600';
      default: return 'text-gray-500';
    }
  };

  const getConfidenceLevel = (confidence: number) => {
    if (confidence >= 80) return { label: 'Very High', color: 'bg-green-500' };
    if (confidence >= 60) return { label: 'High', color: 'bg-green-400' };
    if (confidence >= 40) return { label: 'Medium', color: 'bg-yellow-500' };
    return { label: 'Low', color: 'bg-red-500' };
  };

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Brain className="h-5 w-5 text-purple-500" />
          <h3 className="font-semibold">AI Insights</h3>
          <Badge variant="outline" className="text-xs">
            <Sparkles className="h-3 w-3 mr-1" />
            Powered by AI
          </Badge>
        </div>
        <div className="flex items-center gap-2">
          {lastUpdate && (
            <span className="text-xs text-muted-foreground">
              Updated {lastUpdate.toLocaleTimeString()}
            </span>
          )}
          <Button
            variant="ghost"
            size="sm"
            onClick={fetchAIAnalysis}
            disabled={loading}
          >
            <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
          </Button>
        </div>
      </div>

      <Tabs defaultValue="suggestion" className="w-full">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="suggestion" className="text-xs">
            <Target className="h-3 w-3 mr-1" />
            Trade Signal
          </TabsTrigger>
          <TabsTrigger value="sentiment" className="text-xs">
            <Activity className="h-3 w-3 mr-1" />
            Sentiment
          </TabsTrigger>
          <TabsTrigger value="insights" className="text-xs">
            <Zap className="h-3 w-3 mr-1" />
            Insights
          </TabsTrigger>
        </TabsList>

        {/* Trade Suggestion Tab */}
        <TabsContent value="suggestion" className="mt-4">
          {suggestion ? (
            <Card className="p-4">
              {/* Action Badge */}
              <div className="flex items-center justify-between mb-4">
                <div className="flex items-center gap-2">
                  {getActionIcon(suggestion.action)}
                  <Badge className={getActionColor(suggestion.action)}>
                    {suggestion.action}
                  </Badge>
                  <span className="text-sm text-muted-foreground">
                    {suggestion.symbol}
                  </span>
                </div>
                <Badge variant="outline">
                  {suggestion.timeframe.toUpperCase()}
                </Badge>
              </div>

              {/* Confidence */}
              <div className="mb-4">
                <div className="flex items-center justify-between text-sm mb-1">
                  <span>Confidence</span>
                  <span className="font-medium">{suggestion.confidence}%</span>
                </div>
                <Progress 
                  value={suggestion.confidence} 
                  className="h-2"
                />
                <div className="flex items-center gap-1 mt-1">
                  <div className={`h-2 w-2 rounded-full ${getConfidenceLevel(suggestion.confidence).color}`} />
                  <span className="text-xs text-muted-foreground">
                    {getConfidenceLevel(suggestion.confidence).label} confidence
                  </span>
                </div>
              </div>

              {/* Price Levels */}
              <div className="grid grid-cols-3 gap-2 mb-4">
                <div className="p-2 rounded bg-muted/50 text-center">
                  <div className="text-xs text-muted-foreground">Entry</div>
                  <div className="font-semibold">{suggestion.entryPrice.toFixed(2)}</div>
                </div>
                <div className="p-2 rounded bg-red-500/10 text-center">
                  <div className="text-xs text-red-500">Stop Loss</div>
                  <div className="font-semibold text-red-500">{suggestion.stopLoss.toFixed(2)}</div>
                </div>
                <div className="p-2 rounded bg-green-500/10 text-center">
                  <div className="text-xs text-green-500">Take Profit</div>
                  <div className="font-semibold text-green-500">{suggestion.takeProfit.toFixed(2)}</div>
                </div>
              </div>

              {/* Risk/Reward */}
              <div className="flex items-center justify-between p-2 rounded bg-muted/50 mb-4">
                <div className="flex items-center gap-2">
                  <BarChart3 className="h-4 w-4" />
                  <span className="text-sm">Risk:Reward</span>
                </div>
                <span className="font-semibold text-green-500">1:{suggestion.riskRewardRatio.toFixed(1)}</span>
              </div>

              {/* Position Size */}
              <div className="flex items-center justify-between p-2 rounded bg-muted/50 mb-4">
                <span className="text-sm">Suggested Position Size</span>
                <span className="font-semibold">{suggestion.positionSizePercent.toFixed(1)}% of capital</span>
              </div>

              {/* Reasoning */}
              <div className="mb-4">
                <div className="text-sm font-medium mb-2">AI Reasoning</div>
                <p className="text-sm text-muted-foreground">{suggestion.reasoning}</p>
              </div>

              {/* Technical Reasons */}
              <div className="mb-4">
                <div className="text-sm font-medium mb-2">Technical Factors</div>
                <div className="space-y-1">
                  {suggestion.technicalReasons.map((reason, i) => (
                    <div key={i} className="flex items-start gap-2 text-sm">
                      <CheckCircle className="h-4 w-4 text-green-500 mt-0.5 shrink-0" />
                      <span>{reason}</span>
                    </div>
                  ))}
                </div>
              </div>

              {/* Risk Factors */}
              <div className="mb-4">
                <div className="text-sm font-medium mb-2">Risk Factors</div>
                <div className="space-y-1">
                  {suggestion.riskFactors.map((risk, i) => (
                    <div key={i} className="flex items-start gap-2 text-sm">
                      <AlertTriangle className="h-4 w-4 text-yellow-500 mt-0.5 shrink-0" />
                      <span>{risk}</span>
                    </div>
                  ))}
                </div>
              </div>

              {/* Action Button */}
              {suggestion.action !== 'HOLD' && onTradeSignal && (
                <Button 
                  className="w-full" 
                  onClick={() => onTradeSignal(suggestion)}
                  disabled={suggestion.confidence < 60}
                >
                  <Zap className="h-4 w-4 mr-2" />
                  Execute Trade Signal
                </Button>
              )}

              {/* Expiration */}
              <div className="flex items-center justify-center gap-1 mt-3 text-xs text-muted-foreground">
                <Clock className="h-3 w-3" />
                Signal expires in {Math.max(0, Math.floor((suggestion.expiresAt - Date.now()) / 60000))} minutes
              </div>
            </Card>
          ) : (
            <Card className="p-8 text-center">
              <Brain className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
              <p className="text-muted-foreground">
                {loading ? 'Analyzing market conditions...' : 'No AI analysis available'}
              </p>
            </Card>
          )}
        </TabsContent>

        {/* Sentiment Analysis Tab */}
        <TabsContent value="sentiment" className="mt-4">
          {sentiment ? (
            <Card className="p-4">
              {/* Overall Sentiment */}
              <div className="text-center mb-6">
                <div className={`text-3xl font-bold ${getSentimentColor(sentiment.overallSentiment)}`}>
                  {sentiment.overallSentiment.replace('_', ' ').toUpperCase()}
                </div>
                <div className="flex items-center justify-center gap-2 mt-2">
                  <span className="text-2xl font-semibold">
                    {sentiment.sentimentScore > 0 ? '+' : ''}{sentiment.sentimentScore}
                  </span>
                  <span className="text-sm text-muted-foreground">/ 100</span>
                </div>
                <Progress 
                  value={(sentiment.sentimentScore + 100) / 2} 
                  className="h-3 mt-2"
                />
              </div>

              {/* Sentiment Factors */}
              <div className="space-y-3 mb-4">
                {Object.entries(sentiment.factors).map(([key, factor]) => (
                  <div key={key} className="flex items-center justify-between p-2 rounded bg-muted/50">
                    <div className="flex items-center gap-2">
                      {key === 'technical' && <BarChart3 className="h-4 w-4" />}
                      {key === 'momentum' && <Activity className="h-4 w-4" />}
                      {key === 'volatility' && <Zap className="h-4 w-4" />}
                      {key === 'trend' && <TrendingUp className="h-4 w-4" />}
                      {key === 'volume' && <ChevronUp className="h-4 w-4" />}
                      <span className="text-sm capitalize">{key}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className={`font-medium ${factor.score > 0 ? 'text-green-500' : factor.score < 0 ? 'text-red-500' : ''}`}>
                        {factor.score > 0 ? '+' : ''}{factor.score}
                      </span>
                    </div>
                  </div>
                ))}
              </div>

              {/* Key Levels */}
              <div className="grid grid-cols-2 gap-2 mb-4">
                <div className="p-2 rounded bg-green-500/10">
                  <div className="text-xs text-green-500 mb-1">Support Levels</div>
                  {sentiment.keyLevels.support.map((level, i) => (
                    <div key={i} className="text-sm font-medium">{level.toFixed(2)}</div>
                  ))}
                </div>
                <div className="p-2 rounded bg-red-500/10">
                  <div className="text-xs text-red-500 mb-1">Resistance Levels</div>
                  {sentiment.keyLevels.resistance.map((level, i) => (
                    <div key={i} className="text-sm font-medium">{level.toFixed(2)}</div>
                  ))}
                </div>
              </div>

              {/* Market Phase */}
              <div className="flex items-center justify-between p-2 rounded bg-muted/50 mb-4">
                <span className="text-sm">Market Phase</span>
                <Badge variant="outline">
                  {sentiment.marketPhase.replace('-', ' ').toUpperCase()}
                </Badge>
              </div>

              {/* Recommendation */}
              <div className="p-3 rounded bg-blue-500/10 border border-blue-500/20">
                <div className="flex items-start gap-2">
                  <Info className="h-4 w-4 text-blue-500 mt-0.5" />
                  <div>
                    <div className="text-sm font-medium text-blue-500">Recommendation</div>
                    <div className="text-sm">{sentiment.tradingRecommendation}</div>
                  </div>
                </div>
              </div>
            </Card>
          ) : (
            <Card className="p-8 text-center">
              <Activity className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
              <p className="text-muted-foreground">
                {loading ? 'Analyzing market sentiment...' : 'No sentiment analysis available'}
              </p>
            </Card>
          )}
        </TabsContent>

        {/* Insights Tab */}
        <TabsContent value="insights" className="mt-4">
          {insights.length > 0 ? (
            <ScrollArea className="h-[400px]">
              <div className="space-y-2">
                {insights.map((insight, i) => (
                  <Card key={i} className={`p-3 ${
                    insight.type === 'opportunity' ? 'border-green-500/20 bg-green-500/5' :
                    insight.type === 'warning' ? 'border-yellow-500/20 bg-yellow-500/5' :
                    insight.type === 'critical' ? 'border-red-500/20 bg-red-500/5' :
                    'border-blue-500/20 bg-blue-500/5'
                  }`}>
                    <div className="flex items-start gap-3">
                      {insight.type === 'opportunity' && <CheckCircle className="h-5 w-5 text-green-500 mt-0.5" />}
                      {insight.type === 'warning' && <AlertTriangle className="h-5 w-5 text-yellow-500 mt-0.5" />}
                      {insight.type === 'critical' && <AlertCircle className="h-5 w-5 text-red-500 mt-0.5" />}
                      {insight.type === 'info' && <Info className="h-5 w-5 text-blue-500 mt-0.5" />}
                      <div className="flex-1">
                        <div className="flex items-center justify-between">
                          <span className="font-medium text-sm">{insight.title}</span>
                          <Badge variant="outline" className="text-xs">
                            {insight.priority}
                          </Badge>
                        </div>
                        <p className="text-sm text-muted-foreground mt-1">{insight.message}</p>
                        <div className="flex items-center gap-2 mt-2 text-xs text-muted-foreground">
                          {insight.symbol && <Badge variant="secondary" className="text-xs">{insight.symbol}</Badge>}
                          <Clock className="h-3 w-3" />
                          {new Date(insight.timestamp).toLocaleTimeString()}
                        </div>
                      </div>
                    </div>
                  </Card>
                ))}
              </div>
            </ScrollArea>
          ) : (
            <Card className="p-8 text-center">
              <Sparkles className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
              <p className="text-muted-foreground">
                AI insights will appear here based on market conditions
              </p>
              <Button 
                variant="outline" 
                size="sm" 
                className="mt-4"
                onClick={fetchAIAnalysis}
                disabled={loading}
              >
                Generate Insights
              </Button>
            </Card>
          )}
        </TabsContent>
      </Tabs>

      {/* Auto Refresh Toggle */}
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={autoRefresh}
            onChange={(e) => setAutoRefresh(e.target.checked)}
            className="rounded"
          />
          Auto-refresh every minute
        </label>
      </div>
    </div>
  );
}

// Helper function to generate mock historical data
function generateMockHistoricalData(symbol: string) {
  const basePrice = symbol === 'GOLD' ? 2340 : 1.085;
  const data = [];
  
  for (let i = 0; i < 50; i++) {
    const variation = (Math.random() - 0.5) * 0.02;
    const open = basePrice * (1 + variation);
    const close = open * (1 + (Math.random() - 0.5) * 0.01);
    const high = Math.max(open, close) * (1 + Math.random() * 0.005);
    const low = Math.min(open, close) * (1 - Math.random() * 0.005);
    
    data.push({
      timestamp: Date.now() - (50 - i) * 3600000,
      open,
      high,
      low,
      close,
      volume: Math.floor(Math.random() * 1000000) + 500000
    });
  }
  
  return data;
}
