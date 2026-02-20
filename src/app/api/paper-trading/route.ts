import { NextRequest, NextResponse } from 'next/server';
import { getPaperTradingEngine, resetPaperTradingEngine } from '@/lib/paper-trading';

// GET - Get paper trading account status
export async function GET() {
  try {
    const engine = getPaperTradingEngine();
    const account = engine.getAccount();
    const performance = engine.getPerformanceMetrics();

    return NextResponse.json({
      success: true,
      account,
      performance,
      timestamp: new Date().toISOString()
    });
  } catch (error) {
    console.error('Paper trading GET error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get paper trading status',
      timestamp: new Date().toISOString()
    }, { status: 500 });
  }
}

// POST - Execute paper trading action
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { action, params } = body;
    const engine = getPaperTradingEngine(params?.initialBalance);

    switch (action) {
      case 'open': {
        const { epic, marketName, direction, size, price, stopLevel, limitLevel } = params;
        const result = engine.openPosition(epic, marketName, direction, size, price, stopLevel, limitLevel);
        return NextResponse.json({
          success: result.success,
          position: result.position,
          error: result.error,
          timestamp: new Date().toISOString()
        });
      }

      case 'close': {
        const { epic, closePrice, reason } = params;
        const result = engine.closePosition(epic, closePrice, reason);
        return NextResponse.json({
          success: result.success,
          trade: result.trade,
          error: result.error,
          timestamp: new Date().toISOString()
        });
      }

      case 'updatePrices': {
        const { prices } = params; // Map<string, number> as object
        const pricesMap = new Map(Object.entries(prices).map(([k, v]) => [k, v as number]));
        const closedTrades = engine.updatePrices(pricesMap);
        return NextResponse.json({
          success: true,
          closedTrades,
          account: engine.getAccount(),
          timestamp: new Date().toISOString()
        });
      }

      case 'reset': {
        const { initialBalance } = params || {};
        const newEngine = resetPaperTradingEngine(initialBalance);
        return NextResponse.json({
          success: true,
          account: newEngine.getAccount(),
          message: 'Paper trading account reset',
          timestamp: new Date().toISOString()
        });
      }

      case 'resetDaily': {
        engine.resetDailyTracking();
        return NextResponse.json({
          success: true,
          message: 'Daily tracking reset',
          timestamp: new Date().toISOString()
        });
      }

      case 'resetWeekly': {
        engine.resetWeeklyTracking();
        return NextResponse.json({
          success: true,
          message: 'Weekly tracking reset',
          timestamp: new Date().toISOString()
        });
      }

      default:
        return NextResponse.json({
          success: false,
          error: 'Unknown action',
          timestamp: new Date().toISOString()
        }, { status: 400 });
    }
  } catch (error) {
    console.error('Paper trading POST error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Paper trading action failed',
      timestamp: new Date().toISOString()
    }, { status: 500 });
  }
}
