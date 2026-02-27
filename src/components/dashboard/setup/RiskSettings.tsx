'use client';

import { Shield, CheckCircle2, RefreshCcw } from 'lucide-react';
import { Badge } from '@/components/ui/badge';

interface RiskSettingsProps {
    maxRisk: number;
    setMaxRisk: (val: number) => void;
    maxDailyLoss: number;
    setMaxDailyLoss: (val: number) => void;
    maxPositions: number;
    setMaxPositions: (val: number) => void;
    isUpdating: boolean;
    isEngineConnected: boolean;
    onUpdate: () => void;
}

export function RiskSettings({
    maxRisk, setMaxRisk,
    maxDailyLoss, setMaxDailyLoss,
    maxPositions, setMaxPositions,
    isUpdating, isEngineConnected,
    onUpdate
}: RiskSettingsProps) {
    return (
        <div className="space-y-6">
            <div>
                <div className="flex items-center justify-between mb-4">
                    <h3 className="font-medium text-gray-900 flex items-center gap-2">
                        <Shield className="h-4 w-4 text-blue-500" />
                        Engine Risk Management
                    </h3>
                    <Badge variant={isEngineConnected ? 'default' : 'secondary'}>
                        {isEngineConnected ? 'SYNCED' : 'OFFLINE'}
                    </Badge>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
                    <div className="space-y-1">
                        <label className="block text-xs font-semibold text-gray-500 uppercase tracking-wider">Max Risk/Trade (%)</label>
                        <input
                            type="number"
                            value={maxRisk}
                            onChange={(e) => setMaxRisk(Number(e.target.value))}
                            min={0.1}
                            max={5}
                            step={0.1}
                            className="w-full px-3 py-2 border rounded-lg text-sm bg-gray-50/50 focus:ring-2 focus:ring-blue-500 outline-none transition-all"
                        />
                    </div>
                    <div className="space-y-1">
                        <label className="block text-xs font-semibold text-gray-500 uppercase tracking-wider">Max Daily Loss (%)</label>
                        <input
                            type="number"
                            value={maxDailyLoss}
                            onChange={(e) => setMaxDailyLoss(Number(e.target.value))}
                            min={1}
                            max={10}
                            step={0.5}
                            className="w-full px-3 py-2 border rounded-lg text-sm bg-gray-50/50 focus:ring-2 focus:ring-blue-500 outline-none transition-all"
                        />
                    </div>
                    <div className="space-y-1">
                        <label className="block text-xs font-semibold text-gray-500 uppercase tracking-wider">Max Positions</label>
                        <input
                            type="number"
                            value={maxPositions}
                            onChange={(e) => setMaxPositions(Number(e.target.value))}
                            min={1}
                            max={10}
                            step={1}
                            className="w-full px-3 py-2 border rounded-lg text-sm bg-gray-50/50 focus:ring-2 focus:ring-blue-500 outline-none transition-all"
                        />
                    </div>
                </div>

                <button
                    onClick={onUpdate}
                    disabled={!isEngineConnected || isUpdating}
                    className={`w-full py-3 rounded-xl text-sm font-bold flex items-center justify-center gap-2 transition-all ${!isEngineConnected || isUpdating
                        ? 'bg-gray-200 text-gray-400 cursor-not-allowed'
                        : 'bg-emerald-600 text-white hover:bg-emerald-700 shadow-md hover:shadow-lg active:scale-[0.98]'
                        }`}
                >
                    {isUpdating ? (
                        <RefreshCcw className="h-4 w-4 animate-spin" />
                    ) : (
                        <CheckCircle2 className="h-4 w-4" />
                    )}
                    {isUpdating ? 'Updating Engine...' : 'APPLY CONFIG TO LIVE ENGINE'}
                </button>
                {!isEngineConnected && (
                    <p className="text-center text-[10px] text-red-500 mt-2">
                        ⚠️ Engine must be running to update live configuration.
                    </p>
                )}
            </div>
        </div>
    );
}
