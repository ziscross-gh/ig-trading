import { NextRequest, NextResponse } from 'next/server';
import { aiAnalysisService, MarketContext, TechnicalIndicators, MarketDataPoint } from '@/lib/ai-analysis';

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { action, context, indicators, historicalData } = body;

    switch (action) {
      case 'suggestion':
        return await handleSuggestion(context, historicalData, indicators);
      
      case 'sentiment':
        return await handleSentiment(context, historicalData, indicators);
      
      case 'insights':
        return await handleInsights(body.suggestions, body.sentiments);
      
      case 'quick':
        return await handleQuickAnalysis(context, indicators);
      
      case 'multi-analysis':
        return await handleMultiAnalysis(body.markets);
      
      default:
        return NextResponse.json(
          { success: false, error: 'Invalid action' },
          { status: 400 }
        );
    }
  } catch (error) {
    console.error('AI Analysis API error:', error);
    return NextResponse.json(
      { success: false, error: 'Internal server error' },
      { status: 500 }
    );
  }
}

async function handleSuggestion(
  context: MarketContext,
  historicalData: MarketDataPoint[],
  indicators: TechnicalIndicators
) {
  try {
    const suggestion = await aiAnalysisService.generateTradeSuggestion(
      context,
      indicators,
      historicalData
    );

    return NextResponse.json({
      success: true,
      suggestion,
      timestamp: Date.now()
    });
  } catch (error) {
    console.error('Suggestion error:', error);
    return NextResponse.json(
      { success: false, error: 'Failed to generate suggestion' },
      { status: 500 }
    );
  }
}

async function handleSentiment(
  context: MarketContext,
  historicalData: MarketDataPoint[],
  indicators: TechnicalIndicators
) {
  try {
    const sentiment = await aiAnalysisService.analyzeSentiment(
      context,
      indicators,
      historicalData
    );

    return NextResponse.json({
      success: true,
      sentiment,
      timestamp: Date.now()
    });
  } catch (error) {
    console.error('Sentiment error:', error);
    return NextResponse.json(
      { success: false, error: 'Failed to analyze sentiment' },
      { status: 500 }
    );
  }
}

async function handleInsights(
  suggestions: any[],
  sentiments: any[]
) {
  try {
    const insights = await aiAnalysisService.generateInsights(
      suggestions,
      sentiments
    );

    return NextResponse.json({
      success: true,
      insights,
      timestamp: Date.now()
    });
  } catch (error) {
    console.error('Insights error:', error);
    return NextResponse.json(
      { success: false, error: 'Failed to generate insights' },
      { status: 500 }
    );
  }
}

async function handleQuickAnalysis(
  context: MarketContext,
  indicators: TechnicalIndicators
) {
  try {
    const analysis = await aiAnalysisService.quickAnalysis(
      context,
      indicators
    );

    return NextResponse.json({
      success: true,
      analysis,
      timestamp: Date.now()
    });
  } catch (error) {
    console.error('Quick analysis error:', error);
    return NextResponse.json(
      { success: false, error: 'Failed to perform quick analysis' },
      { status: 500 }
    );
  }
}

async function handleMultiAnalysis(
  markets: Array<{
    context: MarketContext;
    indicators: TechnicalIndicators;
    historicalData: MarketDataPoint[];
  }>
) {
  try {
    const results = await Promise.all(
      markets.map(async (market) => {
        const [suggestion, sentiment, quickAnalysis] = await Promise.all([
          aiAnalysisService.generateTradeSuggestion(
            market.context,
            market.indicators,
            market.historicalData
          ),
          aiAnalysisService.analyzeSentiment(
            market.context,
            market.indicators,
            market.historicalData
          ),
          aiAnalysisService.quickAnalysis(
            market.context,
            market.indicators
          )
        ]);

        return {
          symbol: market.context.symbol,
          suggestion,
          sentiment,
          quickAnalysis
        };
      })
    );

    return NextResponse.json({
      success: true,
      results,
      timestamp: Date.now()
    });
  } catch (error) {
    console.error('Multi-analysis error:', error);
    return NextResponse.json(
      { success: false, error: 'Failed to perform multi-analysis' },
      { status: 500 }
    );
  }
}

export async function GET(request: NextRequest) {
  const { searchParams } = new URL(request.url);
  const symbol = searchParams.get('symbol');
  
  if (!symbol) {
    return NextResponse.json({
      success: false,
      error: 'Symbol parameter required'
    }, { status: 400 });
  }

  // Return mock quick analysis for testing
  const mockContext: MarketContext = {
    symbol: symbol.toUpperCase(),
    name: symbol.toUpperCase(),
    currentPrice: symbol.includes('GOLD') ? 2340.50 : 1.0850,
    dayChange: 0,
    dayChangePercent: 0,
    volume: 1000000,
    high24h: symbol.includes('GOLD') ? 2350 : 1.0900,
    low24h: symbol.includes('GOLD') ? 2330 : 1.0800,
    trend: 'bullish',
    volatility: 'medium'
  };

  const mockIndicators: TechnicalIndicators = {
    rsi: 55,
    macd: { value: 0.001, signal: 0.0005, histogram: 0.0005 },
    ma20: mockContext.currentPrice * 0.998,
    ma50: mockContext.currentPrice * 0.995,
    ma200: mockContext.currentPrice * 0.99,
    bollingerBands: {
      upper: mockContext.currentPrice * 1.01,
      middle: mockContext.currentPrice,
      lower: mockContext.currentPrice * 0.99
    },
    atr: mockContext.currentPrice * 0.005,
    adx: 25,
    stochastic: { k: 60, d: 55 },
    williamsR: -40
  };

  try {
    const quickAnalysis = await aiAnalysisService.quickAnalysis(mockContext, mockIndicators);
    
    return NextResponse.json({
      success: true,
      symbol: symbol.toUpperCase(),
      quickAnalysis,
      timestamp: Date.now()
    });
  } catch (error) {
    console.error('Quick analysis error:', error);
    return NextResponse.json({
      success: false,
      error: 'Failed to analyze'
    }, { status: 500 });
  }
}
