use ic_agent::{Agent, Identity, agent::http_transport::ReqwestHttpReplicaV2Transport};
use candid::{Decode, Encode, Principal};
use serde_json::Value;
use tokio::time::{sleep, Duration};
use std::error::Error;
use std::collections::HashMap;

struct LiquidatorBot {
    agent: Agent,
    protocol_id: Principal,
    iusd_id: Principal,
    min_profit_threshold: f64,
    gas_price_threshold: f64,
    wallet_config: WalletConfig,
}

struct WalletConfig {
    identity: Box<dyn Identity>,
    iusd_balance: u128,
    collateral_balances: HashMap<String, u128>,
}

impl LiquidatorBot {
    async fn new(
        identity: Box<dyn Identity>,
        protocol_id: &str,
        iusd_id: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let transport = ReqwestHttpReplicaV2Transport::create("https://ic0.app")?;
        let agent = Agent::builder()
            .with_transport(transport)
            .with_identity(identity.clone())
            .build()?;
        
        let protocol_principal = Principal::from_text(protocol_id)?;
        let iusd_principal = Principal::from_text(iusd_id)?;
        
        Ok(Self {
            agent,
            protocol_id: protocol_principal,
            iusd_id: iusd_principal,
            min_profit_threshold: 0.5, // 0.5% minimum profit
            gas_price_threshold: 100.0, // Maximum gas price in cycles
            wallet_config: WalletConfig {
                identity,
                iusd_balance: 0,
                collateral_balances: HashMap::new(),
            },
        })
    }
    
    async fn monitor_vaults(&self) -> Result<(), Box<dyn Error>> {
        println!("Starting vault monitoring...");
        
        loop {
            // Get list of liquidatable vaults
            let liquidatable_vaults: Vec<u64> = self
                .call_protocol("get_liquidatable_vaults", ())
                .await?;
                
            for vault_id in liquidatable_vaults {
                if let Ok(profitable) = self.analyze_liquidation_opportunity(vault_id).await {
                    if profitable {
                        match self.execute_liquidation(vault_id).await {
                            Ok(_) => println!("Successfully liquidated vault {}", vault_id),
                            Err(e) => println!("Failed to liquidate vault {}: {}", vault_id, e),
                        }
                    }
                }
            }
            
            // Wait before next scan
            sleep(Duration::from_secs(30)).await;
        }
    }
    
    async fn analyze_liquidation_opportunity(&self, vault_id: u64) -> Result<bool, Box<dyn Error>> {
        // Get vault details
        let vault: Value = self
            .call_protocol("get_vault", (vault_id,))
            .await?;
            
        // Get current prices
        let collateral_price = self.get_collateral_price(&vault["collateral_type"].as_str().unwrap()).await?;
        
        // Calculate potential profit
        let collateral_amount = vault["collateral_amount"].as_u64().unwrap() as f64;
        let debt_amount = vault["debt_amount"].as_u64().unwrap() as f64;
        
        let liquidation_bonus = 0.1; // 10% bonus
        let collateral_value = collateral_amount * collateral_price;
        let debt_value = debt_amount;
        
        let potential_profit = (collateral_value * (1.0 + liquidation_bonus)) - debt_value;
        let profit_percentage = potential_profit / debt_value * 100.0;
        
        // Check if profit meets minimum threshold
        Ok(profit_percentage >= self.min_profit_threshold)
    }
    
    async fn execute_liquidation(&self, vault_id: u64) -> Result<(), Box<dyn Error>> {
        // Get vault details
        let vault: Value = self
            .call_protocol("get_vault", (vault_id,))
            .await?;
            
        let debt_amount = vault["debt_amount"].as_u64().unwrap();
        
        // Ensure we have enough iUSD
        if self.wallet_config.iusd_balance < debt_amount as u128 {
            return Err("Insufficient iUSD balance".into());
        }
        
        // Execute liquidation
        let args = Encode!(&vault_id, &debt_amount)?;
        let response: Value = self
            .call_protocol("liquidate_vault", args)
            .await?;
            
        // Update local balances
        self.update_balances().await?;
        
        Ok(())
    }
    
    async fn get_collateral_price(&self, collateral_type: &str) -> Result<f64, Box<dyn Error>> {
        // Call price feed
        let args = Encode!(&collateral_type)?;
        let price: f64 = self
            .call_protocol("get_price", args)
            .await?;
            
        Ok(price)
    }
    
    async fn update_balances(&self) -> Result<(), Box<dyn Error>> {
        // Update iUSD balance
        let args = Encode!(&self.wallet_config.identity.sender().unwrap())?;
        let iusd_balance: u128 = self
            .call_canister(self.iusd_id, "balance_of", args)
            .await?;
            
        let mut bot = self;
        bot.wallet_config.iusd_balance = iusd_balance;
        
        Ok(())
    }
    
    async fn call_protocol<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        args: impl candid::CandidType,
    ) -> Result<T, Box<dyn Error>> {
        let response = self.agent
            .update(&self.protocol_id, method)
            .with_arg(Encode!(&args)?)
            .call_and_wait()
            .await?;
            
        Ok(Decode!(response.as_slice(), T)?)
    }
    
    async fn call_canister<T: serde::de::DeserializeOwned>(
        &self,
        canister_id: Principal,
        method: &str,
        args: impl candid::CandidType,
    ) -> Result<T, Box<dyn Error>> {
        let response = self.agent
            .update(&canister_id, method)
            .with_arg(Encode!(&args)?)
            .call_and_wait()
            .await?;
            
        Ok(Decode!(response.as_slice(), T)?)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load configuration from environment or config file
    let protocol_id = std::env::var("PROTOCOL_CANISTER_ID")?;
    let iusd_id = std::env::var("IUSD_CANISTER_ID")?;
    
    // Setup identity (you'll need to implement this based on your key management approach)
    let identity = setup_identity()?;
    
    // Create and start the bot
    let bot = LiquidatorBot::new(identity, &protocol_id, &iusd_id).await?;
    
    println!("Liquidator bot starting...");
    bot.monitor_vaults().await?;
    
    Ok(())
}

// You'll need to implement this based on your key management approach
fn setup_identity() -> Result<Box<dyn Identity>, Box<dyn Error>> {
    // Implementation depends on how you want to manage keys
    unimplemented!("Implement identity setup based on your security requirements")
}