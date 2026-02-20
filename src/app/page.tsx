'use client';

import { useState, useEffect } from 'react';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { Card } from '@/components/ui/card';
import { 
  Bot, 
  BarChart3, 
  Activity,
  Wifi,
  WifiOff,
  AlertCircle,
  Menu,
  X,
  FlaskConical,
  Calendar,
  Zap,
  Rocket,
  Download,
  Target,
  Brain
} from 'lucide-react';

// Components
import { ConnectionPanel } from '@/components/dashboard/ConnectionPanel';
import { MarketOverview } from '@/components/dashboard/MarketOverview';
import { PriceChart } from '@/components/dashboard/PriceChart';
import { BotControlPanel } from '@/components/dashboard/BotControlPanel';
import { StrategyConfigPanel } from '@/components/dashboard/StrategyConfig';
import { PositionsPanel } from '@/components/dashboard/PositionsPanel';
import { TradeHistory } from '@/components/dashboard/TradeHistory';
import { ActivityLogPanel } from '@/components/dashboard/ActivityLog';
import { BacktestingPanel } from '@/components/dashboard/BacktestingPanel';
import { EconomicCalendarPanel } from '@/components/dashboard/EconomicCalendarPanel';
import { TrendFilterPanel } from '@/components/dashboard/TrendFilterPanel';
import { SetupPanel } from '@/components/dashboard/setup-panel';
import { MarketScannerPanel } from '@/components/dashboard/MarketScannerPanel';
import { AIInsightsPanel } from '@/components/dashboard/AIInsightsPanel';

// Hooks
import { useIGConnection } from '@/hooks/useIGConnection';
import { useMarketData } from '@/hooks/useMarketData';
import { useBot } from '@/hooks/useBot';

// Types
import { DEFAULT_STRATEGIES } from '@/types/ig';

export default function Dashboard() {
  // State
  const [mockMode, setMockMode] = useState(true);
  const [strategies, setStrategies] = useState(DEFAULT_STRATEGIES);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [activeTab, setActiveTab] = useState('trading');

  // Hooks
  const igConnection = useIGConnection();
  const marketData = useMarketData(mockMode);
  const bot = useBot();

  // Auto-refresh
  useEffect(() => {
    marketData.startAutoRefresh(10000);
    bot.startAutoRefresh(5000);
    
    return () => {
      marketData.stopAutoRefresh();
      bot.stopAutoRefresh();
    };
  }, []);

  // Handle connection
  const handleConnect = async (credentials: Parameters<typeof igConnection.connect>[0]) => {
    const result = await igConnection.connect(credentials);
    if (result.success) {
      setMockMode(false);
    }
    return result;
  };

  const handleDisconnect = async () => {
    const result = await igConnection.disconnect();
    if (result.success) {
      setMockMode(true);
    }
    return result;
  };

  // Handle bot controls
  const handleStartBot = async () => {
    return bot.start();
  };

  const handleStopBot = async () => {
    return bot.stop();
  };

  // Handle strategy toggle
  const handleToggleStrategy = async (name: string, enabled: boolean) => {
    const result = await bot.toggleStrategy(name, enabled);
    if (result.success) {
      setStrategies(prev => 
        prev.map(s => s.name === name ? { ...s, enabled } : s)
      );
    }
    return result;
  };

  // Handle close position
  const handleClosePosition = async (dealId: string, direction: 'BUY' | 'SELL', size: number) => {
    try {
      const response = await fetch('/api/ig/trade', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'close',
          dealId,
          direction,
          size,
          mockMode,
        }),
      });
      
      const data = await response.json();
      if (data.success) {
        marketData.fetchPositions();
      }
    } catch (error) {
      console.error('Failed to close position:', error);
    }
  };

  return (
    <div className="min-h-screen bg-background">
      {/* Header */}
      <header className="sticky top-0 z-50 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="container flex h-14 items-center px-4">
          <div className="flex items-center gap-2 mr-4">
            <Bot className="h-6 w-6 text-primary" />
            <span className="font-bold text-lg hidden sm:inline">IG Trading Bot</span>
          </div>
          
          {/* Main Navigation Tabs */}
          <Tabs value={activeTab} onValueChange={setActiveTab} className="hidden md:block">
            <TabsList>
              <TabsTrigger value="trading" className="flex items-center gap-1">
                <BarChart3 className="h-4 w-4" />
                Trading
              </TabsTrigger>
              <TabsTrigger value="trends" className="flex items-center gap-1">
                <Zap className="h-4 w-4" />
                Trends
              </TabsTrigger>
              <TabsTrigger value="backtest" className="flex items-center gap-1">
                <FlaskConical className="h-4 w-4" />
                Backtest
              </TabsTrigger>
              <TabsTrigger value="calendar" className="flex items-center gap-1">
                <Calendar className="h-4 w-4" />
                Calendar
              </TabsTrigger>
              <TabsTrigger value="scanner" className="flex items-center gap-1">
                <Target className="h-4 w-4" />
                Scanner
              </TabsTrigger>
              <TabsTrigger value="setup" className="flex items-center gap-1">
                <Rocket className="h-4 w-4" />
                Setup
              </TabsTrigger>
              <TabsTrigger value="ai" className="flex items-center gap-1">
                <Brain className="h-4 w-4" />
                AI Insights
              </TabsTrigger>
            </TabsList>
          </Tabs>
          
          <div className="flex items-center gap-2 ml-auto">
            {mockMode && (
              <Badge variant="outline" className="bg-yellow-500/10 text-yellow-600 border-yellow-500/20">
                Demo Mode
              </Badge>
            )}
            
            <Badge 
              variant={igConnection.authenticated ? 'default' : 'secondary'}
              className="flex items-center gap-1"
            >
              {igConnection.authenticated ? (
                <>
                  <Wifi className="h-3 w-3" />
                  Connected
                </>
              ) : (
                <>
                  <WifiOff className="h-3 w-3" />
                  Disconnected
                </>
              )}
            </Badge>
            
            <Badge 
              variant={bot.isRunning ? 'default' : 'secondary'}
              className={`flex items-center gap-1 ${bot.isRunning ? 'bg-green-500 hover:bg-green-600' : ''}`}
            >
              <Activity className={`h-3 w-3 ${bot.isRunning ? 'animate-pulse' : ''}`} />
              {bot.status}
            </Badge>
          </div>
          
          {/* Download Button */}
          <a
            href="/api/download"
            className="ml-3 flex items-center gap-2 px-4 py-2 bg-gradient-to-r from-blue-600 to-purple-600 text-white rounded-lg font-medium text-sm hover:from-blue-700 hover:to-purple-700 transition-all shadow-md hover:shadow-lg"
          >
            <Download className="h-4 w-4" />
            <span className="hidden sm:inline">Download Project</span>
          </a>
          
          <Button
            variant="ghost"
            size="sm"
            className="ml-2 md:hidden"
            onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
          >
            {mobileMenuOpen ? <X className="h-5 w-5" /> : <Menu className="h-5 w-5" />}
          </Button>
        </div>
      </header>

      {/* Main Content */}
      <main className="container px-4 py-6">
        {/* Mobile Navigation */}
        {mobileMenuOpen && (
          <div className="md:hidden mb-4">
            <Tabs value={activeTab} onValueChange={setActiveTab} className="w-full">
              <TabsList className="grid grid-cols-6 w-full">
                <TabsTrigger value="trading">
                  <BarChart3 className="h-4 w-4" />
                </TabsTrigger>
                <TabsTrigger value="trends">
                  <Zap className="h-4 w-4" />
                </TabsTrigger>
                <TabsTrigger value="scanner">
                  <Target className="h-4 w-4" />
                </TabsTrigger>
                <TabsTrigger value="backtest">
                  <FlaskConical className="h-4 w-4" />
                </TabsTrigger>
                <TabsTrigger value="calendar">
                  <Calendar className="h-4 w-4" />
                </TabsTrigger>
                <TabsTrigger value="setup">
                  <Rocket className="h-4 w-4" />
                </TabsTrigger>
                <TabsTrigger value="ai">
                  <Brain className="h-4 w-4" />
                </TabsTrigger>
              </TabsList>
            </Tabs>
          </div>
        )}

        {/* Alerts */}
        {igConnection.error && (
          <Alert variant="destructive" className="mb-4">
            <AlertCircle className="h-4 w-4" />
            <AlertDescription>{igConnection.error}</AlertDescription>
          </Alert>
        )}

        {bot.error && (
          <Alert variant="destructive" className="mb-4">
            <AlertCircle className="h-4 w-4" />
            <AlertDescription>{bot.error}</AlertDescription>
          </Alert>
        )}

        {/* TRADING TAB */}
        {activeTab === 'trading' && (
          <>
            <div className="grid gap-6 lg:grid-cols-12">
              {/* Left Column - Connection & Bot Control */}
              <div className="space-y-6 lg:col-span-3">
                <ConnectionPanel
                  authenticated={igConnection.authenticated}
                  loading={igConnection.loading}
                  error={igConnection.error}
                  isDemo={igConnection.isDemo}
                  onConnect={handleConnect}
                  onDisconnect={handleDisconnect}
                  onClearError={igConnection.clearError}
                />
                
                <BotControlPanel
                  status={bot.status}
                  config={bot.config}
                  stats={bot.stats}
                  loading={bot.loading}
                  onStart={handleStartBot}
                  onStop={handleStopBot}
                  onUpdateRiskConfig={bot.updateRiskConfig}
                />
              </div>

              {/* Center Column - Charts & Markets */}
              <div className="space-y-6 lg:col-span-6">
                <PriceChart
                  candles={marketData.historicalData}
                  epic={marketData.selectedEpic}
                  loading={marketData.loading}
                  onRefresh={() => marketData.fetchHistory(marketData.selectedEpic)}
                />
                
                <MarketOverview
                  markets={marketData.markets}
                  selectedEpic={marketData.selectedEpic}
                  onSelectMarket={marketData.selectMarket}
                  loading={marketData.loading}
                />
                
                <PositionsPanel
                  positions={marketData.positions}
                  loading={marketData.loading}
                  onClosePosition={handleClosePosition}
                  onRefresh={marketData.fetchPositions}
                />
              </div>

              {/* Right Column - Strategies & Logs */}
              <div className="space-y-6 lg:col-span-3">
                <StrategyConfigPanel
                  strategies={strategies}
                  onToggle={handleToggleStrategy}
                  onUpdateParams={async (name, params) => {
                    return bot.updateStrategies(
                      strategies.map(s => s.name === name ? { ...s, parameters: params } : s)
                    );
                  }}
                  disabled={bot.isRunning}
                />
                
                <ActivityLogPanel
                  logs={bot.logs}
                  onClear={bot.clearLogs}
                  maxHeight="250px"
                />
              </div>
            </div>

            <Separator className="my-6" />
            
            <TradeHistory mockMode={mockMode} signals={bot.recentSignals} />

            {marketData.account && (
              <div className="mt-6 p-4 border rounded-lg bg-muted/30">
                <div className="flex items-center justify-between mb-3">
                  <span className="font-medium">Account Summary</span>
                  <Badge variant="outline">{marketData.account.accountType}</Badge>
                </div>
                <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
                  <div>
                    <div className="text-muted-foreground">Balance</div>
                    <div className="font-semibold">${marketData.account.balance.toFixed(2)}</div>
                  </div>
                  <div>
                    <div className="text-muted-foreground">Available</div>
                    <div className="font-semibold">${marketData.account.available.toFixed(2)}</div>
                  </div>
                  <div>
                    <div className="text-muted-foreground">P&L</div>
                    <div className={`font-semibold ${marketData.account.profitLoss >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                      {marketData.account.profitLoss >= 0 ? '+' : ''}${marketData.account.profitLoss.toFixed(2)}
                    </div>
                  </div>
                  <div>
                    <div className="text-muted-foreground">Currency</div>
                    <div className="font-semibold">{marketData.account.currency}</div>
                  </div>
                </div>
              </div>
            )}
          </>
        )}

        {/* TRENDS TAB */}
        {activeTab === 'trends' && (
          <TrendFilterPanel />
        )}

        {/* SCANNER TAB */}
        {activeTab === 'scanner' && (
          <div className="grid gap-6 lg:grid-cols-4">
            <div className="lg:col-span-3">
              <MarketScannerPanel />
            </div>
            <div className="space-y-6">
              <Card className="p-4">
                <h3 className="font-semibold mb-3 flex items-center gap-2">
                  <Target className="h-4 w-4" />
                  Scanner Settings
                </h3>
                <div className="space-y-2 text-sm text-muted-foreground">
                  <p>• Scans all enabled markets automatically</p>
                  <p>• Analyzes RSI, MACD, MA, Bollinger Bands</p>
                  <p>• Calculates entry, stop loss, take profit</p>
                  <p>• Shows confidence score for each signal</p>
                  <p>• Hotness indicates market activity level</p>
                </div>
              </Card>
              
              <Card className="p-4">
                <h3 className="font-semibold mb-3">Signal Legend</h3>
                <div className="space-y-2 text-xs">
                  <div className="flex items-center justify-between">
                    <span>🟢 STRONG BUY</span>
                    <span>Score 80+</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>🟢 BUY</span>
                    <span>Score 60-79</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>⚪ HOLD</span>
                    <span>Score 40-59</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>🔴 SELL</span>
                    <span>Score 20-39</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>🔴 STRONG SELL</span>
                    <span>Score 0-19</span>
                  </div>
                </div>
              </Card>

              <Card className="p-4 bg-blue-50 border-blue-200">
                <h3 className="font-semibold mb-2 text-blue-700">💡 Pro Tips</h3>
                <div className="text-xs text-blue-600 space-y-1">
                  <p>• Look for signals with 70%+ confidence</p>
                  <p>• Check multiple timeframes</p>
                  <p>• Verify with trend direction</p>
                  <p>• Use proper risk management</p>
                </div>
              </Card>
            </div>
          </div>
        )}

        {/* BACKTEST TAB */}
        {activeTab === 'backtest' && (
          <div className="grid gap-6 lg:grid-cols-4">
            <div className="lg:col-span-3">
              <BacktestingPanel />
            </div>
            <div className="space-y-6">
              <Card className="p-4">
                <h3 className="font-semibold mb-3 flex items-center gap-2">
                  <FlaskConical className="h-4 w-4" />
                  Backtest Tips
                </h3>
                <div className="space-y-2 text-sm text-muted-foreground">
                  <p>• Use at least 500 data points for meaningful results</p>
                  <p>• Test with different risk settings (presets available)</p>
                  <p>• Compare multiple strategies before live trading</p>
                  <p>• Past performance doesn&apos;t guarantee future results</p>
                  <p>• Consider slippage and spread costs</p>
                </div>
              </Card>
              
              <Card className="p-4">
                <h3 className="font-semibold mb-3">Available Strategies</h3>
                <div className="space-y-1 text-xs">
                  {['MA Crossover', 'RSI', 'MACD', 'Bollinger Bands', 'Stochastic', 'EMA Scalping', 'S/R Breakout', 'Triple EMA', 'ADX Trend', 'Williams %R'].map(s => (
                    <div key={s} className="flex items-center gap-2">
                      <div className="h-1.5 w-1.5 rounded-full bg-primary" />
                      {s}
                    </div>
                  ))}
                </div>
              </Card>
            </div>
          </div>
        )}

        {/* CALENDAR TAB */}
        {activeTab === 'calendar' && (
          <div className="grid gap-6 lg:grid-cols-3">
            <div className="lg:col-span-2">
              <EconomicCalendarPanel />
            </div>
            <div className="space-y-6">
              <Card className="p-4">
                <h3 className="font-semibold mb-3 flex items-center gap-2">
                  <Calendar className="h-4 w-4" />
                  Trading Tips
                </h3>
                <div className="space-y-2 text-sm text-muted-foreground">
                  <p>• Avoid trading during HIGH impact events</p>
                  <p>• Wait 15-30 min after major news</p>
                  <p>• Watch for NFP, FOMC, CPI releases</p>
                  <p>• Spreads widen during news events</p>
                  <p>• Consider closing positions before major news</p>
                </div>
              </Card>
              
              <Card className="p-4">
                <h3 className="font-semibold mb-3">Key Events to Watch</h3>
                <div className="space-y-2 text-xs">
                  <div className="flex items-center justify-between">
                    <span>🇺🇸 Non-Farm Payrolls</span>
                    <Badge variant="destructive" className="text-xs">HIGH</Badge>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>🇺🇸 Fed Interest Rate</span>
                    <Badge variant="destructive" className="text-xs">HIGH</Badge>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>🇺🇸 CPI / Core CPI</span>
                    <Badge variant="destructive" className="text-xs">HIGH</Badge>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>🇪🇺 ECB Rate Decision</span>
                    <Badge variant="destructive" className="text-xs">HIGH</Badge>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>🇬🇧 BOE Rate Decision</span>
                    <Badge variant="destructive" className="text-xs">HIGH</Badge>
                  </div>
                </div>
              </Card>
              
              <Alert>
                <AlertCircle className="h-4 w-4" />
                <AlertDescription className="text-xs">
                  The bot automatically avoids trading during high-impact news events when enabled.
                </AlertDescription>
              </Alert>
            </div>
          </div>
        )}

        {/* AI INSIGHTS TAB */}
        {activeTab === 'ai' && (
          <div className="grid gap-6 lg:grid-cols-3">
            <div className="lg:col-span-2">
              <Card className="p-4">
                <AIInsightsPanel 
                  selectedSymbol={marketData.selectedEpic?.includes('GOLD') ? 'GOLD' : marketData.selectedEpic}
                />
              </Card>
            </div>
            <div className="space-y-6">
              <Card className="p-4">
                <h3 className="font-semibold mb-3 flex items-center gap-2">
                  <Brain className="h-4 w-4 text-purple-500" />
                  AI-Powered Analysis
                </h3>
                <div className="space-y-2 text-sm text-muted-foreground">
                  <p>• Trade suggestions with entry/exit points</p>
                  <p>• Sentiment analysis from technical indicators</p>
                  <p>• Confidence scoring for each signal</p>
                  <p>• Risk/reward ratio calculations</p>
                  <p>• Market phase detection</p>
                </div>
              </Card>
              
              <Card className="p-4">
                <h3 className="font-semibold mb-3">How It Works</h3>
                <div className="space-y-2 text-xs">
                  <div className="flex items-start gap-2">
                    <span className="font-bold text-purple-500">1.</span>
                    <span>AI analyzes RSI, MACD, MAs, Bollinger Bands, and more</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <span className="font-bold text-purple-500">2.</span>
                    <span>Generates BUY/SELL/HOLD signals with confidence scores</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <span className="font-bold text-purple-500">3.</span>
                    <span>Calculates optimal entry, stop loss, and take profit levels</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <span className="font-bold text-purple-500">4.</span>
                    <span>Provides reasoning and risk factors for each trade</span>
                  </div>
                </div>
              </Card>

              <Card className="p-4 bg-purple-50 border-purple-200">
                <h3 className="font-semibold mb-2 text-purple-700">🧠 Pro Tips</h3>
                <div className="text-xs text-purple-600 space-y-1">
                  <p>• Only trade signals with 60%+ confidence</p>
                  <p>• Always use the suggested stop loss</p>
                  <p>• Combine with your own analysis</p>
                  <p>• Consider market conditions and news</p>
                </div>
              </Card>

              <Alert className="border-purple-500/50 bg-purple-50">
                <AlertCircle className="h-4 w-4 text-purple-600" />
                <AlertDescription className="text-xs text-purple-700">
                  AI suggestions are for informational purposes only. Always do your own research before trading.
                </AlertDescription>
              </Alert>
            </div>
          </div>
        )}

        {/* SETUP TAB */}
        {activeTab === 'setup' && (
          <div className="grid gap-6 lg:grid-cols-3">
            <div className="lg:col-span-2">
              <SetupPanel />
            </div>
            <div className="space-y-6">
              <Card className="p-4">
                <h3 className="font-semibold mb-3 flex items-center gap-2">
                  <Rocket className="h-4 w-4" />
                  Go-Live Checklist
                </h3>
                <div className="space-y-2 text-sm">
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <span>1️⃣</span>
                    <span>Create IG Demo Account</span>
                  </div>
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <span>2️⃣</span>
                    <span>Get API Credentials</span>
                  </div>
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <span>3️⃣</span>
                    <span>Run Pre-Flight Checks</span>
                  </div>
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <span>4️⃣</span>
                    <span>Test with Paper Trading</span>
                  </div>
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <span>5️⃣</span>
                    <span>Backtest Strategies</span>
                  </div>
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <span>6️⃣</span>
                    <span>Demo Trade 2-4 Weeks</span>
                  </div>
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <span>7️⃣</span>
                    <span>Start Small on Live</span>
                  </div>
                </div>
              </Card>

              <Alert className="border-yellow-500/50 bg-yellow-50">
                <AlertCircle className="h-4 w-4 text-yellow-600" />
                <AlertDescription className="text-xs text-yellow-700">
                  <strong>⚠️ Important:</strong> Always start with a demo account. 
                  Test thoroughly before using real money.
                </AlertDescription>
              </Alert>

              <Card className="p-4 bg-red-50 border-red-200">
                <h3 className="font-semibold mb-2 text-red-700">🚨 Risk Warning</h3>
                <div className="text-xs text-red-600 space-y-1">
                  <p>• Trading involves substantial risk of loss</p>
                  <p>• Never trade with money you cannot afford to lose</p>
                  <p>• Past performance does not guarantee future results</p>
                  <p>• Always use proper risk management</p>
                </div>
              </Card>
            </div>
          </div>
        )}
      </main>

      {/* Footer */}
      <footer className="border-t py-4 mt-8">
        <div className="container px-4 text-center text-sm text-muted-foreground">
          <p>IG Trading Bot for Gold & FX • Use at your own risk</p>
          <p className="mt-1">
            Last update: {marketData.lastUpdate?.toLocaleString() || 'N/A'}
          </p>
        </div>
      </footer>
    </div>
  );
}
