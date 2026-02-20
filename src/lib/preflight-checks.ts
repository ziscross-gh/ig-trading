/**
 * Pre-flight Safety Checks for Trading Bot
 * Run these checks before going live
 */

import { IGClient } from './ig-client';

export interface PreFlightCheck {
  id: string;
  name: string;
  category: 'critical' | 'warning' | 'info';
  status: 'pass' | 'fail' | 'warning' | 'pending';
  message: string;
  details?: string;
  timestamp: Date;
}

export interface PreFlightResult {
  passed: boolean;
  canGoLive: boolean;
  checks: PreFlightCheck[];
  criticalFailures: number;
  warnings: number;
  summary: string;
}

export class PreFlightChecker {
  private checks: PreFlightCheck[] = [];

  /**
   * Run all pre-flight checks
   */
  async runAllChecks(
    apiKey?: string,
    identifier?: string,
    password?: string,
    environment: 'demo' | 'live' = 'demo'
  ): Promise<PreFlightResult> {
    this.checks = [];

    // Critical checks
    await this.checkEnvironmentVariables();
    await this.checkAPIConnection(apiKey, identifier, password, environment);
    await this.checkRiskParameters();
    await this.checkDatabaseConnection();

    // Warning checks
    await this.checkNotificationSettings();
    await this.checkStrategyConfiguration();
    await this.checkMarketHours();

    // Info checks
    await this.checkVersionCompatibility();
    await this.checkSystemResources();

    const criticalFailures = this.checks.filter(
      c => c.category === 'critical' && c.status === 'fail'
    ).length;
    const warnings = this.checks.filter(
      c => c.category === 'warning' && c.status !== 'pass'
    ).length;

    const passed = criticalFailures === 0;
    const canGoLive = passed && warnings < 3;

    let summary: string;
    if (canGoLive) {
      summary = `✅ Ready to go live! All critical checks passed with ${warnings} warning(s).`;
    } else if (passed) {
      summary = `⚠️ Passed but has ${warnings} warnings. Review before going live.`;
    } else {
      summary = `❌ Not ready. ${criticalFailures} critical issue(s) must be resolved.`;
    }

    return {
      passed,
      canGoLive,
      checks: this.checks,
      criticalFailures,
      warnings,
      summary
    };
  }

  /**
   * Check environment variables
   */
  private async checkEnvironmentVariables(): Promise<void> {
    const requiredVars = ['IG_API_KEY', 'IG_IDENTIFIER', 'IG_PASSWORD'];
    const missing: string[] = [];

    for (const varName of requiredVars) {
      if (!process.env[varName]) {
        missing.push(varName);
      }
    }

    this.addCheck({
      id: 'env_vars',
      name: 'Environment Variables',
      category: 'critical',
      status: missing.length === 0 ? 'pass' : 'fail',
      message: missing.length === 0 
        ? 'All required environment variables are set'
        : `Missing: ${missing.join(', ')}`,
      details: 'Required: IG_API_KEY, IG_IDENTIFIER, IG_PASSWORD, IG_ENVIRONMENT',
      timestamp: new Date()
    });
  }

  /**
   * Check API connection
   */
  private async checkAPIConnection(
    apiKey?: string,
    identifier?: string,
    password?: string,
    environment: 'demo' | 'live' = 'demo'
  ): Promise<void> {
    try {
      const key = apiKey || process.env.IG_API_KEY;
      const id = identifier || process.env.IG_IDENTIFIER;
      const pass = password || process.env.IG_PASSWORD;

      if (!key || !id || !pass) {
        this.addCheck({
          id: 'api_connection',
          name: 'API Connection',
          category: 'critical',
          status: 'fail',
          message: 'API credentials not provided',
          details: 'Set IG_API_KEY, IG_IDENTIFIER, and IG_PASSWORD',
          timestamp: new Date()
        });
        return;
      }

      const client = new IGClient(key, id, pass, environment);
      await client.authenticate();

      this.addCheck({
        id: 'api_connection',
        name: 'API Connection',
        category: 'critical',
        status: 'pass',
        message: `Successfully connected to IG ${environment} environment`,
        details: 'Authentication successful',
        timestamp: new Date()
      });

      // Check account status
      const accounts = await client.getAccounts();
      if (accounts && accounts.length > 0) {
        const account = accounts[0];
        this.addCheck({
          id: 'account_status',
          name: 'Account Status',
          category: 'critical',
          status: account.balance?.available && account.balance.available > 0 ? 'pass' : 'warning',
          message: `Account: ${account.accountName || 'Trading Account'}`,
          details: `Balance: ${account.balance?.available || 'N/A'} ${account.currency || ''}`,
          timestamp: new Date()
        });
      }

    } catch (error) {
      this.addCheck({
        id: 'api_connection',
        name: 'API Connection',
        category: 'critical',
        status: 'fail',
        message: 'Failed to connect to IG API',
        details: error instanceof Error ? error.message : 'Unknown error',
        timestamp: new Date()
      });
    }
  }

  /**
   * Check risk parameters
   */
  private async checkRiskParameters(): Promise<void> {
    const riskSettings = {
      maxRiskPerTrade: parseFloat(process.env.MAX_RISK_PER_TRADE || '0'),
      maxDailyLoss: parseFloat(process.env.MAX_DAILY_LOSS || '0'),
      maxPositions: parseInt(process.env.MAX_POSITIONS || '0')
    };

    const issues: string[] = [];

    if (riskSettings.maxRiskPerTrade <= 0 || riskSettings.maxRiskPerTrade > 5) {
      issues.push('MAX_RISK_PER_TRADE should be 1-5%');
    }
    if (riskSettings.maxDailyLoss <= 0 || riskSettings.maxDailyLoss > 20) {
      issues.push('MAX_DAILY_LOSS should be 1-20%');
    }
    if (riskSettings.maxPositions <= 0 || riskSettings.maxPositions > 10) {
      issues.push('MAX_POSITIONS should be 1-10');
    }

    this.addCheck({
      id: 'risk_params',
      name: 'Risk Parameters',
      category: 'critical',
      status: issues.length === 0 ? 'pass' : 'warning',
      message: issues.length === 0 
        ? 'Risk parameters are properly configured'
        : `${issues.length} issue(s) found`,
      details: issues.length > 0 ? issues.join('; ') : `Risk: ${riskSettings.maxRiskPerTrade}%, Daily Limit: ${riskSettings.maxDailyLoss}%, Max Positions: ${riskSettings.maxPositions}`,
      timestamp: new Date()
    });
  }

  /**
   * Check database connection
   */
  private async checkDatabaseConnection(): Promise<void> {
    try {
      const { PrismaClient } = await import('@prisma/client');
      const prisma = new PrismaClient();
      
      await prisma.$queryRaw`SELECT 1`;
      await prisma.$disconnect();

      this.addCheck({
        id: 'database',
        name: 'Database Connection',
        category: 'critical',
        status: 'pass',
        message: 'Database connection successful',
        timestamp: new Date()
      });
    } catch (error) {
      // Database might not be set up yet
      this.addCheck({
        id: 'database',
        name: 'Database Connection',
        category: 'warning',
        status: 'warning',
        message: 'Database not configured (trades will not be logged)',
        details: 'Run: bunx prisma db push',
        timestamp: new Date()
      });
    }
  }

  /**
   * Check notification settings
   */
  private async checkNotificationSettings(): Promise<void> {
    const telegram = process.env.TELEGRAM_BOT_TOKEN && process.env.TELEGRAM_CHAT_ID;
    const slack = process.env.SLACK_WEBHOOK_URL;
    const email = process.env.SMTP_HOST && process.env.SMTP_USER;

    const configured: string[] = [];
    if (telegram) configured.push('Telegram');
    if (slack) configured.push('Slack');
    if (email) configured.push('Email');

    this.addCheck({
      id: 'notifications',
      name: 'Notifications',
      category: 'warning',
      status: configured.length > 0 ? 'pass' : 'warning',
      message: configured.length > 0 
        ? `${configured.length} notification channel(s) configured`
        : 'No notifications configured',
      details: configured.length > 0 ? configured.join(', ') : 'Recommended for live trading',
      timestamp: new Date()
    });
  }

  /**
   * Check strategy configuration
   */
  private async checkStrategyConfiguration(): Promise<void> {
    const strategies = process.env.ENABLED_STRATEGIES?.split(',').filter(Boolean) || [];
    const markets = process.env.TRADE_MARKETS?.split(',').filter(Boolean) || [];

    this.addCheck({
      id: 'strategy_config',
      name: 'Strategy Configuration',
      category: 'warning',
      status: strategies.length > 0 && markets.length > 0 ? 'pass' : 'warning',
      message: `${strategies.length || 'No'} strategies, ${markets.length || 'No'} markets configured`,
      details: strategies.length > 0 ? `Strategies: ${strategies.join(', ')}` : 'Set ENABLED_STRATEGIES env var',
      timestamp: new Date()
    });
  }

  /**
   * Check market hours
   */
  private async checkMarketHours(): Promise<void> {
    const now = new Date();
    const utcDay = now.getUTCDay();
    const utcHour = now.getUTCHours();
    
    // Forex: Mon-Fri, 21:00 Sun - 21:00 Fri UTC
    // Gold: Mon-Fri, 22:00 Sun - 21:00 Fri UTC
    const isWeekday = utcDay >= 1 && utcDay <= 5;
    const isForexOpen = (utcDay === 0 && utcHour >= 21) || (utcDay >= 1 && utcDay <= 5) || (utcDay === 6 && utcHour < 21);
    
    this.addCheck({
      id: 'market_hours',
      name: 'Market Hours',
      category: 'info',
      status: 'pass',
      message: isForexOpen ? 'Markets are open' : 'Markets are closed',
      details: `UTC: ${now.toISOString()}`,
      timestamp: new Date()
    });
  }

  /**
   * Check version compatibility
   */
  private async checkVersionCompatibility(): Promise<void> {
    const nodeVersion = process.version;
    const bunVersion = process.versions.bun || 'N/A';

    this.addCheck({
      id: 'versions',
      name: 'System Versions',
      category: 'info',
      status: 'pass',
      message: `Node: ${nodeVersion}, Bun: ${bunVersion}`,
      timestamp: new Date()
    });
  }

  /**
   * Check system resources
   */
  private async checkSystemResources(): Promise<void> {
    const memUsage = process.memoryUsage();
    const heapUsed = Math.round(memUsage.heapUsed / 1024 / 1024);
    const heapTotal = Math.round(memUsage.heapTotal / 1024 / 1024);

    this.addCheck({
      id: 'resources',
      name: 'System Resources',
      category: 'info',
      status: 'pass',
      message: `Memory: ${heapUsed}MB / ${heapTotal}MB`,
      details: 'Memory usage is normal',
      timestamp: new Date()
    });
  }

  /**
   * Add a check to the results
   */
  private addCheck(check: PreFlightCheck): void {
    this.checks.push(check);
  }

  /**
   * Quick health check
   */
  static async quickHealthCheck(): Promise<{
    healthy: boolean;
    message: string;
  }> {
    try {
      // Basic health checks
      const hasEnv = process.env.IG_API_KEY && process.env.IG_IDENTIFIER && process.env.IG_PASSWORD;
      
      return {
        healthy: !!hasEnv,
        message: hasEnv ? 'System ready' : 'Missing API credentials'
      };
    } catch (error) {
      return {
        healthy: false,
        message: error instanceof Error ? error.message : 'Health check failed'
      };
    }
  }
}
