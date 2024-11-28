use candid::{CandidType, Deserialize};
use ic_cdk::api::management_canister::http_request::{
    HttpResponse, TransformArgs, TransformContext,
};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct PriceData {
    /// Price in USD
    price: f64,
    /// Timestamp of the price
    timestamp: u64,
    /// Source of the price
    source: String,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct AggregatedPrice {
    /// Final aggregated price
    price: f64,
    /// Timestamp of the aggregation
    timestamp: u64,
    /// Number of sources used
    sources_used: u8,
    /// Maximum deviation between sources (percentage)
    max_deviation: f64,
}

const MAX_PRICE_AGE_SECONDS: u64 = 300; // 5 minutes
const MAX_DEVIATION_THRESHOLD: f64 = 0.05; // 5% maximum deviation allowed

pub async fn fetch_prices(asset: &str) -> Result<AggregatedPrice, String> {
    let mut prices = Vec::new();
    
    // Fetch from all sources concurrently
    let mut handles = vec![];
    
    // CoinGecko
    handles.push(ic_cdk::spawn(fetch_coingecko_price(asset)));
    // Binance
    handles.push(ic_cdk::spawn(fetch_binance_price(asset)));
    // Kraken
    handles.push(ic_cdk::spawn(fetch_kraken_price(asset)));
    
    // Collect results
    for handle in handles {
        if let Ok(price_data) = handle.await {
            prices.push(price_data);
        }
    }
    
    if prices.is_empty() {
        return Err("No valid prices received from any source".to_string());
    }
    
    aggregate_prices(prices)
}

async fn fetch_coingecko_price(asset: &str) -> Result<PriceData, String> {
    let coingecko_id = match asset {
        "ICP" => "internet-computer",
        "BTC" => "bitcoin",
        "ETH" => "ethereum",
        _ => return Err(format!("Unsupported asset: {}", asset)),
    };
    
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd&include_last_updated_at=true",
        coingecko_id
    );
    
    let response = http_request(url).await?;
    let json: Value = serde_json::from_slice(&response.body)
        .map_err(|e| format!("Failed to parse CoinGecko response: {}", e))?;
    
    let price = json[coingecko_id]["usd"]
        .as_f64()
        .ok_or("Price not found in response")?;
        
    let timestamp = json[coingecko_id]["last_updated_at"]
        .as_u64()
        .unwrap_or_else(|| SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs());
    
    Ok(PriceData {
        price,
        timestamp,
        source: "coingecko".to_string(),
    })
}

async fn fetch_binance_price(asset: &str) -> Result<PriceData, String> {
    let symbol = format!("{}USDT", asset);
    let url = format!(
        "https://api.binance.com/api/v3/ticker/price?symbol={}",
        symbol
    );
    
    let response = http_request(url).await?;
    let json: Value = serde_json::from_slice(&response.body)
        .map_err(|e| format!("Failed to parse Binance response: {}", e))?;
    
    let price = json["price"]
        .as_str()
        .ok_or("Price not found in response")?
        .parse::<f64>()
        .map_err(|e| format!("Failed to parse price: {}", e))?;
    
    Ok(PriceData {
        price,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        source: "binance".to_string(),
    })
}

async fn fetch_kraken_price(asset: &str) -> Result<PriceData, String> {
    let symbol = format!("X{}ZUSD", asset);
    let url = format!(
        "https://api.kraken.com/0/public/Ticker?pair={}",
        symbol
    );
    
    let response = http_request(url).await?;
    let json: Value = serde_json::from_slice(&response.body)
        .map_err(|e| format!("Failed to parse Kraken response: {}", e))?;
    
    let price = json["result"][&symbol]["c"][0]
        .as_str()
        .ok_or("Price not found in response")?
        .parse::<f64>()
        .map_err(|e| format!("Failed to parse price: {}", e))?;
    
    Ok(PriceData {
        price,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        source: "kraken".to_string(),
    })
}

fn aggregate_prices(prices: Vec<PriceData>) -> Result<AggregatedPrice, String> {
    // Filter out stale prices
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
        
    let valid_prices: Vec<_> = prices
        .into_iter()
        .filter(|p| current_time - p.timestamp <= MAX_PRICE_AGE_SECONDS)
        .collect();
    
    if valid_prices.len() < 2 {
        return Err("Insufficient valid price sources".to_string());
    }
    
    // Calculate median price
    let mut price_values: Vec<_> = valid_prices.iter().map(|p| p.price).collect();
    price_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_price = if price_values.len() % 2 == 0 {
        (price_values[price_values.len() / 2 - 1] + price_values[price_values.len() / 2]) / 2.0
    } else {
        price_values[price_values.len() / 2]
    };
    
    // Calculate maximum deviation
    let max_deviation = price_values
        .iter()
        .map(|&p| (p - median_price).abs() / median_price)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);
    
    // Check if deviation is within acceptable range
    if max_deviation > MAX_DEVIATION_THRESHOLD {
        return Err("Price deviation too high between sources".to_string());
    }
    
    Ok(AggregatedPrice {
        price: median_price,
        timestamp: current_time,
        sources_used: valid_prices.len() as u8,
        max_deviation,
    })
}

async fn http_request(url: String) -> Result<HttpResponse, String> {
    let request_headers = vec![
        ("User-Agent".to_string(), "iUSD-Protocol-Bot".to_string()),
    ];
    
    let request = ic_cdk::api::management_canister::http_request::HttpRequest {
        url,
        method: "GET".to_string(),
        body: None,
        max_response_bytes: None,
        transform: Some(TransformContext::new(transform_response, vec![])),
        headers: request_headers,
    };
    
    ic_cdk::api::management_canister::http_request::http_request(request)
        .await
        .map_err(|(code, msg)| format!("HTTP request failed: {} - {}", code, msg))?
        .0
}

fn transform_response(response: TransformArgs) -> HttpResponse {
    HttpResponse {
        status: response.response.status,
        headers: response.response.headers,
        body: response.response.body,
    }
}

// Canister endpoints
#[update]
async fn get_price(asset: String) -> Result<AggregatedPrice, String> {
    fetch_prices(&asset).await
}

#[query]
fn get_supported_assets() -> Vec<String> {
    vec![
        "ICP".to_string(),
        "BTC".to_string(),
        "ETH".to_string(),
    ]
}