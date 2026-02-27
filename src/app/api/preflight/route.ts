import { NextResponse } from 'next/server';
import { PreFlightChecker } from '@/lib/preflight-checks';

/**
 * Pre-flight checks API route.
 * Credentials are handled by the Rust engine — this route only
 * queries engine status and validates dashboard-side config.
 */
export async function GET() {
  try {
    const checker = new PreFlightChecker();
    const result = await checker.runAllChecks();

    return NextResponse.json({
      success: true,
      ...result,
      timestamp: new Date().toISOString()
    });
  } catch (error) {
    console.error('Pre-flight check error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Pre-flight check failed',
      timestamp: new Date().toISOString()
    }, { status: 500 });
  }
}
