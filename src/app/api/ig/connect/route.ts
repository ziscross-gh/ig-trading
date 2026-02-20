import { NextRequest, NextResponse } from 'next/server';
import { IGClient } from '@/lib/ig-client';

// Global IG client instance
let igClient: IGClient | null = null;

// POST - Connect to IG
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { apiKey, identifier, password, environment = 'demo' } = body;

    // Create new client
    igClient = new IGClient(apiKey, identifier, password, environment);
    
    // Authenticate
    const session = await igClient.authenticate();
    
    // Get account info
    const accounts = await igClient.getAccounts();
    const account = accounts[0];

    return NextResponse.json({
      success: true,
      authenticated: true,
      isDemo: environment === 'demo',
      session: {
        accountId: session.accountId,
        lightstreamerEndpoint: session.lightstreamerEndpoint
      },
      account: {
        accountId: account?.accountId,
        accountName: account?.accountName,
        accountType: account?.accountType,
        balance: account?.balance?.balance || 0,
        available: account?.balance?.available || 0,
        profitLoss: account?.balance?.profitLoss || 0,
        currency: account?.currency || 'USD'
      }
    });

  } catch (error) {
    console.error('IG connect error:', error);
    return NextResponse.json({
      success: false,
      error: error instanceof Error ? error.message : 'Connection failed'
    }, { status: 500 });
  }
}

// GET - Check connection status
export async function GET() {
  return NextResponse.json({
    success: true,
    authenticated: igClient?.isAuthenticated() || false
  });
}
