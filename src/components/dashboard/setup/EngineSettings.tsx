'use client';

import { Lock } from 'lucide-react';
import type { EngineState, EngineConfig } from '@/hooks/useEngine';

interface EngineSettingsProps {
    health: EngineState['health'];
    config: EngineConfig | null;
    isEngineConnected: boolean;
}

export function EngineSettings({ health, config, isEngineConnected }: EngineSettingsProps) {
    return (
        <div className="space-y-6">
            {/* IG Connection Status (read-only — engine owns credentials) */}
            <div>
                <h3 className="font-medium text-gray-900 mb-3 flex items-center gap-2">
                    <Lock className="h-4 w-4 text-gray-500" />
                    IG API Connection
                </h3>
                <div className={`p-4 rounded-lg border ${health?.connected_to_ig
                    ? 'border-green-200 bg-green-50'
                    : isEngineConnected
                        ? 'border-yellow-200 bg-yellow-50'
                        : 'border-red-200 bg-red-50'
                    }`}>
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <span className={`h-2.5 w-2.5 rounded-full ${health?.connected_to_ig
                                ? 'bg-green-500'
                                : isEngineConnected
                                    ? 'bg-yellow-500'
                                    : 'bg-red-500'
                                }`} />
                            <span className="font-medium text-gray-900">
                                {health?.connected_to_ig
                                    ? 'Authenticated with IG'
                                    : isEngineConnected
                                        ? 'Engine running, IG not connected'
                                        : 'Engine offline'}
                            </span>
                        </div>
                        <span className="text-xs text-gray-500">
                            {health?.version ? `v${health.version}` : ''}
                        </span>
                    </div>
                    <p className="text-xs text-gray-500 mt-2">
                        Mode: <span className="font-mono">{config?.mode || '—'}</span>
                        {health?.uptime_secs != null && (
                            <> | Uptime: {Math.floor(health.uptime_secs / 60)}m {health.uptime_secs % 60}s</>
                        )}
                    </p>
                </div>
                <p className="text-xs text-gray-500 mt-3">
                    Credentials are managed by the Rust engine via environment variables.
                    Set <code className="bg-gray-200 px-1 rounded">IG_API_KEY</code>,{' '}
                    <code className="bg-gray-200 px-1 rounded">IG_IDENTIFIER</code>, and{' '}
                    <code className="bg-gray-200 px-1 rounded">IG_PASSWORD</code> in the engine&apos;s{' '}
                    <code className="bg-gray-200 px-1 rounded">.env</code> file, then restart the engine.
                </p>
            </div>

            {/* Engine Configuration Guide */}
            <div className="p-4 bg-gray-50 rounded-lg">
                <h3 className="font-medium text-gray-900 mb-2">📋 Engine Environment Setup</h3>
                <p className="text-sm text-gray-600 mb-2">
                    Set these in the Rust engine&apos;s <code className="bg-gray-200 px-1 rounded">.env</code> file (in <code className="bg-gray-200 px-1 rounded">ig-engine/</code>):
                </p>
                <pre className="text-xs bg-gray-800 text-green-400 p-3 rounded overflow-x-auto">
                    {`# IG API credentials (engine-only)
IG_API_KEY=your_api_key
IG_IDENTIFIER=your_username
IG_PASSWORD=your_password
IG_ENVIRONMENT=demo

# Risk settings are configured via
# config/default.toml or this dashboard`}
                </pre>
                <p className="text-xs text-gray-500 mt-2">
                    Risk settings above can be updated live via the engine. Credentials require an engine restart.
                </p>
            </div>
        </div>
    );
}
