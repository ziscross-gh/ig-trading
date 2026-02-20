import { NextRequest, NextResponse } from 'next/server';
import { getAutoTradingEngine } from '@/lib/auto-trading';

// GET - Get auto-trading status and config
export async function GET(request: NextRequest) {
  try {
    const searchParams = request.nextUrl.searchParams;
    const action = searchParams.get('action');
    const engine = getAutoTradingEngine();

    switch (action) {
      case 'config':
        return NextResponse.json({
          success: true,
          config: engine.getConfig()
        });

      case 'stats':
        return NextResponse.json({
          success: true,
          stats: engine.getDailyStats(),
          enabled: engine.isEnabled()
        });

      case 'signals':
        const limit = parseInt(searchParams.get('limit') || '50');
        return NextResponse.json({
          success: true,
          signals: engine.getRecentSignals(limit)
        });

      default:
        return NextResponse.json({
          success: true,
          enabled: engine.isEnabled(),
          config: engine.getConfig(),
          stats: engine.getDailyStats()
        });
    }

  } catch (error) {
    console.error('Auto-trading error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get auto-trading status'
    }, { status: 500 });
  }
}

// POST - Control auto-trading
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { action, signal, config } = body;
    const engine = getAutoTradingEngine();

    switch (action) {
      case 'enable':
        engine.setEnabled(true);
        return NextResponse.json({
          success: true,
          enabled: true,
          message: 'Auto-trading enabled'
        });

      case 'disable':
        engine.setEnabled(false);
        return NextResponse.json({
          success: true,
          enabled: false,
          message: 'Auto-trading disabled'
        });

      case 'toggle':
        const newState = !engine.isEnabled();
        engine.setEnabled(newState);
        return NextResponse.json({
          success: true,
          enabled: newState,
          message: `Auto-trading ${newState ? 'enabled' : 'disabled'}`
        });

      case 'updateConfig':
        engine.updateConfig(config);
        return NextResponse.json({
          success: true,
          config: engine.getConfig()
        });

      case 'processSignal':
        if (!signal) {
          return NextResponse.json({
            success: false,
            error: 'Signal required'
          }, { status: 400 });
        }
        const decision = engine.processSignal(signal);
        return NextResponse.json({
          success: true,
          decision
        });

      case 'execute':
        if (!signal) {
          return NextResponse.json({
            success: false,
            error: 'Decision required'
          }, { status: 400 });
        }
        const result = await engine.executeDecision(signal);
        return NextResponse.json({
          success: result.success,
          tradeId: result.tradeId,
          error: result.error
        });

      default:
        return NextResponse.json({
          success: false,
          error: 'Unknown action'
        }, { status: 400 });
    }

  } catch (error) {
    console.error('Auto-trading action error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Auto-trading action failed'
    }, { status: 500 });
  }
}
