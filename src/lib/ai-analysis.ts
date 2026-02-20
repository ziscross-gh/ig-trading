/**
 * AI-Powered Trade Analysis Service
 * Uses z-ai-web-dev-sdk for intelligent trade suggestions and sentiment analysis
 */

import ZAI from 'z-ai-web-dev-sdk';

export interface MarketDataPoint {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume?: number;
}

export interface TechnicalIndicators {
  rsi: number;
  macd: { value: number; signal: number; histogram: number };
  ma20: number;
  ma50: number;
  ma200: number;
  bollingerBands: { upper: number; middle: number; lower: number };
  atr: number;
  adx: number;
  stochastic: { k: number; d: number };
  williamsR: number;
}

export interface MarketContext {
  symbol: string;
  name: string;
  currentPrice: number;
  dayChange: number;
  dayChangePercent: number;
  volume: number;
  high24h: number;
  low24h: number;
  trend: 'bullish' | 'bearish' | 'sideways';
  volatility: 'low' | 'medium' | 'high';
}

export interface AITradeSuggestion {
  id: string;
  symbol: string;
  action: 'BUY' | 'SELL' | 'HOLD';
  confidence: number; // 0-100
  entryPrice: number;
  stopLoss: number;
  takeProfit: number;
  riskRewardRatio: number;
  positionSizePercent: number;
  timeframe: 'scalp' | 'intraday' | 'swing' | 'position';
  reasoning: string;
  technicalReasons: string[];
  riskFactors: string[];
  marketConditions: string;
  expiresAt: number;
  createdAt: number;
}

export interface SentimentAnalysis {
  symbol: string;
  overallSentiment: 'very_bullish' | 'bullish' | 'neutral' | 'bearish' | 'very_bearish';
  sentimentScore: number; // -100 to 100
  confidence: number;
  factors: {
    technical: { score: number; description: string };
    momentum: { score: number; description: string };
    volatility: { score: number; description: string };
    trend: { score: number; description: string };
    volume: { score: number; description: string };
  };
  keyLevels: {
    support: number[];
    resistance: number[];
  };
  marketPhase: 'accumulation' | 'mark-up' | 'distribution' | 'mark-down';
  tradingRecommendation: string;
  timestamp: number;
}

export interface AIInsight {
  type: 'opportunity' | 'warning' | 'info' | 'critical';
  title: string;
  message: string;
  symbol?: string;
  action?: string;
  priority: 'low' | 'medium' | 'high';
  timestamp: number;
}

class AIAnalysisService {
  private zai: Awaited<ReturnType<typeof ZAI.create>> | null = null;
  private cache: Map<string, { data: any; timestamp: number }> = new Map();
  private cacheTimeout = 60000; // 1 minute cache

  async initialize() {
    if (!this.zai) {
      this.zai = await ZAI.create();
    }
    return this.zai;
  }

  private getCached<T>(key: string): T | null {
    const cached = this.cache.get(key);
    if (cached && Date.now() - cached.timestamp < this.cacheTimeout) {
      return cached.data as T;
    }
    return null;
  }

  private setCache(key: string, data: any) {
    this.cache.set(key, { data, timestamp: Date.now() });
  }

  /**
   * Generate AI-powered trade suggestions based on technical analysis
   */
  async generateTradeSuggestion(
    context: MarketContext,
    indicators: TechnicalIndicators,
    historicalData: MarketDataPoint[]
  ): Promise<AITradeSuggestion> {
    const cacheKey = `suggestion-${context.symbol}-${Math.floor(Date.now() / 30000)}`;
    const cached = this.getCached<AITradeSuggestion>(cacheKey);
    if (cached) return cached;

    await this.initialize();

    const prompt = `You are an expert financial analyst specializing in ${context.symbol} trading. Analyze the following market data and provide a trade recommendation.

MARKET CONTEXT:
- Symbol: ${context.symbol} (${context.name})
- Current Price: ${context.currentPrice}
- Day Change: ${context.dayChangePercent.toFixed(2)}%
- 24h High: ${context.high24h}, Low: ${context.low24h}
- Trend: ${context.trend}
- Volatility: ${context.volatility}

TECHNICAL INDICATORS:
- RSI (14): ${indicators.rsi.toFixed(2)}
- MACD: ${indicators.macd.value.toFixed(4)}, Signal: ${indicators.macd.signal.toFixed(4)}, Histogram: ${indicators.macd.histogram.toFixed(4)}
- Moving Averages: MA20=${indicators.ma20.toFixed(2)}, MA50=${indicators.ma50.toFixed(2)}, MA200=${indicators.ma200.toFixed(2)}
- Bollinger Bands: Upper=${indicators.bollingerBands.upper.toFixed(2)}, Middle=${indicators.bollingerBands.middle.toFixed(2)}, Lower=${indicators.bollingerBands.lower.toFixed(2)}
- ATR: ${indicators.atr.toFixed(4)}
- ADX: ${indicators.adx.toFixed(2)}
- Stochastic: K=${indicators.stochastic.k.toFixed(2)}, D=${indicators.stochastic.d.toFixed(2)}
- Williams %R: ${indicators.williamsR.toFixed(2)}

RECENT PRICE ACTION (last 5 candles):
${historicalData.slice(-5).map((c, i) => 
  `${i + 1}. O:${c.open.toFixed(2)} H:${c.high.toFixed(2)} L:${c.low.toFixed(2)} C:${c.close.toFixed(2)}`
).join('\n')}

Based on this analysis, provide a JSON response with the following structure:
{
  "action": "BUY" | "SELL" | "HOLD",
  "confidence": <number 0-100>,
  "entryPrice": <number>,
  "stopLoss": <number>,
  "takeProfit": <number>,
  "riskRewardRatio": <number>,
  "positionSizePercent": <number 0.5-5>,
  "timeframe": "scalp" | "intraday" | "swing" | "position",
  "reasoning": "<detailed explanation in 2-3 sentences>",
  "technicalReasons": ["<reason 1>", "<reason 2>", "<reason 3>"],
  "riskFactors": ["<risk 1>", "<risk 2>"],
  "marketConditions": "<description of current market state>"
}

IMPORTANT: 
- Only recommend trades with confidence >= 60%
- Set stop loss at 1.5-2x ATR from entry
- Set take profit at 2-3x the stop loss distance
- Consider current market volatility for position sizing
- Respond ONLY with valid JSON, no other text.`;

    try {
      const completion = await this.zai!.chat.completions.create({
        messages: [
          { role: 'system', content: 'You are a professional trading analyst. Respond only with valid JSON.' },
          { role: 'user', content: prompt }
        ],
        temperature: 0.3,
        max_tokens: 500
      });

      const responseText = completion.choices[0]?.message?.content || '';
      const jsonMatch = responseText.match(/\{[\s\S]*\}/);
      
      if (!jsonMatch) {
        return this.getDefaultSuggestion(context, indicators);
      }

      const parsed = JSON.parse(jsonMatch[0]);
      
      const suggestion: AITradeSuggestion = {
        id: `ai-${context.symbol}-${Date.now()}`,
        symbol: context.symbol,
        action: parsed.action || 'HOLD',
        confidence: Math.min(100, Math.max(0, parsed.confidence || 50)),
        entryPrice: parsed.entryPrice || context.currentPrice,
        stopLoss: parsed.stopLoss || context.currentPrice * 0.98,
        takeProfit: parsed.takeProfit || context.currentPrice * 1.04,
        riskRewardRatio: parsed.riskRewardRatio || 2,
        positionSizePercent: Math.min(5, Math.max(0.5, parsed.positionSizePercent || 1)),
        timeframe: parsed.timeframe || 'intraday',
        reasoning: parsed.reasoning || 'Based on technical analysis',
        technicalReasons: parsed.technicalReasons || [],
        riskFactors: parsed.riskFactors || ['Market volatility', 'Economic events'],
        marketConditions: parsed.marketConditions || 'Normal market conditions',
        expiresAt: Date.now() + 3600000, // 1 hour
        createdAt: Date.now()
      };

      this.setCache(cacheKey, suggestion);
      return suggestion;
    } catch (error) {
      console.error('AI Analysis error:', error);
      return this.getDefaultSuggestion(context, indicators);
    }
  }

  /**
   * Perform comprehensive sentiment analysis
   */
  async analyzeSentiment(
    context: MarketContext,
    indicators: TechnicalIndicators,
    historicalData: MarketDataPoint[]
  ): Promise<SentimentAnalysis> {
    const cacheKey = `sentiment-${context.symbol}-${Math.floor(Date.now() / 60000)}`;
    const cached = this.getCached<SentimentAnalysis>(cacheKey);
    if (cached) return cached;

    await this.initialize();

    // Calculate technical sentiment factors
    const technicalScore = this.calculateTechnicalScore(indicators);
    const momentumScore = this.calculateMomentumScore(indicators, historicalData);
    const volatilityScore = this.calculateVolatilityScore(indicators);
    const trendScore = this.calculateTrendScore(indicators, context);
    const volumeScore = this.calculateVolumeScore(historicalData);

    const prompt = `Analyze market sentiment for ${context.symbol} trading.

MARKET DATA:
- Current Price: ${context.currentPrice}
- Day Change: ${context.dayChangePercent.toFixed(2)}%
- Trend: ${context.trend}
- Volatility: ${context.volatility}

CALCULATED SCORES:
- Technical: ${technicalScore} (-100 to 100)
- Momentum: ${momentumScore} (-100 to 100)
- Volatility: ${volatilityScore} (0-100, higher = more volatile)
- Trend Strength: ${trendScore} (-100 to 100)

RSI: ${indicators.rsi.toFixed(2)}
MACD Histogram: ${indicators.macd.histogram.toFixed(4)}
Price vs MA20: ${((context.currentPrice / indicators.ma20 - 1) * 100).toFixed(2)}%
Price vs MA50: ${((context.currentPrice / indicators.ma50 - 1) * 100).toFixed(2)}%
ADX: ${indicators.adx.toFixed(2)}

Provide a JSON sentiment analysis:
{
  "overallSentiment": "very_bullish" | "bullish" | "neutral" | "bearish" | "very_bearish",
  "sentimentScore": <number -100 to 100>,
  "confidence": <number 0-100>,
  "factors": {
    "technical": { "score": <number>, "description": "<brief>" },
    "momentum": { "score": <number>, "description": "<brief>" },
    "volatility": { "score": <number>, "description": "<brief>" },
    "trend": { "score": <number>, "description": "<brief>" },
    "volume": { "score": <number>, "description": "<brief>" }
  },
  "keyLevels": {
    "support": [<price1>, <price2>],
    "resistance": [<price1>, <price2>]
  },
  "marketPhase": "accumulation" | "mark-up" | "distribution" | "mark-down",
  "tradingRecommendation": "<one actionable recommendation>"
}

Respond ONLY with valid JSON.`;

    try {
      const completion = await this.zai!.chat.completions.create({
        messages: [
          { role: 'system', content: 'You are a market sentiment analyst. Respond only with valid JSON.' },
          { role: 'user', content: prompt }
        ],
        temperature: 0.4,
        max_tokens: 400
      });

      const responseText = completion.choices[0]?.message?.content || '';
      const jsonMatch = responseText.match(/\{[\s\S]*\}/);
      
      if (!jsonMatch) {
        return this.getDefaultSentiment(context, indicators, technicalScore, momentumScore);
      }

      const parsed = JSON.parse(jsonMatch[0]);
      
      const sentiment: SentimentAnalysis = {
        symbol: context.symbol,
        overallSentiment: parsed.overallSentiment || 'neutral',
        sentimentScore: Math.max(-100, Math.min(100, parsed.sentimentScore || 0)),
        confidence: Math.min(100, Math.max(0, parsed.confidence || 50)),
        factors: parsed.factors || {
          technical: { score: technicalScore, description: 'Based on technical indicators' },
          momentum: { score: momentumScore, description: 'Based on price momentum' },
          volatility: { score: volatilityScore, description: 'Market volatility assessment' },
          trend: { score: trendScore, description: 'Trend direction and strength' },
          volume: { score: volumeScore, description: 'Volume analysis' }
        },
        keyLevels: parsed.keyLevels || {
          support: [context.currentPrice * 0.98, context.currentPrice * 0.96],
          resistance: [context.currentPrice * 1.02, context.currentPrice * 1.04]
        },
        marketPhase: parsed.marketPhase || 'accumulation',
        tradingRecommendation: parsed.tradingRecommendation || 'Wait for clearer signals',
        timestamp: Date.now()
      };

      this.setCache(cacheKey, sentiment);
      return sentiment;
    } catch (error) {
      console.error('Sentiment analysis error:', error);
      return this.getDefaultSentiment(context, indicators, technicalScore, momentumScore);
    }
  }

  /**
   * Generate real-time trading insights
   */
  async generateInsights(
    suggestions: AITradeSuggestion[],
    sentiments: SentimentAnalysis[]
  ): Promise<AIInsight[]> {
    await this.initialize();

    const insights: AIInsight[] = [];

    // High confidence trade opportunities
    const highConfidenceTrades = suggestions.filter(s => s.confidence >= 75 && s.action !== 'HOLD');
    for (const trade of highConfidenceTrades) {
      insights.push({
        type: 'opportunity',
        title: `High Confidence ${trade.action} Signal`,
        message: `${trade.symbol}: ${trade.action} at ${trade.entryPrice.toFixed(2)} with ${trade.confidence}% confidence. R:R = ${trade.riskRewardRatio}`,
        symbol: trade.symbol,
        action: trade.action,
        priority: 'high',
        timestamp: Date.now()
      });
    }

    // Sentiment shifts
    for (const sentiment of sentiments) {
      if (Math.abs(sentiment.sentimentScore) > 70) {
        insights.push({
          type: sentiment.sentimentScore > 0 ? 'opportunity' : 'warning',
          title: `Strong ${sentiment.overallSentiment.replace('_', ' ').toUpperCase()} Sentiment`,
          message: `${sentiment.symbol}: Sentiment score ${sentiment.sentimentScore}. ${sentiment.tradingRecommendation}`,
          symbol: sentiment.symbol,
          priority: 'medium',
          timestamp: Date.now()
        });
      }
    }

    return insights;
  }

  /**
   * Quick market analysis for scanner
   */
  async quickAnalysis(context: MarketContext, indicators: TechnicalIndicators): Promise<{
    signal: 'strong_buy' | 'buy' | 'hold' | 'sell' | 'strong_sell';
    score: number;
    summary: string;
  }> {
    const cacheKey = `quick-${context.symbol}-${Math.floor(Date.now() / 30000)}`;
    const cached = this.getCached<{ signal: string; score: number; summary: string }>(cacheKey);
    if (cached) return cached as any;

    // Fast local calculation for immediate response
    let score = 50;
    
    // RSI contribution
    if (indicators.rsi < 30) score += 15;
    else if (indicators.rsi < 40) score += 8;
    else if (indicators.rsi > 70) score -= 15;
    else if (indicators.rsi > 60) score -= 8;

    // MACD contribution
    if (indicators.macd.histogram > 0) score += 10;
    else score -= 10;

    // MA alignment
    if (context.currentPrice > indicators.ma20) score += 5;
    if (context.currentPrice > indicators.ma50) score += 5;
    if (indicators.ma20 > indicators.ma50) score += 5;

    // ADX trend strength
    if (indicators.adx > 25) {
      score += context.trend === 'bullish' ? 10 : -10;
    }

    // Bollinger position
    const bbPosition = (context.currentPrice - indicators.bollingerBands.lower) / 
                       (indicators.bollingerBands.upper - indicators.bollingerBands.lower);
    if (bbPosition < 0.2) score += 8;
    else if (bbPosition > 0.8) score -= 8;

    score = Math.max(0, Math.min(100, score));

    let signal: 'strong_buy' | 'buy' | 'hold' | 'sell' | 'strong_sell';
    if (score >= 80) signal = 'strong_buy';
    else if (score >= 60) signal = 'buy';
    else if (score >= 40) signal = 'hold';
    else if (score >= 20) signal = 'sell';
    else signal = 'strong_sell';

    const summary = signal === 'hold' 
      ? 'No clear directional bias. Wait for confirmation.'
      : `${signal.replace('_', ' ').toUpperCase()} signal detected with score ${score}/100`;

    const result = { signal, score, summary };
    this.setCache(cacheKey, result);
    return result;
  }

  // Helper calculation methods
  private calculateTechnicalScore(indicators: TechnicalIndicators): number {
    let score = 0;
    
    // RSI (-30 to +30)
    if (indicators.rsi < 30) score += 30;
    else if (indicators.rsi < 40) score += 15;
    else if (indicators.rsi > 70) score -= 30;
    else if (indicators.rsi > 60) score -= 15;

    // MACD (-20 to +20)
    if (indicators.macd.histogram > 0) score += 20;
    else score -= 20;

    // MA alignment (-20 to +20)
    if (indicators.ma20 > indicators.ma50) score += 10;
    else score -= 10;
    if (indicators.ma50 > indicators.ma200) score += 10;
    else score -= 10;

    return Math.max(-100, Math.min(100, score));
  }

  private calculateMomentumScore(indicators: TechnicalIndicators, data: MarketDataPoint[]): number {
    let score = 0;
    
    // Stochastic
    if (indicators.stochastic.k > indicators.stochastic.d) score += 15;
    else score -= 15;
    
    if (indicators.stochastic.k < 20) score += 10;
    else if (indicators.stochastic.k > 80) score -= 10;

    // Williams %R
    if (indicators.williamsR < -80) score += 15;
    else if (indicators.williamsR > -20) score -= 15;

    // Price momentum
    if (data.length >= 5) {
      const recentClose = data[data.length - 1].close;
      const olderClose = data[data.length - 5].close;
      const priceChange = ((recentClose - olderClose) / olderClose) * 100;
      score += Math.max(-30, Math.min(30, priceChange * 10));
    }

    return Math.max(-100, Math.min(100, score));
  }

  private calculateVolatilityScore(indicators: TechnicalIndicators): number {
    // ATR-based volatility (normalized 0-100)
    return Math.min(100, indicators.atr * 1000);
  }

  private calculateTrendScore(indicators: TechnicalIndicators, context: MarketContext): number {
    let score = 0;
    
    // ADX strength
    if (indicators.adx > 40) score += 20;
    else if (indicators.adx > 25) score += 10;
    else score -= 10;

    // Trend direction
    if (context.trend === 'bullish') score += 30;
    else if (context.trend === 'bearish') score -= 30;

    return Math.max(-100, Math.min(100, score));
  }

  private calculateVolumeScore(data: MarketDataPoint[]): number {
    if (data.length < 10) return 50;
    
    const recentVolume = data.slice(-5).reduce((sum, d) => sum + (d.volume || 0), 0) / 5;
    const olderVolume = data.slice(-10, -5).reduce((sum, d) => sum + (d.volume || 0), 0) / 5;
    
    if (recentVolume > olderVolume * 1.5) return 80;
    if (recentVolume > olderVolume) return 60;
    if (recentVolume < olderVolume * 0.5) return 20;
    return 50;
  }

  private getDefaultSuggestion(context: MarketContext, indicators: TechnicalIndicators): AITradeSuggestion {
    const atr = indicators.atr || context.currentPrice * 0.01;
    
    return {
      id: `ai-${context.symbol}-${Date.now()}`,
      symbol: context.symbol,
      action: 'HOLD',
      confidence: 50,
      entryPrice: context.currentPrice,
      stopLoss: context.currentPrice - (atr * 2),
      takeProfit: context.currentPrice + (atr * 4),
      riskRewardRatio: 2,
      positionSizePercent: 1,
      timeframe: 'intraday',
      reasoning: 'Market conditions are unclear. Waiting for better entry signals.',
      technicalReasons: ['Mixed technical signals', 'No clear trend direction'],
      riskFactors: ['Market uncertainty', 'Potential volatility'],
      marketConditions: 'Neutral market conditions',
      expiresAt: Date.now() + 3600000,
      createdAt: Date.now()
    };
  }

  private getDefaultSentiment(
    context: MarketContext, 
    indicators: TechnicalIndicators,
    technicalScore: number,
    momentumScore: number
  ): SentimentAnalysis {
    const totalScore = (technicalScore + momentumScore) / 2;
    let sentiment: 'very_bullish' | 'bullish' | 'neutral' | 'bearish' | 'very_bearish';
    
    if (totalScore > 60) sentiment = 'very_bullish';
    else if (totalScore > 20) sentiment = 'bullish';
    else if (totalScore > -20) sentiment = 'neutral';
    else if (totalScore > -60) sentiment = 'bearish';
    else sentiment = 'very_bearish';

    return {
      symbol: context.symbol,
      overallSentiment: sentiment,
      sentimentScore: totalScore,
      confidence: 60,
      factors: {
        technical: { score: technicalScore, description: 'Technical indicators analysis' },
        momentum: { score: momentumScore, description: 'Momentum analysis' },
        volatility: { score: 50, description: 'Moderate volatility' },
        trend: { score: totalScore, description: 'Trend analysis' },
        volume: { score: 50, description: 'Volume analysis' }
      },
      keyLevels: {
        support: [context.currentPrice * 0.98, context.currentPrice * 0.96],
        resistance: [context.currentPrice * 1.02, context.currentPrice * 1.04]
      },
      marketPhase: 'accumulation',
      tradingRecommendation: 'Wait for clearer market signals',
      timestamp: Date.now()
    };
  }
}

// Singleton export
export const aiAnalysisService = new AIAnalysisService();
