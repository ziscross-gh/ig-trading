import { NextRequest, NextResponse } from 'next/server';
import { PreFlightChecker } from '@/lib/preflight-checks';

export async function GET(request: NextRequest) {
  try {
    const searchParams = request.nextUrl.searchParams;
    const environment = (searchParams.get('environment') || 'demo') as 'demo' | 'live';
    const apiKey = searchParams.get('apiKey') || undefined;
    const identifier = searchParams.get('identifier') || undefined;
    const password = searchParams.get('password') || undefined;

    const checker = new PreFlightChecker();
    const result = await checker.runAllChecks(apiKey, identifier, password, environment);

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
