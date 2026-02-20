import { NextRequest, NextResponse } from 'next/server';

// Bot state (in-memory for demo)
let botState = {
  isRunning: false,
  status: 'stopped' as 'stopped' | 'running' | 'paused',
  config: {
    maxRiskPerTrade: 1,
    maxDailyLoss: 5,
    maxPositions: 3,
    tradingEnabled: true
  },
  stats: {
    totalTrades: 0,
    winRate: 0,
    dailyPnl: 0,
    openPositions: 0
  },
  logs: [] as Array<{ timestamp: Date; message: string; type: 'info' | 'warning' | 'error' | 'success' }>
};

// GET - Get bot status or logs
export async function GET(request: NextRequest) {
  const searchParams = request.nextUrl.searchParams;
  const action = searchParams.get('action');

  if (action === 'logs') {
    const limit = parseInt(searchParams.get('limit') || '50');
    return NextResponse.json({
      success: true,
      logs: botState.logs.slice(-limit)
    });
  }

  // Default: return status
  return NextResponse.json({
    success: true,
    isRunning: botState.isRunning,
    status: botState.status,
    config: botState.config,
    stats: botState.stats
  });
}

// POST - Control bot
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { action, config, strategies } = body;

    switch (action) {
      case 'start':
        botState.isRunning = true;
        botState.status = 'running';
        botState.logs.push({
          timestamp: new Date(),
          message: 'Bot started',
          type: 'success'
        });
        break;

      case 'stop':
        botState.isRunning = false;
        botState.status = 'stopped';
        botState.logs.push({
          timestamp: new Date(),
          message: 'Bot stopped',
          type: 'info'
        });
        break;

      case 'pause':
        botState.status = 'paused';
        botState.logs.push({
          timestamp: new Date(),
          message: 'Bot paused',
          type: 'warning'
        });
        break;

      case 'updateConfig':
        if (config) {
          botState.config = { ...botState.config, ...config };
          botState.logs.push({
            timestamp: new Date(),
            message: 'Configuration updated',
            type: 'info'
          });
        }
        break;

      case 'updateStrategies':
        botState.logs.push({
          timestamp: new Date(),
          message: 'Strategies updated',
          type: 'info'
        });
        break;

      case 'toggleStrategy':
        const { name, enabled } = body;
        botState.logs.push({
          timestamp: new Date(),
          message: `Strategy ${name} ${enabled ? 'enabled' : 'disabled'}`,
          type: 'info'
        });
        break;

      case 'clearLogs':
        botState.logs = [];
        break;

      default:
        return NextResponse.json({
          success: false,
          error: 'Unknown action'
        }, { status: 400 });
    }

    return NextResponse.json({
      success: true,
      isRunning: botState.isRunning,
      status: botState.status,
      config: botState.config,
      stats: botState.stats
    });

  } catch (error) {
    console.error('Bot control error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error'
    }, { status: 500 });
  }
}
