'use client';

import { useState, useEffect } from 'react';
import { useEngine } from '@/hooks/useEngine';
import { Activity, Settings, RefreshCcw } from 'lucide-react';
import { PreFlightChecks, PreFlightResult } from './setup/PreFlightChecks';
import { RiskSettings } from './setup/RiskSettings';
import { EngineSettings } from './setup/EngineSettings';

export function SetupPanel() {
  const engine = useEngine();
  const [environment, setEnvironment] = useState<'demo' | 'live'>('demo');
  const [isRunningChecks, setIsRunningChecks] = useState(false);
  const [result, setResult] = useState<PreFlightResult | null>(null);
  const [activeTab, setActiveTab] = useState<'checks' | 'settings'>('checks');

  // Risk settings (synced with engine)
  const [maxRisk, setMaxRisk] = useState(1);
  const [maxDailyLoss, setMaxDailyLoss] = useState(5);
  const [maxPositions, setMaxPositions] = useState(3);
  const [isUpdatingEngine, setIsUpdatingEngine] = useState(false);

  // Sync with engine config when it loads
  useEffect(() => {
    if (engine.config) {
      setMaxRisk(engine.config.max_risk_per_trade);
      setMaxDailyLoss(engine.config.max_daily_loss_pct);
      setMaxPositions(engine.config.max_open_positions);
      setEnvironment(engine.config.mode === 'live' ? 'live' : 'demo');
    }
  }, [engine.config]);

  const runChecks = async () => {
    setIsRunningChecks(true);
    try {
      const response = await fetch(`/api/preflight`);
      const data = await response.json();
      setResult(data);
    } catch (error) {
      console.error('Failed to run checks:', error);
    } finally {
      setIsRunningChecks(false);
    }
  };

  const updateEngineConfig = async () => {
    setIsUpdatingEngine(true);
    try {
      const resp = await engine.updateRisk({
        max_risk_per_trade: maxRisk,
        max_daily_loss_pct: maxDailyLoss,
        max_open_positions: maxPositions,
      });
      if (resp.success) {
        // Success
      }
    } catch (error) {
      console.error('Failed to update engine config:', error);
    } finally {
      setIsUpdatingEngine(false);
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
            className="px-3 py-2 border rounded-lg text-sm bg-white"
          >
            <option value="demo">Demo Account</option>
            <option value="live">Live Account</option>
          </select>
          <button
            onClick={runChecks}
            disabled={isRunningChecks}
            className={`px-4 py-2 rounded-lg text-sm font-medium text-white transition-colors ${isRunningChecks ? 'bg-gray-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700'
              }`}
          >
            {isRunningChecks ? (
              <span className="flex items-center gap-2">
                <RefreshCcw className="h-4 w-4 animate-spin" />
                Running...
              </span>
            ) : 'Run Checks'}
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-6 bg-gray-100 p-1 rounded-lg">
        {[
          { id: 'checks', label: 'Pre-Flight Checks', icon: <Activity className="h-4 w-4" /> },
          { id: 'settings', label: 'Engine Settings', icon: <Settings className="h-4 w-4" /> }
        ].map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id as 'checks' | 'settings')}
            className={`flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === tab.id
              ? 'bg-white shadow text-gray-900'
              : 'text-gray-600 hover:text-gray-900'
              }`}
          >
            {tab.icon} {tab.label}
          </button>
        ))}
      </div>

      {activeTab === 'checks' && (
        <PreFlightChecks
          result={result}
          isRunningChecks={isRunningChecks}
          onRunChecks={runChecks}
        />
      )}

      {activeTab === 'settings' && (
        <div className="space-y-8">
          <RiskSettings
            maxRisk={maxRisk}
            setMaxRisk={setMaxRisk}
            maxDailyLoss={maxDailyLoss}
            setMaxDailyLoss={setMaxDailyLoss}
            maxPositions={maxPositions}
            setMaxPositions={setMaxPositions}
            isUpdating={isUpdatingEngine}
            isEngineConnected={engine.connected}
            onUpdate={updateEngineConfig}
          />
          <div className="h-px bg-gray-100" />
          <EngineSettings
            health={engine.health}
            config={engine.config}
            isEngineConnected={engine.connected}
          />
        </div>
      )}
    </div>
  );
}
