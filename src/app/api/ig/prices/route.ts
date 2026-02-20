import { NextRequest, NextResponse } from 'next/server';
import { DEFAULT_MARKETS, MARKET_NAMES } from '@/types/ig';

// Generate mock price data
function generateMockPrice(epic: string) {
  const basePrice = epic.includes('GOLD') ? 2000 + Math.random() * 50 :
                    epic.includes('EURUSD') ? 1.08 + Math.random() * 0.02 :
                    epic.includes('GBPUSD') ? 1.26 + Math.random() * 0.02 :
                    epic.includes('USDJPY') ? 149 + Math.random() * 2 :
                    epic.includes('AUDUSD') ? 0.65 + Math.random() * 0.02 : 100;
  
  const spread = basePrice * 0.0001;

  return {
    epic,
    name: MARKET_NAMES[epic] || epic,
    bid: basePrice - spread / 2,
    ask: basePrice + spread / 2,
    last: basePrice,
    change: (Math.random() - 0.5) * 0.5,
    changePercent: (Math.random() - 0.5) * 0.5,
    high: basePrice + Math.random() * 0.3,
    low: basePrice - Math.random() * 0.3,
    timestamp: new Date().toISOString()
  };
}

// GET - Get current prices
export async function GET(request: NextRequest) {
  const searchParams = request.nextUrl.searchParams;
  const epics = searchParams.get('epics')?.split(',') || Object.values(DEFAULT_MARKETS);
  const mockMode = searchParams.get('mockMode') !== 'false';

  try {
    const prices = epics.map(epic => generateMockPrice(epic));

    return NextResponse.json({
      success: true,
      prices,
      timestamp: new Date().toISOString()
    });

  } catch (error) {
    console.error('Prices error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get prices'
    }, { status: 500 });
  }
}
