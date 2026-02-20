import { NextRequest, NextResponse } from 'next/server';
import { MARKET_NAMES } from '@/types/ig';

// In-memory trade history for demo
let tradeHistory: any[] = [];

// GET - Get trade history
export async function GET(request: NextRequest) {
  const searchParams = request.nextUrl.searchParams;
  const limit = parseInt(searchParams.get('limit') || '50');

  return NextResponse.json({
    success: true,
    trades: tradeHistory.slice(-limit),
    count: tradeHistory.length
  });
}

// POST - Execute trade
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { action, epic, direction, size, stopLevel, limitLevel, mockMode = true } = body;

    if (action === 'open') {
      const dealId = `DEAL_${Date.now()}`;
      const price = epic?.includes('GOLD') ? 2000 + Math.random() * 10 : 1.1;

      const trade = {
        dealId,
        dealReference: `REF_${Date.now()}`,
        epic,
        direction,
        size,
        level: price,
        stopLevel,
        limitLevel,
        status: 'SUCCESS',
        timestamp: new Date().toISOString()
      };

      tradeHistory.push(trade);

      return NextResponse.json({
        success: true,
        ...trade
      });
    }

    if (action === 'close') {
      const { dealId } = body;
      
      const trade = {
        dealId,
        dealReference: `REF_${Date.now()}`,
        status: 'CLOSED',
        closeTime: new Date().toISOString()
      };

      tradeHistory.push(trade);

      return NextResponse.json({
        success: true,
        ...trade
      });
    }

    return NextResponse.json({
      success: false,
      error: 'Unknown action'
    }, { status: 400 });

  } catch (error) {
    console.error('Trade error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Trade failed'
    }, { status: 500 });
  }
}
