/**
 * Pre-flight Safety Checks for Trading Bot
 * Run these checks before going live
 * Note: IG API connection is handled by the Rust engine, not this dashboard
 */

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
  envConfigured: boolean;
}

interface EngineHealthData { connected_to_ig?: boolean; version?: string; uptime_secs?: number; }
interface EngineStatusData { account?: { balance: number; available: number }; mode?: string; open_positions?: number; circuit_breaker?: { is_paused: boolean; consecutive_losses: number; size_multiplier: number } }
interface EngineConfigData { max_risk_per_trade?: number; max_daily_loss_pct?: number; strategies?: Record<string, unknown>; markets?: string[]; min_consensus?: number }

export class PreFlightChecker {
  private checks: PreFlightCheck[] = [];

  async runAllChecks(): Promise<PreFlightResult> {
    this.checks = [];
    const engineUrl = process.env.ENGINE_INTERNAL_URL || 'http://localhost:9090';

    // 1. Fetch live data from engine
    let engineHealth = null;
    let engineStatus = null;
    let engineConfig = null;

    try {
      const [h, s, c] = await Promise.all([
        fetch(`${engineUrl}/api/health`, { signal: AbortSignal.timeout(3000) }).then(r => r.ok ? r.json() : null).catch(() => null),
        fetch(`${engineUrl}/api/status`, { signal: AbortSignal.timeout(3000) }).then(r => r.ok ? r.json() : null).catch(() => null),
        fetch(`${engineUrl}/api/config`, { signal: AbortSignal.timeout(3000) }).then(r => r.ok ? r.json() : null).catch(() => null)
      ]);
      engineHealth = h;
      engineStatus = s;
      engineConfig = c;
    } catch (e) {
      console.warn('Preflight: Failed to fetch some engine data', e);
    }

    // 2. Execute refined checks using the fetched data
    this.checkEngineConnectivity(engineHealth, engineUrl);
    this.checkIgAuthentication(engineHealth);
    this.checkAccountHealth(engineStatus);
    this.checkRiskSettings(engineConfig);
    await this.checkDatabaseIntegrity();
    this.checkMarketAvailability();

    const criticalFailures = this.checks.filter(
      c => c.category === 'critical' && c.status === 'fail'
    ).length;
    const warnings = this.checks.filter(
      c => c.status === 'warning'
    ).length;

    const passed = criticalFailures === 0;
    const canGoLive = passed && warnings < 5;
    const envConfigured = !!engineHealth;

    let summary: string;
    if (canGoLive) {
      summary = `✅ System ready. ${warnings} minor warning(s) found.`;
    } else if (passed) {
      summary = `⚠️ Critical checks passed, but review ${warnings} warning(s) before live trading.`;
    } else {
      summary = `❌ Pre-flight failed. ${criticalFailures} critical issue(s) require intervention.`;
    }

    return {
      passed,
      canGoLive,
      checks: this.checks,
      criticalFailures,
      warnings,
      summary,
      envConfigured
    };
  }

  private checkEngineConnectivity(health: EngineHealthData | null, url: string): void {
    this.addCheck({
      id: 'engine_conn',
      name: 'Rust Engine Connection',
      category: 'critical',
      status: health ? 'pass' : 'fail',
      message: health ? 'Connected to Rust core' : 'Rust engine unreachable',
      details: `URL: ${url} | Version: ${health?.version || 'N/A'}`,
      timestamp: new Date()
    });
  }

  private checkIgAuthentication(health: EngineHealthData | null): void {
    const connected = health?.connected_to_ig === true;
    this.addCheck({
      id: 'ig_auth',
      name: 'IG API Authentication',
      category: 'critical',
      status: connected ? 'pass' : 'fail',
      message: connected ? 'Authenticated with IG' : 'IG Session not established',
      details: 'Ensure credentials are correct in engine config',
      timestamp: new Date()
    });
  }

  private checkAccountHealth(status: EngineStatusData | null): void {
    const account = status?.account;
    const issues: string[] = [];

    if (!account) {
      issues.push('Account info unavailable');
    } else {
      if (account.balance <= 0) issues.push('Zero balance');
      if (account.available < (account.balance * 0.05)) issues.push('Very low margin');
    }

    this.addCheck({
      id: 'account_health',
      name: 'Account Health',
      category: 'critical',
      status: issues.length === 0 ? 'pass' : 'warning',
      message: issues.length === 0 ? 'Account ready' : 'Review account limits',
      details: issues.length > 0 ? issues.join(', ') : `Balance: $${account?.balance?.toFixed(2)} | Mode: ${status?.mode}`,
      timestamp: new Date()
    });
  }

  private checkRiskSettings(config: EngineConfigData | null): void {
    if (!config) {
      this.addCheck({
        id: 'risk_cfg',
        name: 'Risk Configuration',
        category: 'warning',
        status: 'warning',
        message: 'Using engine defaults',
        timestamp: new Date()
      });
      return;
    }

    const issues: string[] = [];
    if ((config.max_risk_per_trade || 0) > 5) issues.push('High risk/trade (>5%)');
    if ((config.max_daily_loss_pct || 0) > 10) issues.push('High daily limit (>10%)');

    this.addCheck({
      id: 'risk_cfg',
      name: 'Risk Management',
      category: 'warning',
      status: issues.length === 0 ? 'pass' : 'warning',
      message: issues.length === 0 ? 'Risk parameters safe' : 'Aggressive risk settings',
      details: issues.length > 0 ? issues.join(', ') : `Risk: ${config.max_risk_per_trade || 0}% | Daily Stop: ${config.max_daily_loss_pct || 0}%`,
      timestamp: new Date()
    });
  }

  private async checkDatabaseIntegrity(): Promise<void> {
    try {
      const { PrismaClient } = await import('@prisma/client');
      const prisma = new PrismaClient();
      await prisma.$queryRaw`SELECT 1`;
      await prisma.$disconnect();

      this.addCheck({
        id: 'db_integrity',
        name: 'Database Sync',
        category: 'warning',
        status: 'pass',
        message: 'Syncing trades to DB',
        timestamp: new Date()
      });
    } catch {
      this.addCheck({
        id: 'db_integrity',
        name: 'Database Sync',
        category: 'warning',
        status: 'warning',
        message: 'DB offline (logs cached in RAM only)',
        timestamp: new Date()
      });
    }
  }

  private checkMarketAvailability(): void {
    const now = new Date();
    const sgtOffset = 8 * 60;
    const sgtTime = new Date(now.getTime() + now.getTimezoneOffset() * 60000 + sgtOffset * 60000);
    const day = sgtTime.getDay();
    const hour = sgtTime.getHours();

    const isWeekend = (day === 6 && hour >= 6) || (day === 0) || (day === 1 && hour < 6);

    this.addCheck({
      id: 'market_status',
      name: 'Market Status',
      category: 'info',
      status: isWeekend ? 'warning' : 'pass',
      message: isWeekend ? 'Currently Weekend (Indices/Gold only)' : 'Weekday Markets Open',
      details: `SGT Time: ${sgtTime.toLocaleTimeString()}`,
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
    } catch {
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

    this.addCheck({
      id: 'notifications',
      name: 'Notifications',
      category: 'warning',
      status: telegram ? 'pass' : 'warning',
      message: telegram
        ? 'Telegram notifications configured'
        : 'No notifications configured',
      details: 'Recommended for live trading',
      timestamp: new Date()
    });
  }

  /**
   * Check strategy configuration from engine (Engine-Aware)
   */
  private async checkStrategyConfiguration(): Promise<void> {
    try {
      const engineUrl = process.env.ENGINE_INTERNAL_URL || 'http://localhost:9090';
      const response = await fetch(`${engineUrl}/api/config`, {
        method: 'GET',
        signal: AbortSignal.timeout(5000)
      });

      if (response.ok) {
        const config = await response.json();
        const enabledStrategies: string[] = [];

        // Check which strategies are enabled
        if (config.strategies.ma_crossover) enabledStrategies.push('MA_Crossover');
        if (config.strategies.rsi_divergence) enabledStrategies.push('RSI_Reversal');
        if (config.strategies.macd_momentum) enabledStrategies.push('MACD_Momentum');
        if (config.strategies.bollinger_reversion) enabledStrategies.push('Bollinger_Reversion');

        const markets = config.markets || [];
        const issues: string[] = [];

        if (enabledStrategies.length === 0) {
          issues.push('No strategies enabled');
        }
        if (markets.length === 0) {
          issues.push('No markets configured');
        }
        if (config.strategies.min_consensus > enabledStrategies.length) {
          issues.push('Min consensus higher than enabled strategies');
        }

        this.addCheck({
          id: 'strategy_config',
          name: 'Strategy Configuration',
          category: 'warning',
          status: issues.length === 0 ? 'pass' : 'warning',
          message: issues.length === 0
            ? `${enabledStrategies.length} strategies, ${markets.length} markets active`
            : `${issues.length} issue(s) found`,
          details: issues.length > 0
            ? issues.join('; ')
            : `Strategies: ${enabledStrategies.join(', ')} | Min Consensus: ${config.strategies.min_consensus} | Markets: ${markets.length}`,
          timestamp: new Date()
        });
      } else {
        throw new Error(`Engine returned ${response.status}`);
      }
    } catch (error) {
      this.addCheck({
        id: 'strategy_config',
        name: 'Strategy Configuration',
        category: 'warning',
        status: 'warning',
        message: 'Cannot retrieve strategy config from engine',
        details: error instanceof Error ? error.message : 'Check engine connection',
        timestamp: new Date()
      });
    }
  }

  /**
   * Check position management capabilities (Engine-Aware)
   */
  private async checkPositionManagement(): Promise<void> {
    try {
      const engineUrl = process.env.ENGINE_INTERNAL_URL || 'http://localhost:9090';
      const response = await fetch(`${engineUrl}/api/status`, {
        method: 'GET',
        signal: AbortSignal.timeout(5000)
      });

      if (response.ok) {
        const status = await response.json();
        const issues: string[] = [];

        // Check circuit breaker status
        if (status.circuit_breaker?.is_paused) {
          issues.push(`Circuit breaker PAUSED (${status.circuit_breaker.consecutive_losses} consecutive losses)`);
        }

        // Check for excessive open positions
        if (status.open_positions > (status.account?.balance > 0 ? 5 : 1)) {
          issues.push(`Too many open positions (${status.open_positions})`);
        }

        this.addCheck({
          id: 'position_mgmt',
          name: 'Position Management',
          category: 'warning',
          status: issues.length === 0 ? 'pass' : 'warning',
          message: issues.length === 0
            ? `Ready to trade | ${status.open_positions} open position(s)`
            : `${issues.length} issue(s) found`,
          details: issues.length > 0
            ? issues.join('; ')
            : `Open Positions: ${status.open_positions}, Size Multiplier: ${status.circuit_breaker?.size_multiplier || 1.0}x`,
          timestamp: new Date()
        });
      } else {
        throw new Error(`Engine returned ${response.status}`);
      }
    } catch (error) {
      this.addCheck({
        id: 'position_mgmt',
        name: 'Position Management',
        category: 'warning',
        status: 'warning',
        message: 'Cannot retrieve position status from engine',
        details: error instanceof Error ? error.message : 'Check engine connection',
        timestamp: new Date()
      });
    }
  }

  /**
   * Check market hours (IG trading schedule in SGT = UTC+8)
   *
   * Weekday markets:
   * - Regular: Monday 7am to Saturday 6am SGT
   * - Forex: Monday 5am to Saturday 6am SGT
   *
   * Weekend markets:
   * - Indices, Spot Gold, Spot Silver: Saturday 3pm to Monday 5:40am SGT
   * - Weekend forex (GBP/USD, EUR/USD, USD/JPY): Saturday 3pm to Monday 3:40am SGT
   */
  private async checkMarketHours(): Promise<void> {
    const now = new Date();

    // Convert to SGT (UTC+8)
    const sgtOffset = 8 * 60; // minutes
    const utcTime = now.getTime() + now.getTimezoneOffset() * 60000;
    const sgtTime = new Date(utcTime + sgtOffset * 60000);

    const sgtDay = sgtTime.getDay(); // 0=Sunday, 6=Saturday
    const sgtHour = sgtTime.getHours();
    const sgtMinute = sgtTime.getMinutes();
    const sgtTotalMinutes = sgtHour * 60 + sgtMinute;

    let weekdayMarketOpen = false;
    let weekendMarketOpen = false;
    let marketStatus = 'closed';

    // Check weekday markets (Monday 5am to Saturday 6am SGT)
    if (sgtDay >= 1 && sgtDay <= 5) {
      // Monday-Friday
      weekdayMarketOpen = true;
    } else if (sgtDay === 6 && sgtTotalMinutes < 6 * 60) {
      // Saturday before 6am
      weekdayMarketOpen = true;
    } else if (sgtDay === 0 && sgtTotalMinutes >= 5 * 60) {
      // Sunday after 5am (forex opens early)
      weekdayMarketOpen = true;
    }

    // Check weekend markets (Saturday 3pm to Monday 5:40am SGT for Gold/Indices)
    if (sgtDay === 6 && sgtTotalMinutes >= 15 * 60) {
      // Saturday after 3pm
      weekendMarketOpen = true;
    } else if (sgtDay === 0) {
      // Sunday all day
      weekendMarketOpen = true;
    } else if (sgtDay === 1 && sgtTotalMinutes < 5 * 60 + 40) {
      // Monday before 5:40am
      weekendMarketOpen = true;
    }

    // Determine overall status
    if (weekdayMarketOpen && weekendMarketOpen) {
      marketStatus = 'Both weekday and weekend markets open';
    } else if (weekdayMarketOpen) {
      marketStatus = 'Weekday markets open';
    } else if (weekendMarketOpen) {
      marketStatus = 'Weekend markets open (Gold/Indices/Select Forex)';
    } else {
      marketStatus = 'All markets closed';
    }

    const anyMarketOpen = weekdayMarketOpen || weekendMarketOpen;

    this.addCheck({
      id: 'market_hours',
      name: 'Market Hours',
      category: 'info',
      status: anyMarketOpen ? 'pass' : 'warning',
      message: anyMarketOpen ? marketStatus : 'Markets are closed',
      details: `SGT: ${sgtTime.toISOString().slice(11, 19)} (Day ${['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'][sgtDay]})`,
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
      const engineUrl = process.env.ENGINE_INTERNAL_URL || 'http://localhost:9090';
      const response = await fetch(`${engineUrl}/api/health`, {
        method: 'GET',
        signal: AbortSignal.timeout(3000)
      });

      if (response.ok) {
        const health = await response.json();
        return {
          healthy: health.connected_to_ig === true,
          message: health.connected_to_ig ? 'Engine authenticated with IG' : 'Engine running but IG not connected'
        };
      }
      return { healthy: false, message: `Engine returned ${response.status}` };
    } catch (error) {
      return {
        healthy: false,
        message: error instanceof Error ? error.message : 'Engine not reachable'
      };
    }
  }
}
