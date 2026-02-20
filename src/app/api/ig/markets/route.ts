import { NextRequest, NextResponse } from 'next/server';
import { DEFAULT_MARKETS, MARKET_NAMES } from '@/types/ig';

// Mock market data for demo
function generateMockMarketData(epic: string) {
  const basePrice = epic.includes('GOLD') ? 2000 + Math.random() * 100 :
                    epic.includes('EURUSD') ? 1.08 + Math.random() * 0.02 :
                    epic.includes('GBPUSD') ? 1.26 + Math.random() * 0.02 :
                    epic.includes('USDJPY') ? 149 + Math.random() * 2 :
                    epic.includes('AUDUSD') ? 0.65 + Math.random() * 0.02 : 100;
  
  const spread = basePrice * 0.0001;
  const change = (Math.random() - 0.5) * 0.5;

  return {
    epic,
    instrumentName: MARKET_NAMES[epic] || epic,
    instrumentType: epic.includes('GOLD') ? 'COMMODITY' : 'CURRENCY',
    expiry: '-',
    streamingPricesAvailable: true,
    marketStatus: 'TRADEABLE',
    bid: basePrice - spread / 2,
    ask: basePrice + spread / 2,
    high: basePrice + Math.random() * 0.5,
    low: basePrice - Math.random() * 0.5,
    change,
    changePercent: (change / basePrice) * 100,
    delayTime: 0
  };
}

// GET - Get markets
export async function GET(request: NextRequest) {
  const searchParams = request.nextUrl.searchParams;
  const epic = searchParams.get('epic');
  const mockMode = searchParams.get('mockMode') !== 'false';

  try {
    if (epic) {
      // Return single market
      if (mockMode) {
        return NextResponse.json({
          success: true,
          market: generateMockMarketData(epic)
        });
      }

      // Real API call would go here
      return NextResponse.json({
        success: true,
        market: generateMockMarketData(epic)
      });
    }

    // Return all default markets
    const epics = Object.values(DEFAULT_MARKETS);
    const markets = epics.map(e => generateMockMarketData(e));

    return NextResponse.json({
      success: true,
      markets,
      timestamp: new Date().toISOString()
    });

  } catch (error) {
    console.error('Markets error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get markets'
    }, { status: 500 });
  }
}
