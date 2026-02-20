'use client';

import { useState, useEffect } from 'react';

interface PreFlightCheck {
  id: string;
  name: string;
  category: 'critical' | 'warning' | 'info';
  status: 'pass' | 'fail' | 'warning' | 'pending';
  message: string;
  details?: string;
  timestamp: string;
}

interface PreFlightResult {
  passed: boolean;
  canGoLive: boolean;
  checks: PreFlightCheck[];
  criticalFailures: number;
  warnings: number;
  summary: string;
}

interface PaperAccount {
  balance: number;
  available: number;
  margin: number;
  equity: number;
  openPositions: any[];
  tradeHistory: any[];
  dailyPnl: number;
  weeklyPnl: number;
  totalTrades: number;
  winRate: number;
}

export function SetupPanel() {
  const [environment, setEnvironment] = useState<'demo' | 'live'>('demo');
  const [isRunning, setIsRunning] = useState(false);
  const [result, setResult] = useState<PreFlightResult | null>(null);
  const [paperAccount, setPaperAccount] = useState<PaperAccount | null>(null);
  const [activeTab, setActiveTab] = useState<'checks' | 'paper' | 'settings'>('checks');

  // API credentials for testing
  const [apiKey, setApiKey] = useState('');
  const [identifier, setIdentifier] = useState('');
  const [password, setPassword] = useState('');

  // Risk settings
  const [maxRisk, setMaxRisk] = useState(1);
  const [maxDailyLoss, setMaxDailyLoss] = useState(5);
  const [maxPositions, setMaxPositions] = useState(3);

  const runChecks = async () => {
    setIsRunning(true);
    try {
      const params = new URLSearchParams({
        environment,
        ...(apiKey && { apiKey }),
        ...(identifier && { identifier }),
        ...(password && { password })
      });

      const response = await fetch(`/api/preflight?${params}`);
      const data = await response.json();
      setResult(data);
    } catch (error) {
      console.error('Failed to run checks:', error);
    } finally {
      setIsRunning(false);
    }
  };

  const fetchPaperAccount = async () => {
    try {
      const response = await fetch('/api/paper-trading');
      const data = await response.json();
      if (data.success) {
        setPaperAccount(data.account);
      }
    } catch (error) {
      console.error('Failed to fetch paper account:', error);
    }
  };

  const resetPaperAccount = async () => {
    try {
      const response = await fetch('/api/paper-trading', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'reset',
          params: { initialBalance: 10000 }
        })
      });
      const data = await response.json();
      if (data.success) {
        setPaperAccount(data.account);
      }
    } catch (error) {
      console.error('Failed to reset paper account:', error);
    }
  };

  useEffect(() => {
    if (activeTab === 'paper') {
      fetchPaperAccount();
    }
  }, [activeTab]);

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'pass': return '✅';
      case 'fail': return '❌';
      case 'warning': return '⚠️';
      default: return '⏳';
    }
  };

  const getCategoryColor = (category: string) => {
    switch (category) {
      case 'critical': return 'border-red-200 bg-red-50';
      case 'warning': return 'border-yellow-200 bg-yellow-50';
      default: return 'border-blue-200 bg-blue-50';
    }
  };

  return (
    <div className="bg-white rounded-xl shadow-lg p-6">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-xl font-bold text-gray-900">🚀 Pre-Launch Setup</h2>
          <p className="text-gray-500 text-sm mt-1">Run checks before going live</p>
        </div>
        <div className="flex items-center gap-3">
          <select
            value={environment}
            onChange={(e) => setEnvironment(e.target.value as 'demo' | 'live')}
            className="px-3 py-2 border rounded-lg text-sm"
          >
            <option value="demo">Demo Account</option>
            <option value="live">Live Account</option>
          </select>
          <button
            onClick={runChecks}
            disabled={isRunning}
            className={`px-4 py-2 rounded-lg text-sm font-medium text-white ${
              isRunning ? 'bg-gray-400' : 'bg-blue-600 hover:bg-blue-700'
            }`}
          >
            {isRunning ? 'Running...' : 'Run Checks'}
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-6 bg-gray-100 p-1 rounded-lg">
        {[
          { id: 'checks', label: 'Pre-Flight Checks', icon: '🔍' },
          { id: 'paper', label: 'Paper Trading', icon: '📝' },
          { id: 'settings', label: 'Settings', icon: '⚙️' }
        ].map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id as any)}
            className={`flex-1 px-4 py-2 rounded-md text-sm font-medium transition-all ${
              activeTab === tab.id
                ? 'bg-white shadow text-gray-900'
                : 'text-gray-600 hover:text-gray-900'
            }`}
          >
            {tab.icon} {tab.label}
          </button>
        ))}
      </div>

      {/* Pre-Flight Checks Tab */}
      {activeTab === 'checks' && (
        <div className="space-y-4">
          {result && (
            <>
              {/* Summary Card */}
              <div className={`p-4 rounded-lg border-2 ${
                result.canGoLive ? 'border-green-300 bg-green-50' : 
                result.passed ? 'border-yellow-300 bg-yellow-50' : 
                'border-red-300 bg-red-50'
              }`}>
                <div className="flex items-center gap-3">
                  <span className="text-3xl">
                    {result.canGoLive ? '✅' : result.passed ? '⚠️' : '❌'}
                  </span>
                  <div>
                    <p className="font-medium text-gray-900">{result.summary}</p>
                    <div className="flex gap-4 mt-1 text-sm text-gray-600">
                      <span>Critical Issues: {result.criticalFailures}</span>
                      <span>Warnings: {result.warnings}</span>
                    </div>
                  </div>
                </div>
              </div>

              {/* Check Results */}
              <div className="space-y-2">
                {result.checks.map((check) => (
                  <div
                    key={check.id}
                    className={`p-3 rounded-lg border ${getCategoryColor(check.category)}`}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <span>{getStatusIcon(check.status)}</span>
                        <span className="font-medium text-gray-900">{check.name}</span>
                        <span className={`text-xs px-2 py-0.5 rounded-full ${
                          check.category === 'critical' ? 'bg-red-100 text-red-700' :
                          check.category === 'warning' ? 'bg-yellow-100 text-yellow-700' :
                          'bg-blue-100 text-blue-700'
                        }`}>
                          {check.category}
                        </span>
                      </div>
                    </div>
                    <p className="text-sm text-gray-600 mt-1">{check.message}</p>
                    {check.details && (
                      <p className="text-xs text-gray-500 mt-1 font-mono">{check.details}</p>
                    )}
                  </div>
                ))}
              </div>
            </>
          )}

          {!result && (
            <div className="text-center py-12 text-gray-500">
              <p className="text-4xl mb-3">🔍</p>
              <p>Click &quot;Run Checks&quot; to verify your setup</p>
            </div>
          )}
        </div>
      )}

      {/* Paper Trading Tab */}
      {activeTab === 'paper' && (
        <div className="space-y-4">
          {paperAccount && (
            <>
              {/* Account Summary */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <div className="p-4 bg-gray-50 rounded-lg">
                  <p className="text-xs text-gray-500 uppercase">Balance</p>
                  <p className="text-xl font-bold text-gray-900">
                    ${paperAccount.balance.toFixed(2)}
                  </p>
                </div>
                <div className="p-4 bg-gray-50 rounded-lg">
                  <p className="text-xs text-gray-500 uppercase">Equity</p>
                  <p className="text-xl font-bold text-gray-900">
                    ${paperAccount.equity.toFixed(2)}
                  </p>
                </div>
                <div className="p-4 bg-gray-50 rounded-lg">
                  <p className="text-xs text-gray-500 uppercase">Daily P&L</p>
                  <p className={`text-xl font-bold ${paperAccount.dailyPnl >= 0 ? 'text-green-600' : 'text-red-600'}`}>
                    {paperAccount.dailyPnl >= 0 ? '+' : ''}${paperAccount.dailyPnl.toFixed(2)}
                  </p>
                </div>
                <div className="p-4 bg-gray-50 rounded-lg">
                  <p className="text-xs text-gray-500 uppercase">Win Rate</p>
                  <p className="text-xl font-bold text-gray-900">
                    {paperAccount.winRate.toFixed(1)}%
                  </p>
                </div>
              </div>

              {/* Open Positions */}
              {paperAccount.openPositions.length > 0 && (
                <div>
                  <h3 className="font-medium text-gray-900 mb-2">Open Positions</h3>
                  <div className="space-y-2">
                    {paperAccount.openPositions.map((pos: any) => (
                      <div key={pos.id} className="p-3 bg-gray-50 rounded-lg flex justify-between items-center">
                        <div>
                          <span className={`px-2 py-1 rounded text-xs font-medium ${
                            pos.direction === 'BUY' ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'
                          }`}>
                            {pos.direction}
                          </span>
                          <span className="ml-2 font-medium">{pos.marketName}</span>
                        </div>
                        <div className="text-right">
                          <p className={`font-medium ${pos.pnl >= 0 ? 'text-green-600' : 'text-red-600'}`}>
                            {pos.pnl >= 0 ? '+' : ''}${pos.pnl.toFixed(2)}
                          </p>
                          <p className="text-xs text-gray-500">{pos.size} @ {pos.openPrice}</p>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Actions */}
              <div className="flex gap-2">
                <button
                  onClick={fetchPaperAccount}
                  className="px-4 py-2 bg-blue-600 text-white rounded-lg text-sm hover:bg-blue-700"
                >
                  Refresh
                </button>
                <button
                  onClick={resetPaperAccount}
                  className="px-4 py-2 bg-gray-200 text-gray-700 rounded-lg text-sm hover:bg-gray-300"
                >
                  Reset Account ($10,000)
                </button>
              </div>
            </>
          )}

          {!paperAccount && (
            <div className="text-center py-8">
              <button
                onClick={resetPaperAccount}
                className="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
              >
                Initialize Paper Trading Account
              </button>
            </div>
          )}
        </div>
      )}

      {/* Settings Tab */}
      {activeTab === 'settings' && (
        <div className="space-y-6">
          {/* API Credentials */}
          <div>
            <h3 className="font-medium text-gray-900 mb-3">API Credentials</h3>
            <div className="space-y-3">
              <div>
                <label className="block text-sm text-gray-600 mb-1">API Key</label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="Enter your IG API key"
                  className="w-full px-3 py-2 border rounded-lg text-sm"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-600 mb-1">Identifier</label>
                <input
                  type="text"
                  value={identifier}
                  onChange={(e) => setIdentifier(e.target.value)}
                  placeholder="Enter your IG identifier"
                  className="w-full px-3 py-2 border rounded-lg text-sm"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-600 mb-1">Password</label>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="Enter your IG password"
                  className="w-full px-3 py-2 border rounded-lg text-sm"
                />
              </div>
            </div>
            <p className="text-xs text-gray-500 mt-2">
              ⚠️ Credentials are only used for testing and not stored.
            </p>
          </div>

          {/* Risk Settings */}
          <div>
            <h3 className="font-medium text-gray-900 mb-3">Risk Management</h3>
            <div className="grid grid-cols-3 gap-4">
              <div>
                <label className="block text-sm text-gray-600 mb-1">Max Risk/Trade (%)</label>
                <input
                  type="number"
                  value={maxRisk}
                  onChange={(e) => setMaxRisk(Number(e.target.value))}
                  min={0.5}
                  max={10}
                  step={0.5}
                  className="w-full px-3 py-2 border rounded-lg text-sm"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-600 mb-1">Max Daily Loss (%)</label>
                <input
                  type="number"
                  value={maxDailyLoss}
                  onChange={(e) => setMaxDailyLoss(Number(e.target.value))}
                  min={1}
                  max={20}
                  step={1}
                  className="w-full px-3 py-2 border rounded-lg text-sm"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-600 mb-1">Max Positions</label>
                <input
                  type="number"
                  value={maxPositions}
                  onChange={(e) => setMaxPositions(Number(e.target.value))}
                  min={1}
                  max={10}
                  step={1}
                  className="w-full px-3 py-2 border rounded-lg text-sm"
                />
              </div>
            </div>
          </div>

          {/* Environment Variables Guide */}
          <div className="p-4 bg-gray-50 rounded-lg">
            <h3 className="font-medium text-gray-900 mb-2">📋 Environment Variables</h3>
            <p className="text-sm text-gray-600 mb-2">
              Create a <code className="bg-gray-200 px-1 rounded">.env.local</code> file in the project root:
            </p>
            <pre className="text-xs bg-gray-800 text-green-400 p-3 rounded overflow-x-auto">
{`IG_API_KEY=your_api_key
IG_IDENTIFIER=your_username
IG_PASSWORD=your_password
IG_ENVIRONMENT=demo
MAX_RISK_PER_TRADE=${maxRisk}
MAX_DAILY_LOSS=${maxDailyLoss}
MAX_POSITIONS=${maxPositions}`}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}
