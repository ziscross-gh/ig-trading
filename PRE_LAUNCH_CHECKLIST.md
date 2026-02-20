# 🚀 IG Trading Bot - Pre-Launch Checklist

## ⚠️ CRITICAL - Complete BEFORE Live Trading

### 1. API Setup & Credentials

- [ ] **Create IG Demo Account First** (HIGHLY RECOMMENDED)
  - Sign up at: https://www.ig.com/demo
  - Test ALL strategies with virtual money ($10,000 - $50,000 demo balance)
  - Run for at least 2-4 weeks in demo mode
  - Verify all trade executions work correctly

- [ ] **Get API Credentials**
  - Log in to IG web platform → Settings → API Access
  - Create new API key with appropriate permissions
  - Note your: API Key, Identifier, Password
  - For live: Also get your Account ID

- [ ] **Store Credentials Securely**
  ```bash
  # Create .env.local file (NEVER commit this!)
  IG_API_KEY=your_api_key_here
  IG_IDENTIFIER=your_username_here
  IG_PASSWORD=your_password_here
  IG_ACCOUNT_ID=your_account_id  # Optional for live
  IG_ENVIRONMENT=demo  # Start with 'demo', change to 'live' later
  ```

### 2. Risk Management Setup

- [ ] **Set Maximum Risk Per Trade** (Recommended: 1-2% of account)
- [ ] **Set Daily Loss Limit** (Recommended: 5% of account)
- [ ] **Set Maximum Open Positions** (Recommended: 3-5)
- [ ] **Configure Stop Loss** (MUST HAVE - never trade without)
- [ ] **Test with minimal position sizes first**

### 3. Paper Trading Mode

- [ ] **Enable Paper Trading Mode in Bot**
  - Set `PAPER_TRADING=true` in environment
  - All trades will be simulated without real execution

- [ ] **Run in Paper Mode for 1-2 weeks minimum**
  - Monitor strategy performance
  - Check risk management triggers
  - Validate notification system

### 4. Strategy Validation

- [ ] **Run Backtests for Each Strategy**
  - Test on historical data (6-12 months)
  - Check win rate > 50% for trend strategies
  - Verify max drawdown is acceptable (< 20%)
  - Review equity curve for consistency

- [ ] **Forward Test in Demo Account**
  - Run live demo for at least 2 weeks
  - Compare results with backtest expectations
  - Adjust parameters if needed

### 5. Technical Infrastructure

- [ ] **Hosting Options:**
  - **Local**: Run on your computer (must stay on 24/7)
  - **VPS**: Cloud server (recommended for 24/7 operation)
  - Options: AWS, Google Cloud, DigitalOcean, Vultr

- [ ] **Database Setup** (for trade history)
  - SQLite (simple, local)
  - PostgreSQL (production recommended)
  - Configure Prisma with your database

- [ ] **Monitoring & Alerts**
  - Set up Telegram bot for notifications
  - Configure Slack webhooks (optional)
  - Email alerts for critical events

### 6. Legal & Compliance

- [ ] **Understand IG's API Terms of Service**
- [ ] **Check your country's trading regulations**
- [ ] **Keep records of all trades for tax purposes**
- [ ] **Never trade money you can't afford to lose**

### 7. Final Checks Before Live

- [ ] Run API connection test
- [ ] Verify demo account has positive results
- [ ] Start with SMALL position sizes
- [ ] Monitor first few live trades closely
- [ ] Have an emergency stop plan

---

## 📊 Recommended Starting Configuration

```json
{
  "risk": {
    "maxRiskPerTrade": 1,      // 1% per trade
    "maxDailyLoss": 5,          // 5% daily limit
    "maxPositions": 3,          // Max 3 open positions
    "defaultStopLoss": 2,       // 2% default stop loss
    "defaultTakeProfit": 4      // 4% default take profit
  },
  "strategies": {
    "enabled": ["ma_crossover", "rsi_strategy"],
    "timeframes": ["1h", "4h"],
    "markets": ["GOLD", "EURUSD"]
  },
  "notifications": {
    "telegram": true,
    "email": true,
    "slack": false
  }
}
```

---

## 🔧 Quick Start Commands

```bash
# 1. Install dependencies
bun install

# 2. Set up environment variables
cp .env.example .env.local
# Edit .env.local with your credentials

# 3. Initialize database
bunx prisma generate
bunx prisma db push

# 4. Run in development
bun run dev

# 5. Run lint check
bun run lint
```

---

## ⚡ Emergency Procedures

**If something goes wrong:**
1. Immediately disable the bot via dashboard
2. Close all open positions manually in IG platform
3. Check IG API status page
4. Review logs for errors
5. Contact IG support if needed

**Stop Loss Failed:**
1. Log into IG web/mobile app immediately
2. Close position manually
3. Set tighter stop losses in future

---

## 📞 Support Resources

- IG Help: https://www.ig.com/help
- IG API Docs: https://labs.ig.com/
- IG Status: https://status.ig.com/

---

**⚠️ REMEMBER: Trading involves significant risk. Past performance does not guarantee future results. Only trade with money you can afford to lose.**
