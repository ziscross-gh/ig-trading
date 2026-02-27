'use client';

import { CheckCircle2, AlertTriangle, XCircle, RefreshCcw, Rocket } from 'lucide-react';

export interface PreFlightCheck {
    id: string;
    name: string;
    category: 'critical' | 'warning' | 'info';
    status: 'pass' | 'fail' | 'warning' | 'pending';
    message: string;
    details?: string;
    timestamp: string;
}

export interface PreFlightResult {
    passed: boolean;
    canGoLive: boolean;
    checks: PreFlightCheck[];
    criticalFailures: number;
    warnings: number;
    summary: string;
}

interface PreFlightChecksProps {
    result: PreFlightResult | null;
    isRunningChecks: boolean;
    onRunChecks: () => void;
}

export function PreFlightChecks({ result, isRunningChecks, onRunChecks }: PreFlightChecksProps) {
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

    if (!result && !isRunningChecks) {
        return (
            <div className="text-center py-16 text-gray-500 bg-gray-50/50 rounded-xl border border-dashed border-gray-200">
                <Rocket className="h-12 w-12 mx-auto mb-4 text-gray-300 animate-bounce" />
                <p className="text-lg font-medium">Ready for Takeoff?</p>
                <p className="text-sm">Click &quot;Run Checks&quot; to verify the Rust Engine setup</p>
                <button
                    onClick={onRunChecks}
                    className="mt-6 px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors text-sm font-medium"
                >
                    Run Performance Scan
                </button>
            </div>
        );
    }

    return (
        <div className="space-y-4">
            {isRunningChecks && (
                <div className="flex flex-col items-center justify-center py-12 bg-blue-50/30 rounded-xl border border-blue-100">
                    <RefreshCcw className="h-10 w-10 text-blue-500 animate-spin mb-4" />
                    <p className="text-blue-700 font-medium">Scanning Engine Infrastructure...</p>
                    <p className="text-blue-500 text-xs mt-1">Verifying IG API connectivity and market data flows</p>
                </div>
            )}

            {result && !isRunningChecks && (
                <>
                    {/* Summary Card */}
                    <div className={`p-4 rounded-lg border-2 flex items-start gap-4 ${result.canGoLive ? 'border-green-300 bg-green-50 shadow-sm' :
                        result.passed ? 'border-yellow-300 bg-yellow-50 shadow-sm' :
                            'border-red-300 bg-red-50 shadow-sm'
                        }`}>
                        <div className="mt-1">
                            {result.canGoLive ? (
                                <CheckCircle2 className="h-8 w-8 text-green-600" />
                            ) : result.passed ? (
                                <AlertTriangle className="h-8 w-8 text-yellow-600" />
                            ) : (
                                <XCircle className="h-8 w-8 text-red-600" />
                            )}
                        </div>
                        <div className="flex-1">
                            <p className="font-bold text-gray-900 text-lg">{result.summary}</p>
                            <div className="flex gap-6 mt-2 text-sm text-gray-600">
                                <span className="flex items-center gap-1">
                                    <XCircle className="h-4 w-4 text-red-500" />
                                    Critical Issues: <span className="font-bold text-red-700">{result.criticalFailures}</span>
                                </span>
                                <span className="flex items-center gap-1">
                                    <AlertTriangle className="h-4 w-4 text-yellow-500" />
                                    Warnings: <span className="font-bold text-yellow-700">{result.warnings}</span>
                                </span>
                            </div>
                            {result.canGoLive && (
                                <p className="mt-3 text-sm text-green-700 bg-green-100 p-2 rounded border border-green-200">
                                    ✨ High confidence. The Rust engine is warmed up and ready.
                                </p>
                            )}
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
                                        <span className={`text-xs px-2 py-0.5 rounded-full ${check.category === 'critical' ? 'bg-red-100 text-red-700' :
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
        </div>
    );
}
