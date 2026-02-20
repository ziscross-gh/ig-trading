'use client';

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Brain, Settings2, ChevronDown, ChevronUp } from 'lucide-react';
import { useState } from 'react';
import type { StrategyConfig } from '@/types/ig';
import { STRATEGY_METADATA } from '@/lib/trading-strategies';

interface StrategyConfigPanelProps {
  strategies: StrategyConfig[];
  onToggle: (name: string, enabled: boolean) => Promise<{ success: boolean }>;
  onUpdateParams?: (name: string, params: Record<string, number>) => Promise<{ success: boolean }>;
  disabled?: boolean;
}

function StrategyCard({ strategy, onToggle, onUpdateParams, disabled }: { strategy: StrategyConfig; onToggle: (enabled: boolean) => void; onUpdateParams?: (params: Record<string, number>) => void; disabled?: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const [params, setParams] = useState(strategy.parameters);
  const metadata = STRATEGY_METADATA[strategy.name as keyof typeof STRATEGY_METADATA];
  const categoryColor = metadata?.category === 'Trend Following' ? 'text-blue-500' : 'text-purple-500';

  return (
    <div className={`border rounded-lg transition-all ${strategy.enabled ? 'border-primary/50 bg-primary/5' : 'border-border'}`}>
      <div className="p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Switch checked={strategy.enabled} onCheckedChange={onToggle} disabled={disabled} />
            <div>
              <div className="font-medium text-sm">{metadata?.displayName || strategy.name}</div>
              <div className="text-xs text-muted-foreground">{metadata?.description}</div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant="outline" className={`text-xs ${categoryColor}`}>{metadata?.category || 'Strategy'}</Badge>
            <Button variant="ghost" size="sm" onClick={() => setExpanded(!expanded)} disabled={disabled} className="h-6 w-6 p-0">
              {expanded ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
            </Button>
          </div>
        </div>
      </div>
      {expanded && metadata?.parameters && (
        <div className="px-4 pb-4 pt-0 border-t">
          <div className="grid grid-cols-2 gap-3 mt-3">
            {metadata.parameters.map((param) => (
              <div key={param.name} className="space-y-1">
                <Label htmlFor={param.name} className="text-xs">{param.label}</Label>
                <Input id={param.name} type="number" value={params[param.name] as number} onChange={(e) => { const newParams = { ...params, [param.name]: parseFloat(e.target.value) || param.default }; setParams(newParams); onUpdateParams?.(newParams); }} disabled={disabled || !strategy.enabled} className="h-8" />
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export function StrategyConfigPanel({ strategies, onToggle, onUpdateParams, disabled }: StrategyConfigPanelProps) {
  const [loading, setLoading] = useState<string | null>(null);

  const handleToggle = async (name: string, enabled: boolean) => {
    setLoading(name);
    await onToggle(name, enabled);
    setLoading(null);
  };

  const handleUpdateParams = async (name: string, params: Record<string, number>) => {
    if (onUpdateParams) {
      await onUpdateParams(name, params);
    }
  };

  const enabledCount = strategies.filter((s) => s.enabled).length;

  return (
    <Card className="w-full">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Brain className="h-5 w-5 text-primary" />
            <CardTitle className="text-lg">Trading Strategies</CardTitle>
          </div>
          <Badge variant="secondary">{enabledCount}/{strategies.length} Active</Badge>
        </div>
        <CardDescription>Configure technical analysis strategies for signal generation</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        {strategies.length === 0 ? (
          <div className="text-center text-muted-foreground py-8">No strategies configured</div>
        ) : (
          strategies.map((strategy) => (
            <StrategyCard key={strategy.name} strategy={strategy} onToggle={(enabled) => handleToggle(strategy.name, enabled)} onUpdateParams={(params) => handleUpdateParams(strategy.name, params)} disabled={disabled || loading === strategy.name} />
          ))
        )}
        <div className="pt-4 mt-4 border-t">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Brain className="h-4 w-4" />
            <span>Signals are generated when multiple strategies agree</span>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
