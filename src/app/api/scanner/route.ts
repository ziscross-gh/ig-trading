import { NextRequest, NextResponse } from 'next/server';
import { getMarketScanner } from '@/lib/market-scanner';

// GET - Run market scan
export async function GET(request: NextRequest) {
  try {
    const searchParams = request.nextUrl.searchParams;
    const action = searchParams.get('action');
    const scanner = getMarketScanner();

    switch (action) {
      case 'top':
        const limit = parseInt(searchParams.get('limit') || '5');
        const top = scanner.getTopOpportunities(limit);
        return NextResponse.json({
          success: true,
          opportunities: top,
          timestamp: new Date().toISOString()
        });

      case 'single':
        const epic = searchParams.get('epic');
        if (!epic) {
          return NextResponse.json({ success: false, error: 'Epic required' }, { status: 400 });
        }
        const singleResult = await scanner.scanMarket(epic);
        return NextResponse.json({
          success: true,
          result: singleResult,
          timestamp: new Date().toISOString()
        });

      default:
        // Default: run full scan
        const allResults = await scanner.scanAll();
        return NextResponse.json({
          success: true,
          results: allResults,
          count: allResults.length,
          timestamp: new Date().toISOString()
        });
    }

  } catch (error) {
    console.error('Scanner error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Scan failed'
    }, { status: 500 });
  }
}
