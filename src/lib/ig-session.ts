/**
 * IG Session Manager
 * Handles persistent sessions with auto-refresh and reconnection
 */

import { IGClient, IGSession } from './ig-client';

interface SessionState {
  client: IGClient | null;
  session: IGSession | null;
  lastActivity: Date | null;
  refreshTimer: NodeJS.Timeout | null;
  isAuthenticated: boolean;
  environment: 'demo' | 'live' | null;
}

// Global session state
const sessionState: SessionState = {
  client: null,
  session: null,
  lastActivity: null,
  refreshTimer: null,
  isAuthenticated: false,
  environment: null
};

// Session config
const SESSION_CONFIG = {
  refreshInterval: 55 * 60 * 1000, // Refresh every 55 minutes (IG sessions last 1 hour)
  inactivityTimeout: 30 * 60 * 1000, // 30 minutes inactivity timeout
  maxRetries: 3,
  retryDelay: 5000 // 5 seconds
};

/**
 * Get or create IG session
 */
export async function getIGSession(
  apiKey?: string,
  identifier?: string,
  password?: string,
  environment: 'demo' | 'live' = 'demo'
): Promise<{ success: boolean; session?: IGSession; error?: string }> {
  try {
    // Check if we have a valid session
    if (sessionState.isAuthenticated && sessionState.client && sessionState.session) {
      // Check if session needs refresh
      if (sessionState.lastActivity) {
        const timeSinceActivity = Date.now() - sessionState.lastActivity.getTime();
        if (timeSinceActivity > SESSION_CONFIG.inactivityTimeout) {
          // Session expired due to inactivity
          await clearSession();
        } else {
          // Return existing session
          sessionState.lastActivity = new Date();
          return { success: true, session: sessionState.session };
        }
      }
    }

    // Create new session
    const client = new IGClient(apiKey, identifier, password, environment);
    const session = await client.authenticate();

    // Store session
    sessionState.client = client;
    sessionState.session = session;
    sessionState.lastActivity = new Date();
    sessionState.isAuthenticated = true;
    sessionState.environment = environment;

    // Setup auto-refresh
    setupSessionRefresh();

    return { success: true, session };

  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Session creation failed'
    };
  }
}

/**
 * Setup automatic session refresh
 */
function setupSessionRefresh(): void {
  // Clear existing timer
  if (sessionState.refreshTimer) {
    clearInterval(sessionState.refreshTimer);
  }

  // Setup new refresh timer
  sessionState.refreshTimer = setInterval(async () => {
    try {
      if (sessionState.client && sessionState.isAuthenticated) {
        // Refresh session by making a lightweight API call
        await sessionState.client.getAccounts();
        sessionState.lastActivity = new Date();
        console.log('[IG Session] Session refreshed successfully');
      }
    } catch (error) {
      console.error('[IG Session] Session refresh failed:', error);
      sessionState.isAuthenticated = false;
    }
  }, SESSION_CONFIG.refreshInterval);
}

/**
 * Get the current IG client
 */
export function getIGClient(): IGClient | null {
  if (!sessionState.isAuthenticated || !sessionState.client) {
    return null;
  }
  sessionState.lastActivity = new Date();
  return sessionState.client;
}

/**
 * Check if authenticated
 */
export function isAuthenticated(): boolean {
  return sessionState.isAuthenticated;
}

/**
 * Clear the current session
 */
export async function clearSession(): Promise<void> {
  try {
    if (sessionState.client) {
      await sessionState.client.logout();
    }
  } catch (error) {
    console.error('[IG Session] Logout error:', error);
  }

  // Clear timer
  if (sessionState.refreshTimer) {
    clearInterval(sessionState.refreshTimer);
  }

  // Reset state
  sessionState.client = null;
  sessionState.session = null;
  sessionState.lastActivity = null;
  sessionState.refreshTimer = null;
  sessionState.isAuthenticated = false;
  sessionState.environment = null;
}

/**
 * Execute API call with retry logic
 */
export async function executeWithRetry<T>(
  operation: (client: IGClient) => Promise<T>,
  maxRetries: number = SESSION_CONFIG.maxRetries
): Promise<{ success: boolean; data?: T; error?: string }> {
  let lastError: Error | null = null;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      const client = getIGClient();
      if (!client) {
        throw new Error('Not authenticated');
      }

      const data = await operation(client);
      return { success: true, data };

    } catch (error) {
      lastError = error instanceof Error ? error : new Error('Unknown error');
      console.error(`[IG Session] Attempt ${attempt}/${maxRetries} failed:`, lastError.message);

      // Check if it's an authentication error
      if (lastError.message.includes('401') || lastError.message.includes('403')) {
        sessionState.isAuthenticated = false;
        return { success: false, error: 'Authentication expired. Please reconnect.' };
      }

      // Wait before retry
      if (attempt < maxRetries) {
        await new Promise(resolve => setTimeout(resolve, SESSION_CONFIG.retryDelay));
      }
    }
  }

  return {
    success: false,
    error: lastError?.message || 'Operation failed after retries'
  };
}

/**
 * Rate limiter for IG API
 */
export class IGRateLimiter {
  private requests: number[] = [];
  private maxRequests: number;
  private windowMs: number;

  constructor(maxRequests: number = 60, windowMs: number = 60000) {
    this.maxRequests = maxRequests;
    this.windowMs = windowMs;
  }

  /**
   * Check if we can make a request
   */
  canMakeRequest(): boolean {
    const now = Date.now();
    // Remove old requests outside the window
    this.requests = this.requests.filter(time => now - time < this.windowMs);
    
    return this.requests.length < this.maxRequests;
  }

  /**
   * Record a request
   */
  recordRequest(): void {
    this.requests.push(Date.now());
  }

  /**
   * Wait until we can make a request
   */
  async waitForSlot(): Promise<void> {
    while (!this.canMakeRequest()) {
      const oldestRequest = this.requests[0];
      const waitTime = this.windowMs - (Date.now() - oldestRequest);
      await new Promise(resolve => setTimeout(resolve, Math.min(waitTime, 1000)));
    }
    this.recordRequest();
  }

  /**
   * Get remaining requests
   */
  getRemainingRequests(): number {
    const now = Date.now();
    this.requests = this.requests.filter(time => now - time < this.windowMs);
    return Math.max(0, this.maxRequests - this.requests.length);
  }
}

// Global rate limiter instance
export const igRateLimiter = new IGRateLimiter();
