/**
 * Notification Service
 * Send alerts via Telegram, Slack, Email
 */

// Types
export interface NotificationMessage {
  title: string;
  message: string;
  type: 'info' | 'warning' | 'error' | 'success' | 'trade';
  data?: Record<string, unknown>;
  timestamp: Date;
}

export interface NotificationConfig {
  telegram: {
    enabled: boolean;
    botToken: string;
    chatId: string;
  };
  slack: {
    enabled: boolean;
    webhookUrl: string;
  };
  email: {
    enabled: boolean;
    host: string;
    port: number;
    user: string;
    password: string;
    from: string;
    to: string;
  };
}

// Default config from environment
function getDefaultConfig(): NotificationConfig {
  return {
    telegram: {
      enabled: !!process.env.TELEGRAM_BOT_TOKEN && !!process.env.TELEGRAM_CHAT_ID,
      botToken: process.env.TELEGRAM_BOT_TOKEN || '',
      chatId: process.env.TELEGRAM_CHAT_ID || ''
    },
    slack: {
      enabled: !!process.env.SLACK_WEBHOOK_URL,
      webhookUrl: process.env.SLACK_WEBHOOK_URL || ''
    },
    email: {
      enabled: !!(process.env.SMTP_HOST && process.env.SMTP_USER),
      host: process.env.SMTP_HOST || '',
      port: parseInt(process.env.SMTP_PORT || '587'),
      user: process.env.SMTP_USER || '',
      password: process.env.SMTP_PASSWORD || '',
      from: process.env.SMTP_FROM || 'trading-bot@example.com',
      to: process.env.SMTP_TO || ''
    }
  };
}

/**
 * Notification Service Class
 */
export class NotificationService {
  private config: NotificationConfig;
  private queue: NotificationMessage[] = [];
  private isProcessing: boolean = false;

  constructor(config?: Partial<NotificationConfig>) {
    this.config = { ...getDefaultConfig(), ...config };
  }

  /**
   * Send a notification
   */
  async send(message: Omit<NotificationMessage, 'timestamp'>): Promise<boolean> {
    const fullMessage: NotificationMessage = {
      ...message,
      timestamp: new Date()
    };

    // Add to queue
    this.queue.push(fullMessage);
    
    // Process queue
    this.processQueue();

    return true;
  }

  /**
   * Send trade notification
   */
  async sendTradeAlert(trade: {
    action: 'OPEN' | 'CLOSE' | 'MODIFY';
    epic: string;
    direction: 'BUY' | 'SELL';
    size: number;
    price: number;
    pnl?: number;
    reason?: string;
  }): Promise<boolean> {
    const pnlText = trade.pnl !== undefined 
      ? `\n💰 P&L: ${trade.pnl >= 0 ? '+' : ''}$${trade.pnl.toFixed(2)}`
      : '';
    
    return this.send({
      title: `📊 Trade ${trade.action}`,
      message: `${trade.direction} ${trade.size} ${trade.epic}\n@ $${trade.price.toFixed(2)}${pnlText}${trade.reason ? `\n📝 ${trade.reason}` : ''}`,
      type: 'trade',
      data: trade
    });
  }

  /**
   * Send bot status notification
   */
  async sendBotStatus(status: {
    action: 'STARTED' | 'STOPPED' | 'ERROR' | 'WARNING';
    message: string;
    stats?: Record<string, unknown>;
  }): Promise<boolean> {
    const emoji = {
      STARTED: '🚀',
      STOPPED: '🛑',
      ERROR: '❌',
      WARNING: '⚠️'
    }[status.action];

    return this.send({
      title: `${emoji} Bot ${status.action}`,
      message: status.message,
      type: status.action === 'ERROR' ? 'error' : status.action === 'WARNING' ? 'warning' : 'info',
      data: status.stats
    });
  }

  /**
   * Send risk alert
   */
  async sendRiskAlert(alert: {
    type: 'DAILY_LOSS_LIMIT' | 'MAX_POSITIONS' | 'DRAWDOWN' | 'MARGIN';
    message: string;
    current: number;
    limit: number;
  }): Promise<boolean> {
    return this.send({
      title: `⚠️ Risk Alert: ${alert.type}`,
      message: `${alert.message}\nCurrent: ${alert.current}\nLimit: ${alert.limit}`,
      type: 'warning',
      data: alert
    });
  }

  /**
   * Process notification queue
   */
  private async processQueue(): Promise<void> {
    if (this.isProcessing || this.queue.length === 0) return;

    this.isProcessing = true;

    while (this.queue.length > 0) {
      const message = this.queue.shift();
      if (!message) continue;

      try {
        await this.sendToAll(message);
      } catch (error) {
        console.error('[Notifications] Failed to send:', error);
      }

      // Small delay between messages
      await new Promise(resolve => setTimeout(resolve, 100));
    }

    this.isProcessing = false;
  }

  /**
   * Send to all enabled channels
   */
  private async sendToAll(message: NotificationMessage): Promise<void> {
    const promises: Promise<void>[] = [];

    if (this.config.telegram.enabled) {
      promises.push(this.sendToTelegram(message));
    }

    if (this.config.slack.enabled) {
      promises.push(this.sendToSlack(message));
    }

    if (this.config.email.enabled) {
      promises.push(this.sendToEmail(message));
    }

    await Promise.allSettled(promises);
  }

  /**
   * Send to Telegram
   */
  private async sendToTelegram(message: NotificationMessage): Promise<void> {
    if (!this.config.telegram.botToken || !this.config.telegram.chatId) return;

    const emoji = {
      info: 'ℹ️',
      warning: '⚠️',
      error: '❌',
      success: '✅',
      trade: '📊'
    }[message.type];

    const text = `${emoji} *${message.title}*\n\n${message.message}\n\n_${message.timestamp.toISOString()}_`;

    try {
      const response = await fetch(
        `https://api.telegram.org/bot${this.config.telegram.botToken}/sendMessage`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            chat_id: this.config.telegram.chatId,
            text,
            parse_mode: 'Markdown'
          })
        }
      );

      if (!response.ok) {
        throw new Error(`Telegram API error: ${response.status}`);
      }

      console.log('[Notifications] Telegram message sent');
    } catch (error) {
      console.error('[Notifications] Telegram error:', error);
      throw error;
    }
  }

  /**
   * Send to Slack
   */
  private async sendToSlack(message: NotificationMessage): Promise<void> {
    if (!this.config.slack.webhookUrl) return;

    const color = {
      info: '#36a64f',
      warning: '#ff9900',
      error: '#ff0000',
      success: '#00ff00',
      trade: '#0099ff'
    }[message.type];

    const payload = {
      attachments: [{
        color,
        title: message.title,
        text: message.message,
        footer: message.timestamp.toISOString(),
        fields: message.data ? Object.entries(message.data).map(([key, value]) => ({
          title: key,
          value: String(value),
          short: true
        })) : []
      }]
    };

    try {
      const response = await fetch(this.config.slack.webhookUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });

      if (!response.ok) {
        throw new Error(`Slack API error: ${response.status}`);
      }

      console.log('[Notifications] Slack message sent');
    } catch (error) {
      console.error('[Notifications] Slack error:', error);
      throw error;
    }
  }

  /**
   * Send to Email
   */
  private async sendToEmail(message: NotificationMessage): Promise<void> {
    if (!this.config.email.host || !this.config.email.user) return;

    // For simplicity, we'll just log the email
    // In production, use a proper email library like nodemailer
    console.log('[Notifications] Email would be sent:', {
      to: this.config.email.to,
      subject: `[IG Trading Bot] ${message.title}`,
      body: message.message
    });

    // Note: To actually send emails, you would use nodemailer:
    // import nodemailer from 'nodemailer';
    // const transporter = nodemailer.createTransport({...});
    // await transporter.sendMail({...});
  }
}

// Singleton instance
let notificationService: NotificationService | null = null;

export function getNotificationService(): NotificationService {
  if (!notificationService) {
    notificationService = new NotificationService();
  }
  return notificationService;
}

export function initNotificationService(config?: Partial<NotificationConfig>): NotificationService {
  notificationService = new NotificationService(config);
  return notificationService;
}
