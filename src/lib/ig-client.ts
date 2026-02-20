/**
 * IG API Client
 * Handles authentication and API calls to IG Trading Platform
 */

// Types
export interface IGCredentials {
  apiKey: string;
  identifier: string;
  password: string;
  environment?: 'demo' | 'live';
}

export interface IGSession {
  cst: string;
  securityToken: string;
  accountId: string;
  lightstreamerEndpoint: string;
  currentUser: {
    accountId: string;
    userName: string;
  };
}

export interface IGAccount {
  accountId: string;
  accountName: string;
  accountType: string;
  balance: {
    available: number;
    balance: number;
    deposit: number;
    profitLoss: number;
  };
  currency: string;
}

export interface IGMarket {
  epic: string;
  instrumentName: string;
  instrumentType: string;
  expiry: string;
  streamingPricesAvailable: boolean;
  marketStatus: string;
  bid: number;
  ask: number;
  high: number;
  low: number;
  change: number;
  changePercent: number;
  delayTime: number;
}

export interface IGPosition {
  position: {
    dealId: string;
    epic: string;
    direction: 'BUY' | 'SELL';
    size: number;
    level: number;
    currency: string;
    createdDate: string;
  };
  market: {
    instrumentName: string;
    bid: number;
    ask: number;
  };
}

export interface IGTradeResult {
  dealId: string;
  dealReference: string;
  status: string;
  level: number;
}

// API Base URLs
const BASE_URLS = {
  demo: 'https://demo-api.ig.com/gateway/deal',
  live: 'https://api.ig.com/gateway/deal'
};

export class IGClient {
  private apiKey: string;
  private identifier: string;
  private password: string;
  private environment: 'demo' | 'live';
  private session: IGSession | null = null;
  private baseUrl: string;

  constructor(
    apiKey?: string,
    identifier?: string,
    password?: string,
    environment: 'demo' | 'live' = 'demo'
  ) {
    this.apiKey = apiKey || process.env.IG_API_KEY || '';
    this.identifier = identifier || process.env.IG_IDENTIFIER || '';
    this.password = password || process.env.IG_PASSWORD || '';
    this.environment = environment;
    this.baseUrl = BASE_URLS[environment];
  }

  /**
   * Authenticate with IG API
   */
  async authenticate(): Promise<IGSession> {
    const response = await fetch(`${this.baseUrl}/session`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json; charset=UTF-8',
        'Accept': 'application/json; charset=UTF-8',
        'X-IG-API-KEY': this.apiKey,
        'Version': '2'
      },
      body: JSON.stringify({
        identifier: this.identifier,
        password: this.password,
        encryptedPassword: false
      })
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Authentication failed: ${response.status} - ${errorText}`);
    }

    const data = await response.json();
    
    this.session = {
      cst: response.headers.get('CST') || '',
      securityToken: response.headers.get('X-SECURITY-TOKEN') || '',
      accountId: data.currentAccountId,
      lightstreamerEndpoint: data.lightstreamerEndpoint,
      currentUser: {
        accountId: data.currentAccountId,
        userName: data.userName
      }
    };

    return this.session;
  }

  /**
   * Get authenticated headers
   */
  private getAuthHeaders(): Record<string, string> {
    if (!this.session) {
      throw new Error('Not authenticated. Call authenticate() first.');
    }

    return {
      'Content-Type': 'application/json; charset=UTF-8',
      'Accept': 'application/json; charset=UTF-8',
      'X-IG-API-KEY': this.apiKey,
      'CST': this.session.cst,
      'X-SECURITY-TOKEN': this.session.securityToken,
      'Version': '2'
    };
  }

  /**
   * Check if authenticated
   */
  isAuthenticated(): boolean {
    return this.session !== null;
  }

  /**
   * Get accounts
   */
  async getAccounts(): Promise<IGAccount[]> {
    const response = await fetch(`${this.baseUrl}/accounts`, {
      method: 'GET',
      headers: this.getAuthHeaders()
    });

    if (!response.ok) {
      throw new Error(`Failed to get accounts: ${response.status}`);
    }

    const data = await response.json();
    return data.accounts || [];
  }

  /**
   * Get market details
   */
  async getMarket(epic: string): Promise<IGMarket> {
    const response = await fetch(`${this.baseUrl}/markets/${epic}`, {
      method: 'GET',
      headers: this.getAuthHeaders()
    });

    if (!response.ok) {
      throw new Error(`Failed to get market: ${response.status}`);
    }

    const data = await response.json();
    const market = data.snapshot || data.marketSnapshot || {};
    
    return {
      epic: epic,
      instrumentName: market.instrumentName || epic,
      instrumentType: market.instrumentType || 'UNKNOWN',
      expiry: market.expiry || '-',
      streamingPricesAvailable: market.streamingPricesAvailable || false,
      marketStatus: market.marketStatus || 'UNKNOWN',
      bid: market.bid || 0,
      ask: market.ask || 0,
      high: market.high || 0,
      low: market.low || 0,
      change: market.netChange || 0,
      changePercent: market.percentageChange || 0,
      delayTime: market.delayTime || 0
    };
  }

  /**
   * Get multiple markets
   */
  async getMarkets(epics: string[]): Promise<IGMarket[]> {
    const markets: IGMarket[] = [];
    
    for (const epic of epics) {
      try {
        const market = await this.getMarket(epic);
        markets.push(market);
      } catch (error) {
        console.error(`Failed to get market ${epic}:`, error);
      }
    }
    
    return markets;
  }

  /**
   * Get price history
   */
  async getPriceHistory(
    epic: string,
    resolution: string = 'HOUR',
    max: number = 100
  ): Promise<Array<{ open: number; high: number; low: number; close: number; volume: number; timestamp: Date }>> {
    const response = await fetch(
      `${this.baseUrl}/prices/${epic}?resolution=${resolution}&max=${max}`,
      {
        method: 'GET',
        headers: this.getAuthHeaders()
      }
    );

    if (!response.ok) {
      throw new Error(`Failed to get price history: ${response.status}`);
    }

    const data = await response.json();
    
    return (data.prices || []).map((p: any) => ({
      open: p.openPrice?.bid || p.open || 0,
      high: p.highPrice?.bid || p.high || 0,
      low: p.lowPrice?.bid || p.low || 0,
      close: p.closePrice?.bid || p.close || 0,
      volume: p.lastTradedVolume || 0,
      timestamp: new Date(p.snapshotTime || p.timestamp)
    }));
  }

  /**
   * Get open positions
   */
  async getPositions(): Promise<IGPosition[]> {
    const response = await fetch(`${this.baseUrl}/positions`, {
      method: 'GET',
      headers: this.getAuthHeaders()
    });

    if (!response.ok) {
      throw new Error(`Failed to get positions: ${response.status}`);
    }

    const data = await response.json();
    return data.positions || [];
  }

  /**
   * Open a position
   */
  async openPosition(params: {
    epic: string;
    direction: 'BUY' | 'SELL';
    size: number;
    stopLevel?: number;
    limitLevel?: number;
    guaranteedStop?: boolean;
  }): Promise<IGTradeResult> {
    const body: any = {
      epic: params.epic,
      direction: params.direction,
      size: params.size.toString(),
      orderType: 'MARKET',
      currencyCode: 'USD',
      forceOpen: true
    };

    if (params.stopLevel) {
      body.stopLevel = params.stopLevel;
      body.stopDistance = null;
    }
    if (params.limitLevel) {
      body.limitLevel = params.limitLevel;
    }
    if (params.guaranteedStop) {
      body.guaranteedStop = params.guaranteedStop;
    }

    const response = await fetch(`${this.baseUrl}/positions/otc`, {
      method: 'POST',
      headers: {
        ...this.getAuthHeaders(),
        '_method': 'POST'
      },
      body: JSON.stringify(body)
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Failed to open position: ${response.status} - ${errorText}`);
    }

    const data = await response.json();
    return {
      dealId: data.dealId,
      dealReference: data.dealReference,
      status: data.dealStatus || 'SUCCESS',
      level: data.level || 0
    };
  }

  /**
   * Close a position
   */
  async closePosition(params: {
    dealId: string;
    direction: 'BUY' | 'SELL';
    size: number;
  }): Promise<IGTradeResult> {
    const body = {
      dealId: params.dealId,
      direction: params.direction === 'BUY' ? 'SELL' : 'BUY',
      size: params.size.toString(),
      orderType: 'MARKET'
    };

    const response = await fetch(`${this.baseUrl}/positions/otc`, {
      method: 'POST',
      headers: {
        ...this.getAuthHeaders(),
        '_method': 'DELETE'
      },
      body: JSON.stringify(body)
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Failed to close position: ${response.status} - ${errorText}`);
    }

    const data = await response.json();
    return {
      dealId: data.dealId,
      dealReference: data.dealReference,
      status: data.dealStatus || 'SUCCESS',
      level: data.level || 0
    };
  }

  /**
   * Search markets
   */
  async searchMarkets(query: string): Promise<any[]> {
    const response = await fetch(
      `${this.baseUrl}/markets?searchTerm=${encodeURIComponent(query)}`,
      {
        method: 'GET',
        headers: this.getAuthHeaders()
      }
    );

    if (!response.ok) {
      throw new Error(`Failed to search markets: ${response.status}`);
    }

    const data = await response.json();
    return data.markets || [];
  }

  /**
   * Logout
   */
  async logout(): Promise<void> {
    if (!this.session) return;

    try {
      await fetch(`${this.baseUrl}/session`, {
        method: 'DELETE',
        headers: this.getAuthHeaders()
      });
    } catch (error) {
      console.error('Logout error:', error);
    }

    this.session = null;
  }
}

// Export singleton instance creator
export function createIGClient(
  apiKey?: string,
  identifier?: string,
  password?: string,
  environment?: 'demo' | 'live'
): IGClient {
  return new IGClient(apiKey, identifier, password, environment);
}
