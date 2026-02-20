'use client';

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Briefcase, TrendingUp, TrendingDown, X, RefreshCw } from 'lucide-react';
import type { Position } from '@/types/ig';
import { MARKET_NAMES } from '@/types/ig';

interface PositionsPanelProps {
  positions: Position[];
  loading?: boolean;
  onClosePosition?: (dealId: string, direction: 'BUY' | 'SELL', size: number) => Promise<void>;
  onRefresh?: () => void;
}

export function PositionsPanel({ positions, loading, onClosePosition, onRefresh }: PositionsPanelProps) {
  const totalPnl = positions.reduce((sum, pos) => sum + pos.pnl, 0);
  const winningPositions = positions.filter((pos) => pos.pnl > 0).length;

  return (
    <Card className="w-full">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Briefcase className="h-5 w-5 text-primary" />
            <CardTitle className="text-lg">Open Positions</CardTitle>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant={totalPnl >= 0 ? 'default' : 'destructive'} className="font-mono">{totalPnl >= 0 ? '+' : ''}{totalPnl.toFixed(2)}</Badge>
            {onRefresh && (
              <Button variant="ghost" size="icon" onClick={onRefresh} disabled={loading}>
                <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
              </Button>
            )}
          </div>
        </div>
        <CardDescription>{positions.length} position{positions.length !== 1 ? 's' : ''} • {winningPositions} winning • {positions.length - winningPositions} losing</CardDescription>
      </CardHeader>
      <CardContent>
        {positions.length === 0 ? (
          <div className="text-center text-muted-foreground py-8">No open positions</div>
        ) : (
          <div className="rounded-lg border overflow-hidden">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[100px]">Market</TableHead>
                  <TableHead className="w-[60px]">Side</TableHead>
                  <TableHead className="w-[60px]">Size</TableHead>
                  <TableHead className="w-[80px]">Entry</TableHead>
                  <TableHead className="w-[80px]">Stop</TableHead>
                  <TableHead className="w-[80px]">Limit</TableHead>
                  <TableHead className="w-[80px]">P&L</TableHead>
                  <TableHead className="w-[60px]"></TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {positions.map((position) => (
                  <TableRow key={position.dealId}>
                    <TableCell className="font-medium">
                      <div>
                        <div className="text-sm">{MARKET_NAMES[position.epic] || position.marketName || position.epic}</div>
                        <div className="text-xs text-muted-foreground">{position.dealId.slice(0, 8)}</div>
                      </div>
                    </TableCell>
                    <TableCell>
                      <Badge variant={position.direction === 'BUY' ? 'default' : 'secondary'} className={`flex items-center gap-1 w-fit ${position.direction === 'BUY' ? 'bg-green-500/10 text-green-500 hover:bg-green-500/20' : 'bg-red-500/10 text-red-500 hover:bg-red-500/20'}`}>
                        {position.direction === 'BUY' ? <TrendingUp className="h-3 w-3" /> : <TrendingDown className="h-3 w-3" />}
                        {position.direction}
                      </Badge>
                    </TableCell>
                    <TableCell className="font-mono text-sm">{position.size.toFixed(2)}</TableCell>
                    <TableCell className="font-mono text-sm">{position.level.toFixed(position.level < 10 ? 4 : 2)}</TableCell>
                    <TableCell className="font-mono text-sm text-red-500">{position.stopLevel ? position.stopLevel.toFixed(position.stopLevel < 10 ? 4 : 2) : '-'}</TableCell>
                    <TableCell className="font-mono text-sm text-green-500">{position.limitLevel ? position.limitLevel.toFixed(position.limitLevel < 10 ? 4 : 2) : '-'}</TableCell>
                    <TableCell>
                      <span className={`font-mono font-semibold ${position.pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>{position.pnl >= 0 ? '+' : ''}{position.pnl.toFixed(2)}</span>
                    </TableCell>
                    <TableCell>
                      {onClosePosition && (
                        <Button variant="ghost" size="sm" onClick={() => onClosePosition(position.dealId, position.direction, position.size)} className="h-7 w-7 p-0 text-red-500 hover:text-red-600 hover:bg-red-500/10">
                          <X className="h-4 w-4" />
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
