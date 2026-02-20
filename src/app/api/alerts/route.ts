import { NextRequest, NextResponse } from 'next/server';
import { getAlertManager } from '@/lib/alert-system';

// GET - Get alerts
export async function GET(request: NextRequest) {
  try {
    const searchParams = request.nextUrl.searchParams;
    const type = searchParams.get('type');
    const alertManager = getAlertManager();

    switch (type) {
      case 'price':
        return NextResponse.json({
          success: true,
          alerts: alertManager.getActivePriceAlerts()
        });

      case 'indicator':
        return NextResponse.json({
          success: true,
          alerts: alertManager.getActiveIndicatorAlerts()
        });

      case 'trade':
        const limit = parseInt(searchParams.get('limit') || '20');
        return NextResponse.json({
          success: true,
          alerts: alertManager.getRecentTradeAlerts(limit)
        });

      case 'stats':
        return NextResponse.json({
          success: true,
          stats: alertManager.getStats()
        });

      default:
        return NextResponse.json({
          success: true,
          priceAlerts: alertManager.getActivePriceAlerts(),
          indicatorAlerts: alertManager.getActiveIndicatorAlerts(),
          tradeAlerts: alertManager.getRecentTradeAlerts(10),
          stats: alertManager.getStats()
        });
    }

  } catch (error) {
    console.error('Alerts error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get alerts'
    }, { status: 500 });
  }
}

// POST - Create or manage alerts
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { action, alert } = body;
    const alertManager = getAlertManager();

    switch (action) {
      case 'createPrice':
        const priceAlert = alertManager.createPriceAlert(alert);
        return NextResponse.json({
          success: true,
          alert: priceAlert
        });

      case 'createIndicator':
        const indicatorAlert = alertManager.createIndicatorAlert(alert);
        return NextResponse.json({
          success: true,
          alert: indicatorAlert
        });

      case 'createTrade':
        const tradeAlert = alertManager.createTradeAlert(alert);
        return NextResponse.json({
          success: true,
          alert: tradeAlert
        });

      case 'cancel':
        const cancelled = alertManager.cancelAlert(alert.id);
        return NextResponse.json({
          success: cancelled
        });

      case 'delete':
        const deleted = alertManager.deleteAlert(alert.id);
        return NextResponse.json({
          success: deleted
        });

      case 'acknowledge':
        const acknowledged = alertManager.acknowledgeTradeAlert(alert.id);
        return NextResponse.json({
          success: acknowledged
        });

      case 'clearTriggered':
        const count = alertManager.clearTriggeredAlerts();
        return NextResponse.json({
          success: true,
          cleared: count
        });

      default:
        return NextResponse.json({
          success: false,
          error: 'Unknown action'
        }, { status: 400 });
    }

  } catch (error) {
    console.error('Alert action error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Alert action failed'
    }, { status: 500 });
  }
}
