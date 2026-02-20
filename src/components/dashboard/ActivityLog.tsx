'use client';

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { List, Info, AlertTriangle, CheckCircle, XCircle, ArrowUpCircle, ArrowDownCircle, Trash2 } from 'lucide-react';
import type { ActivityLog } from '@/types/ig';

interface ActivityLogPanelProps {
  logs: ActivityLog[];
  onClear?: () => void;
  maxHeight?: string;
}

function LogIcon({ type }: { type: ActivityLog['type'] }) {
  switch (type) {
    case 'INFO': return <Info className="h-4 w-4 text-blue-500" />;
    case 'TRADE': return <ArrowUpCircle className="h-4 w-4 text-green-500" />;
    case 'SUCCESS': return <CheckCircle className="h-4 w-4 text-green-500" />;
    case 'WARNING': return <AlertTriangle className="h-4 w-4 text-yellow-500" />;
    case 'ERROR': return <XCircle className="h-4 w-4 text-red-500" />;
    default: return <Info className="h-4 w-4 text-muted-foreground" />;
  }
}

function LogTypeBadge({ type }: { type: ActivityLog['type'] }) {
  const variants: Record<ActivityLog['type'], { bg: string; text: string }> = {
    INFO: { bg: 'bg-blue-500/10', text: 'text-blue-500' },
    TRADE: { bg: 'bg-green-500/10', text: 'text-green-500' },
    SUCCESS: { bg: 'bg-green-500/10', text: 'text-green-500' },
    WARNING: { bg: 'bg-yellow-500/10', text: 'text-yellow-500' },
    ERROR: { bg: 'bg-red-500/10', text: 'text-red-500' },
  };
  const { bg, text } = variants[type];
  return (
    <Badge variant="outline" className={`${bg} ${text} text-xs font-normal`}>{type}</Badge>
  );
}

function formatTime(date: Date): string {
  return new Date(date).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false });
}

export function ActivityLogPanel({ logs, onClear, maxHeight = '300px' }: ActivityLogPanelProps) {
  return (
    <Card className="w-full">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <List className="h-5 w-5 text-primary" />
            <CardTitle className="text-lg">Activity Log</CardTitle>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant="secondary">{logs.length} entries</Badge>
            {onClear && logs.length > 0 && (
              <Button variant="ghost" size="sm" onClick={onClear} className="h-7 text-muted-foreground hover:text-foreground">
                <Trash2 className="h-3 w-3 mr-1" />Clear
              </Button>
            )}
          </div>
        </div>
        <CardDescription>Real-time bot activity and trade notifications</CardDescription>
      </CardHeader>
      <CardContent>
        {logs.length === 0 ? (
          <div className="text-center text-muted-foreground py-8">No activity yet</div>
        ) : (
          <ScrollArea style={{ height: maxHeight }}>
            <div className="space-y-2 pr-4">
              {logs.map((log) => (
                <div key={log.id} className="flex items-start gap-3 p-3 rounded-lg bg-muted/30 hover:bg-muted/50 transition-colors">
                  <LogIcon type={log.type} />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <LogTypeBadge type={log.type} />
                      <span className="text-xs text-muted-foreground">{formatTime(log.timestamp)}</span>
                    </div>
                    <p className="text-sm">{log.message}</p>
                    {log.details && Object.keys(log.details).length > 0 && (
                      <div className="mt-2 text-xs text-muted-foreground font-mono bg-muted/50 p-2 rounded">
                        {Object.entries(log.details).map(([key, value]) => (
                          <span key={key} className="mr-3">{key}: {String(value)}</span>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </ScrollArea>
        )}
      </CardContent>
    </Card>
  );
}
