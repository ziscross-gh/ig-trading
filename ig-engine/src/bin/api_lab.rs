use ig_engine::api::rest_client::IGRestClient;
use ig_engine::api::types::IGTradeRequest;
use ig_engine::api::traits::TraderAPI;
use dotenvy::dotenv;
use std::env;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();
    
    println!("============================================================");
    println!(" 🧪 IG RUST API LAB — Standalone Injector");
    println!("============================================================");

    let api_key = env::var("IG_API_KEY").expect("IG_API_KEY not set");
    let identifier = env::var("IG_IDENTIFIER").expect("IG_IDENTIFIER not set");
    let password = env::var("IG_PASSWORD").expect("IG_PASSWORD not set");
    let environment = env::var("IG_ENVIRONMENT").unwrap_or_else(|_| "demo".to_string());
    let is_demo = environment.to_lowercase() == "demo";

    // 1. Initialize and Authenticate
    let mut client = IGRestClient::new(api_key, identifier, password, is_demo).await?;
    println!("✅ Authenticated successfully with {} API.", environment);

    // 2. Get account currency
    let accounts = client.get_accounts().await?;
    let account_currency = accounts.accounts.first()
        .map(|a| a.currency.clone())
        .unwrap_or_else(|| "USD".to_string());
    println!("💰 Account Currency: {}", account_currency);

    // 3. Get arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage:");
        println!("  List positions:  cargo run --bin api_lab list");
        println!("  Inject trade:    cargo run --bin api_lab <EPIC> <SIZE>");
        println!("  Close position:  cargo run --bin api_lab close <DEAL_ID>");
        println!("  Clear Profitable: cargo run --bin api_lab clear_profit");
        return Ok(());
    }

    if args[1] == "list" {
        println!("🔍 Fetching open positions and prices...");
        let pos_resp = client.get_positions().await?;
        if pos_resp.positions.is_empty() {
            println!("📭 No open positions found.");
        } else {
            println!("{:<20} | {:<25} | {:<4} | {:<6} | {:<10} | {:<10} | {:<10}", 
                "Deal ID", "Market", "Dir", "Size", "Entry", "Current", "PnL (Est)");
            println!("{}", "-".repeat(95));
            for wrapper in pos_resp.positions {
                let p = wrapper.position;
                let m = wrapper.market;
                
                let current_price = match client.get_market(&m.epic).await {
                    Ok(minfo) => (minfo.snapshot.bid.unwrap_or(0.0) + minfo.snapshot.offer.unwrap_or(0.0)) / 2.0,
                    Err(_) => 0.0,
                };
                
                let pnl = if p.direction == "BUY" {
                    (current_price - p.level) * p.size
                } else {
                    (p.level - current_price) * p.size
                };

                let adj_pnl = if m.epic.contains("JPY") {
                    pnl * 100.0 
                } else if m.epic.contains("GOLD") {
                    pnl
                } else {
                    pnl * 10000.0
                };

                println!("{:<20} | {:<25} | {:<4} | {:<6} | {:<10.5} | {:<10.5} | {:<10.2}", 
                    p.deal_id, m.instrument_name.unwrap_or_default(), p.direction, p.size, p.level, current_price, adj_pnl);
            }
        }
        return Ok(());
    }

    if args[1] == "close" || args[1] == "clear_profit" {
        let pos_resp = client.get_positions().await?;
        let positions_to_process = if args[1] == "clear_profit" {
            println!("🔍 Scanning for profitable positions...");
            let mut profitable = Vec::new();
            for wrapper in pos_resp.positions {
                let current_price = match client.get_market(&wrapper.market.epic).await {
                    Ok(minfo) => (minfo.snapshot.bid.unwrap_or(0.0) + minfo.snapshot.offer.unwrap_or(0.0)) / 2.0,
                    Err(_) => 0.0,
                };
                let pnl = if wrapper.position.direction == "BUY" {
                    (current_price - wrapper.position.level) * wrapper.position.size
                } else {
                    (wrapper.position.level - current_price) * wrapper.position.size
                };
                if pnl > 0.0 {
                    profitable.push(wrapper);
                }
            }
            profitable
        } else {
            if args.len() < 3 {
                println!("Error: Missing DEAL_ID. Usage: cargo run --bin api_lab close <DEAL_ID>");
                return Ok(());
            }
            let deal_id = &args[2];
            pos_resp.positions.into_iter().filter(|w| w.position.deal_id == *deal_id).collect()
        };

        if positions_to_process.is_empty() {
            println!("📭 No matching positions to close.");
            return Ok(());
        }

        for wrapper in positions_to_process {
            let p = &wrapper.position;
            let m = &wrapper.market;
            let close_direction = if p.direction == "BUY" { "SELL" } else { "BUY" };
            println!("🎬 Closing position {}: {} {} of {:?}...", p.deal_id, close_direction, p.size, m.instrument_name);
            
            match client.close_position(&p.deal_id, close_direction, p.size).await {
                Ok(resp) => {
                    println!("✅ Close submitted! Deal Reference: {}", resp.deal_reference);
                },
                Err(e) => println!("❌ Failed to close {}: {}", p.deal_id, e),
            }
        }
        return Ok(());
    }

    // Default: Open position
    let epic = &args[1];
    let size: f64 = args[2].parse().expect("Invalid size");
    let request_direction = "BUY".to_string();

    let market_info = client.get_market(epic).await?;
    println!("📈 Market Name: {}", market_info.instrument.name);
    println!("💹 Allowed Currencies: {:?}", market_info.instrument.currencies.iter().map(|c| &c.code).collect::<Vec<_>>());
    
    let currency_code = if epic.contains("JPY") {
        "JPY".to_string()
    } else if epic.contains("EURUSD") || epic.contains("GBPUSD") || epic.contains("AUDUSD") { 
        "USD".to_string() 
    } else {
        account_currency.clone()
    };

    let bid = market_info.snapshot.bid.unwrap_or(0.0);
    let offer = market_info.snapshot.offer.unwrap_or(0.0);
    let price = if request_direction == "BUY" { offer } else { bid };
    
    let pip_scale = if epic.contains("JPY") { 0.01 } else if epic.contains("GOLD") { 1.0 } else { 0.0001 };
    
    let (stop_level, limit_level) = if request_direction == "BUY" {
        (Some(price - 1000.0 * pip_scale), Some(price + 2000.0 * pip_scale))
    } else {
        (Some(price + 1000.0 * pip_scale), Some(price - 1000.0 * pip_scale))
    };

    let request = IGTradeRequest {
        epic: epic.clone(),
        direction: request_direction.clone(),
        size,
        order_type: "MARKET".to_string(),
        level: None,
        stop_level, 
        stop_distance: None, 
        limit_level,
        currency_code: Some(currency_code),
        guaranteed_stop: Some(false),
        trailing_stop: None,
        force_open: Some(true),
        expiry: "-".to_string(),
    };

    println!("🚀 Injecting trade: {} {} of {} @ {} (SL={:?}, TP={:?})...", 
        request_direction, size, epic, price, stop_level, limit_level);
    
    let resp = client.open_position(request).await?;
    println!("✅ Trade SUBMITTED! Deal Reference: {}", resp.deal_reference);
    
    for i in 0..10 {
        sleep(Duration::from_secs(1)).await;
        match client.get_deal_confirmation(&resp.deal_reference).await {
            Ok(conf) => {
                println!("📊 Result [Attempt {}]: deal_status={}, reason={:?}, deal_id={}", 
                    i+1, conf.deal_status, conf.reason, conf.deal_id);
                if conf.deal_status != "PENDING" {
                    break;
                }
            }
            Err(e) => println!("⏳ [Attempt {}] Waiting for confirmation... ({})", i+1, e),
        }
    }

    Ok(())
}
