import { NextRequest, NextResponse } from 'next/server';
import { MARKET_NAMES } from '@/types/ig';

// In-memory positions for demo
let mockPositions: any[] = [];

// GET - Get open positions
export async function GET(request: NextRequest) {
  const searchParams = request.nextUrl.searchParams;
  const mockMode = searchParams.get('mockMode') !== 'false';

  try {
    if (mockMode) {
      return NextResponse.json({
        success: true,
        positions: mockPositions,
        timestamp: new Date().toISOString()
      });
    }

    // Real API call would go here
    return NextResponse.json({
      success: true,
      positions: mockPositions,
      timestamp: new Date().toISOString()
    });

  } catch (error) {
    console.error('Positions error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get positions'
    }, { status: 500 });
  }
}

// POST - Update positions (for demo/testing)
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { action, position } = body;

    if (action === 'add') {
      mockPositions.push({
        position: {
          dealId: `DEAL_${Date.now()}`,
          epic: position.epic,
          direction: position.direction,
          size: position.size,
          level: position.level,
          currency: 'USD',
          createdDate: new Date().toISOString()
        },
        market: {
          instrumentName: MARKET_NAMES[position.epic] || position.epic,
          bid: position.level * 0.9999,
          ask: position.level * 1.0001
        }
      });
    } else if (action === 'clear') {
      mockPositions = [];
    }

    return NextResponse.json({
      success: true,
      positions: mockPositions
    });

  } catch (error) {
    console.error('Positions POST error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Failed to update positions'
    }, { status: 500 });
  }
}
