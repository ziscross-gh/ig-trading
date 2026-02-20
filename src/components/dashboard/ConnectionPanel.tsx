'use client';

import { useState } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Wifi, WifiOff, Loader2, Shield, Eye, EyeOff, Server, CheckCircle2, XCircle } from 'lucide-react';
import type { IGCredentials } from '@/types/ig';

interface ConnectionPanelProps {
  authenticated: boolean;
  loading: boolean;
  error: string | null;
  isDemo: boolean;
  onConnect: (credentials: IGCredentials) => Promise<{ success: boolean; error?: string }>;
  onDisconnect: () => Promise<{ success: boolean }>;
  onClearError: () => void;
}

export function ConnectionPanel({
  authenticated,
  loading,
  error,
  isDemo,
  onConnect,
  onDisconnect,
  onClearError,
}: ConnectionPanelProps) {
  const [apiKey, setApiKey] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [demoMode, setDemoMode] = useState(true);
  const [showPassword, setShowPassword] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    onClearError();
    await onConnect({ apiKey, username, password, isDemo: demoMode });
  };

  const handleDisconnect = async () => {
    await onDisconnect();
    setPassword('');
  };

  return (
    <Card className="w-full">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            {authenticated ? (
              <Wifi className="h-5 w-5 text-green-500" />
            ) : (
              <WifiOff className="h-5 w-5 text-muted-foreground" />
            )}
            <CardTitle className="text-lg">IG Connection</CardTitle>
          </div>
          <Badge variant={authenticated ? 'default' : 'secondary'} className="flex items-center gap-1">
            {authenticated ? (
              <><CheckCircle2 className="h-3 w-3" />Connected</>
            ) : (
              <><XCircle className="h-3 w-3" />Disconnected</>
            )}
          </Badge>
        </div>
        <CardDescription>
          {authenticated ? `Connected to ${isDemo ? 'Demo' : 'Live'} account` : 'Enter your IG API credentials'}
        </CardDescription>
      </CardHeader>
      <CardContent>
        {authenticated ? (
          <div className="space-y-4">
            <div className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
              <div className="flex items-center gap-2">
                <Server className="h-4 w-4 text-muted-foreground" />
                <span className="text-sm">{demoMode ? 'Demo Server' : 'Live Server'}</span>
              </div>
              <Badge variant={demoMode ? 'secondary' : 'destructive'}>{demoMode ? 'Demo' : 'LIVE'}</Badge>
            </div>
            <Button onClick={handleDisconnect} variant="outline" className="w-full" disabled={loading}>
              {loading ? <Loader2 className="h-4 w-4 mr-2 animate-spin" /> : <WifiOff className="h-4 w-4 mr-2" />}
              Disconnect
            </Button>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="apiKey">API Key</Label>
              <Input id="apiKey" type="password" placeholder="Enter your API key" value={apiKey} onChange={(e) => setApiKey(e.target.value)} required disabled={loading} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="username">Username</Label>
              <Input id="username" type="text" placeholder="Enter your username" value={username} onChange={(e) => setUsername(e.target.value)} required disabled={loading} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="password">Password</Label>
              <div className="relative">
                <Input id="password" type={showPassword ? 'text' : 'password'} placeholder="Enter your password" value={password} onChange={(e) => setPassword(e.target.value)} required disabled={loading} className="pr-10" />
                <Button type="button" variant="ghost" size="sm" className="absolute right-0 top-0 h-full px-3" onClick={() => setShowPassword(!showPassword)}>
                  {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </Button>
              </div>
            </div>
            <div className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
              <div className="flex items-center gap-2">
                <Shield className="h-4 w-4 text-muted-foreground" />
                <Label htmlFor="demoMode" className="text-sm cursor-pointer">Demo Account</Label>
              </div>
              <Switch id="demoMode" checked={demoMode} onCheckedChange={setDemoMode} disabled={loading} />
            </div>
            {!demoMode && (
              <Alert variant="destructive">
                <AlertDescription className="text-xs">Warning: You are connecting to a LIVE account. Real money is at risk.</AlertDescription>
              </Alert>
            )}
            {error && (
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}
            <Button type="submit" className="w-full" disabled={loading}>
              {loading ? <><Loader2 className="h-4 w-4 mr-2 animate-spin" />Connecting...</> : <><Wifi className="h-4 w-4 mr-2" />Connect</>}
            </Button>
          </form>
        )}
      </CardContent>
    </Card>
  );
}
