import { NextRequest, NextResponse } from 'next/server';

// Generate mock historical candles
function generateMockCandles(epic: string, count: number = 100) {
  const basePrice = epic.includes('GOLD') ? 2000 :
                    epic.includes('EURUSD') ? 1.08 :
                    epic.includes('GBPUSD') ? 1.26 :
                    epic.includes('USDJPY') ? 149 :
                    epic.includes('AUDUSD') ? 0.65 : 100;
  
  const candles = [];
  let price = basePrice;
  const now = Date.now();
  const hourMs = 60 * 60 * 1000;

  for (let i = count - 1; i >= 0; i--) {
    const change = (Math.random() - 0.5) * 0.02 * price;
    const open = price;
    const close = price + change;
    const high = Math.max(open, close) + Math.random() * 0.01 * price;
    const low = Math.min(open, close) - Math.random() * 0.01 * price;
    
    candles.push({
      open,
      high,
      low,
      close,
      volume: Math.floor(Math.random() * 10000) + 1000,
      timestamp: new Date(now - i * hourMs)
    });
    
    price = close;
  }

  return candles;
}

// GET - Get price history
export async function GET(request: NextRequest) {
  const searchParams = request.nextUrl.searchParams;
  const epic = searchParams.get('epic') || 'CS.D.GOLDUSD.CFD';
  const resolution = searchParams.get('resolution') || 'HOUR';
  const max = parseInt(searchParams.get('max') || '100');
  const mockMode = searchParams.get('mockMode') !== 'false';

  try {
    if (mockMode) {
      const candles = generateMockCandles(epic, max);
      return NextResponse.json({
        success: true,
        epic,
        resolution,
        candles,
        count: candles.length,
        timestamp: new Date().toISOString()
      });
    }

    // Real API call would go here
    const candles = generateMockCandles(epic, max);
    return NextResponse.json({
      success: true,
      epic,
      resolution,
      candles,
      count: candles.length,
      timestamp: new Date().toISOString()
    });

  } catch (error) {
    console.error('History error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get price history'
    }, { status: 500 });
  }
}
