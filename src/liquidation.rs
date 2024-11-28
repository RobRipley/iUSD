use candid::{CandidType, Deserialize, Principal};
use ic_cdk::api::call::CallResult;
use ic_cdk_macros::*;
use std::collections::HashMap;

/// Configuration for liquidation parameters
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct LiquidationConfig {
    /// Liquidation bonus (in basis points, e.g. 1000 = 10% discount)
    liquidation_bonus: u32,
    /// Maximum liquidation amount per transaction (in USD value)
    max_liquidation_amount: u128,
    /// Minimum liquidation amount per transaction (in USD value)
    min_liquidation_amount: u128,
    /// Whitelisted liquidator addresses
    liquidators: Vec<Principal>,
}

/// Represents a liquidation event
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct LiquidationEvent {
    /// ID of the vault being liquidated
    vault_id: u64,
    /// Amount of debt being repaid
    debt_amount: u128,
    /// Amount of collateral being liquidated
    collateral_amount: u128,
    /// Address of the liquidator
    liquidator: Principal,
    /// Timestamp of the liquidation
    timestamp: u64,
    /// Collateral type being liquidated
    collateral_type: CollateralType,
}

#[derive(Default)]
pub struct LiquidationController {
    config: LiquidationConfig,
    events: Vec<LiquidationEvent>,
}

impl LiquidationController {
    /// Scans for vaults eligible for liquidation
    pub async fn scan_vaults(&self) -> Result<Vec<u64>, String> {
        let vault_controller = ic_cdk::storage::get::<VaultController>();
        let mut liquidatable_vaults = Vec::new();
        
        // Iterate through all vaults
        for (vault_id, _) in vault_controller.vaults.iter() {
            if vault_controller.is_liquidatable(*vault_id).await? {
                liquidatable_vaults.push(*vault_id);
            }
        }
        
        Ok(liquidatable_vaults)
    }
    
    /// Executes a liquidation on a vault
    pub async fn execute_liquidation(
        &mut self,
        vault_id: u64,
        debt_to_cover: u128,
    ) -> Result<LiquidationEvent, String> {
        // Verify caller is whitelisted liquidator
        let caller = ic_cdk::caller();
        if !self.config.liquidators.contains(&caller) {
            return Err("Unauthorized liquidator".to_string());
        }
        
        let mut vault_controller = ic_cdk::storage::get_mut::<VaultController>();
        let vault = vault_controller.vaults.get(&vault_id)
            .ok_or("Vault not found")?;
            
        // Verify vault is actually liquidatable
        if !vault_controller.is_liquidatable(vault_id).await? {
            return Err("Vault is not liquidatable".to_string());
        }
        
        // Calculate collateral to seize including bonus
        let collateral_value = vault_controller
            .get_collateral_value(&vault.collateral_type, vault.collateral_amount)
            .await?;
        
        let bonus_multiplier = (10000 + self.config.liquidation_bonus) as f64 / 10000.0;
        let collateral_to_seize = (debt_to_cover as f64 * bonus_multiplier) as u128;
        
        // Verify liquidation amount is within bounds
        if collateral_to_seize > self.config.max_liquidation_amount 
            || collateral_to_seize < self.config.min_liquidation_amount {
            return Err("Invalid liquidation amount".to_string());
        }
        
        // Execute the token transfers
        // 1. Transfer iUSD from liquidator to protocol
        self.transfer_iusd_to_protocol(caller, debt_to_cover).await?;
        
        // 2. Transfer collateral to liquidator
        self.transfer_collateral_to_liquidator(
            vault_id,
            caller,
            collateral_to_seize,
            vault.collateral_type.clone(),
        ).await?;
        
        // Record the liquidation event
        let event = LiquidationEvent {
            vault_id,
            debt_amount: debt_to_cover,
            collateral_amount: collateral_to_seize,
            liquidator: caller,
            timestamp: ic_cdk::api::time(),
            collateral_type: vault.collateral_type.clone(),
        };
        
        self.events.push(event.clone());
        
        Ok(event)
    }
    
    async fn transfer_iusd_to_protocol(
        &self,
        from: Principal,
        amount: u128,
    ) -> Result<(), String> {
        let iusd_canister = Principal::from_text("CANISTER-ID-HERE").unwrap();
        
        let from_account = Account {
            owner: from,
            subaccount: None,
        };
        
        let to_account = Account {
            owner: ic_cdk::id(),  // Protocol's address
            subaccount: None,
        };
        
        // Call iUSD transfer function
        let args = TransferArgs {
            from: from_account,
            to: to_account,
            amount,
        };
        
        match ic_cdk::call(iusd_canister, "transfer", (args,)).await {
            Ok(()) => Ok(()),
            Err((code, msg)) => Err(format!("Failed to transfer iUSD: {:?} - {}", code, msg))
        }
    }
    
    async fn transfer_collateral_to_liquidator(
        &self,
        vault_id: u64,
        to: Principal,
        amount: u128,
        collateral_type: CollateralType,
    ) -> Result<(), String> {
        let collateral_canister = match collateral_type {
            CollateralType::ICP => Principal::from_text("ICP-LEDGER-CANISTER-ID").unwrap(),
            CollateralType::CkBTC => Principal::from_text("CKBTC-CANISTER-ID").unwrap(),
            CollateralType::CkETH => Principal::from_text("CKETH-CANISTER-ID").unwrap(),
        };
        
        let to_account = Account {
            owner: to,
            subaccount: None,
        };
        
        // Call appropriate transfer function based on collateral type
        let args = TransferArgs {
            to: to_account,
            amount,
        };
        
        match ic_cdk::call(collateral_canister, "transfer", (args,)).await {
            Ok(()) => Ok(()),
            Err((code, msg)) => Err(format!("Failed to transfer collateral: {:?} - {}", code, msg))
        }
    }
}

// Canister endpoints for liquidation bot interface
#[update]
async fn get_liquidatable_vaults() -> Result<Vec<u64>, String> {
    let liquidation_controller = ic_cdk::storage::get::<LiquidationController>();
    liquidation_controller.scan_vaults().await
}

#[update]
async fn liquidate_vault(vault_id: u64, debt_to_cover: u128) -> Result<LiquidationEvent, String> {
    let mut liquidation_controller = ic_cdk::storage::get_mut::<LiquidationController>();
    liquidation_controller.execute_liquidation(vault_id, debt_to_cover).await
}

#[query]
fn get_liquidation_config() -> LiquidationConfig {
    let liquidation_controller = ic_cdk::storage::get::<LiquidationController>();
    liquidation_controller.config.clone()
}

#[query]
fn get_liquidation_events() -> Vec<LiquidationEvent> {
    let liquidation_controller = ic_cdk::storage::get::<LiquidationController>();
    liquidation_controller.events.clone()
}

#[update]
fn update_liquidation_config(new_config: LiquidationConfig) -> Result<(), String> {
    // Only callable by protocol admin
    if ic_cdk::caller() != ic_cdk::id() {
        return Err("Unauthorized".to_string());
    }
    
    let mut liquidation_controller = ic_cdk::storage::get_mut::<LiquidationController>();
    liquidation_controller.config = new_config;
    Ok(())
}

#[update]
fn add_liquidator(liquidator: Principal) -> Result<(), String> {
    // Only callable by protocol admin
    if ic_cdk::caller() != ic_cdk::id() {
        return Err("Unauthorized".to_string());
    }
    
    let mut liquidation_controller = ic_cdk::storage::get_mut::<LiquidationController>();
    liquidation_controller.config.liquidators.push(liquidator);
    Ok(())
}