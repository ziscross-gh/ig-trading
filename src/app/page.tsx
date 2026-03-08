'use client';

import { useState } from 'react';
import { Card } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Separator } from '@/components/ui/separator';
import { AlertCircle, Cpu, Rocket } from 'lucide-react';

// Components
import { PriceChart } from '@/components/dashboard/PriceChart';
import { MarketOverview } from '@/components/dashboard/MarketOverview';
import { SetupPanel } from '@/components/dashboard/setup-panel';
import { EngineControlPanel, EnginePositionsPanel, EngineSignalsPanel } from '@/components/dashboard/EnginePanel';
import { StrategyLab } from '@/components/dashboard/StrategyLab';
import { LearningPanel } from '@/components/dashboard/LearningPanel';
import { TradeHistory } from '@/components/dashboard/TradeHistory';
import { EquityCurvePanel } from '@/components/dashboard/EquityCurvePanel';

// Layout Components
import { DashboardHeader } from '@/components/dashboard/layout/DashboardHeader';
import { MobileNav } from '@/components/dashboard/layout/MobileNav';

// Hooks from Context
import { useEngine, useMarketData } from '@/context/EngineContext';

export default function Dashboard() {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [activeTab, setActiveTab] = useState('engine');

  const marketData = useMarketData();
  const engine = useEngine();

  return (
    <div className="min-h-screen bg-background">
      <DashboardHeader
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        engine={engine}
        mobileMenuOpen={mobileMenuOpen}
        setMobileMenuOpen={setMobileMenuOpen}
      />

      <main className="container px-4 py-6">
        <MobileNav activeTab={activeTab} setActiveTab={setActiveTab} />

        {/* ENGINE TAB */}
        {activeTab === 'engine' && (
          <>
            {engine.error && (
              <Alert variant="destructive" className="mb-4">
                <AlertCircle className="h-4 w-4" />
                <AlertDescription>{engine.error}</AlertDescription>
              </Alert>
            )}

            <div className="grid gap-6 lg:grid-cols-12 mb-6">
              <div className="lg:col-span-8">
                <PriceChart
                  candles={marketData.historicalData}
                  epic={marketData.selectedEpic}
                  marketName={marketData.marketName}
                  loading={marketData.loading}
                  onRefresh={() => marketData.fetchHistory(marketData.selectedEpic)}
                  livePrice={(() => {
                    const m = marketData.markets.find(mk => mk.epic === marketData.selectedEpic);
                    return m ? { bid: m.bid, offer: m.offer } : undefined;
                  })()}
                />
              </div>
              <div className="lg:col-span-4">
                <MarketOverview
                  markets={marketData.markets}
                  selectedEpic={marketData.selectedEpic}
                  onSelectMarket={marketData.selectMarket}
                  loading={marketData.loading}
                />
              </div>
            </div>

            <div className="grid gap-6 lg:grid-cols-12">
              <div className="space-y-6 lg:col-span-3">
                <EngineControlPanel
                  status={engine.status}
                  connected={engine.connected}
                  loading={engine.loading}
                  onStart={engine.startEngine}
                  onStop={engine.stopEngine}
                  onPause={engine.pauseEngine}
                />
              </div>

              <div className="space-y-6 lg:col-span-6">
                <EnginePositionsPanel
                  positions={engine.positions}
                  loading={engine.loading}
                  onRefresh={engine.fetchPositions}
                />
                <EngineSignalsPanel
                  signals={engine.signals}
                  onRefresh={engine.fetchSignals}
                />
              </div>

              <div className="space-y-6 lg:col-span-3">
                <Card className="p-4">
                  <h3 className="font-semibold mb-3 flex items-center gap-2 text-sm">
                    <Cpu className="h-4 w-4" />
                    Engine Config
                  </h3>
                  {engine.config ? (
                    <div className="space-y-3 text-sm">
                      <div>
                        <div className="text-muted-foreground text-xs">Mode</div>
                        <Badge variant="outline" className="capitalize">{engine.config.mode}</Badge>
                      </div>
                      <div>
                        <div className="text-muted-foreground text-xs">Risk / Trade</div>
                        <div className="font-medium">{engine.config.max_risk_per_trade}%</div>
                      </div>
                      <div>
                        <div className="text-muted-foreground text-xs">Daily Loss Limit</div>
                        <div className="font-medium">{engine.config.max_daily_loss_pct}%</div>
                      </div>
                      <div>
                        <div className="text-muted-foreground text-xs">Max Open Positions</div>
                        <div className="font-medium">{engine.config.max_open_positions}</div>
                      </div>
                      <div>
                        <div className="text-muted-foreground text-xs">Markets</div>
                        <div className="flex flex-wrap gap-1 mt-1">
                          {engine.config.markets.map(m => (
                            <Badge key={m} variant="secondary" className="text-[10px]">{m.split('.')[2]}</Badge>
                          ))}
                        </div>
                      </div>
                      <Separator />
                      <div>
                        <div className="text-muted-foreground text-xs mb-1">Strategies</div>
                        <div className="space-y-1">
                          {[
                            { name: 'MA Crossover', enabled: engine.config.strategies.ma_crossover },
                            { name: 'RSI Divergence', enabled: engine.config.strategies.rsi_divergence },
                            { name: 'MACD Momentum', enabled: engine.config.strategies.macd_momentum },
                            { name: 'Bollinger Reversion', enabled: engine.config.strategies.bollinger_reversion },
                          ].map(s => (
                            <div key={s.name} className="flex items-center justify-between text-xs py-0.5">
                              <span>{s.name}</span>
                              <Badge variant={s.enabled ? 'default' : 'secondary'} className="text-[10px]">
                                {s.enabled ? 'ON' : 'OFF'}
                              </Badge>
                            </div>
                          ))}
                        </div>
                      </div>
                    </div>
                  ) : (
                    <p className="text-sm text-muted-foreground">Engine not connected</p>
                  )}
                </Card>
              </div>
            </div>

            <Separator className="my-6" />

            <TradeHistory
              mockMode={false}
              signals={[]}
              equityCurve={engine.stats?.all_time.equity_curve}
            />

            <EquityCurvePanel trades={engine.trades} initialBalance={10000} />

            <LearningPanel
              learning={engine.learning}
              onRefresh={engine.fetchLearning}
            />
          </>
        )}

        {activeTab === 'strategy-lab' && <StrategyLab />}

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
                  {['Create IG Demo Account', 'Get API Credentials', 'Run Pre-Flight Checks', 'Start Engine in Paper Mode', 'Optimize in Strategy Lab', 'Demo Trade 2-4 Weeks', 'Go Live — Small Size'].map((item, i) => (
                    <div key={item} className="flex items-center gap-2 text-muted-foreground">
                      <span>{i + 1}️⃣</span>
                      <span>{item}</span>
                    </div>
                  ))}
                </div>
              </Card>

              <Alert className="border-yellow-500/50 bg-yellow-50">
                <AlertCircle className="h-4 w-4 text-yellow-600" />
                <AlertDescription className="text-xs text-yellow-700">
                  <strong>Important:</strong> Always start with a demo account.
                  Test thoroughly before using real money.
                </AlertDescription>
              </Alert>
            </div>
          </div>
        )}
      </main>

      <footer className="border-t py-4 mt-8">
        <div className="container px-4 text-center text-sm text-muted-foreground">
          <p>IG Trading Bot — Gold & FX • Rust Engine on port 9090</p>
          <p className="mt-1">
            Last update: {engine.lastUpdate?.toLocaleString('en-US', { timeZone: 'Asia/Singapore', hour12: false }) || 'N/A'}
          </p>
        </div>
      </footer>
    </div>
  );
}
