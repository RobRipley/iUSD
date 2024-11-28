use candid::{CandidType, Deserialize, Principal};
use ic_cdk::api::call::CallResult;
use std::collections::HashMap;
use ic_cdk_macros::*;
use crate::price_feed::{self, AggregatedPrice};

/// Supported collateral types
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CollateralType {
    ICP,
    CkBTC,
    CkETH,
}

/// Represents a user's vault
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Vault {
    /// Owner of the vault
    owner: String,
    /// Amount of collateral deposited
    collateral_amount: u128,
    /// Type of collateral
    collateral_type: CollateralType,
    /// Amount of iUSD debt
    debt_amount: u128,
    /// Last updated timestamp
    last_updated: u64,
}

#[derive(CandidType)]
struct Account {
    owner: Principal,
    subaccount: Option<[u8; 32]>,
}

#[derive(CandidType)]
struct MintArgs {
    to: Account,
    amount: u128,
}

#[derive(CandidType)]
struct BurnArgs {
    from: Account,
    amount: u128,
}

/// Main vault controller
#[derive(Default)]
pub struct VaultController {
    /// Maps vault_id to Vault
    vaults: HashMap<u64, Vault>,
    /// Next available vault ID
    next_vault_id: u64,
    /// Collateralization ratios for each asset (in basis points, e.g. 7500 = 75%)
    collateral_ratios: HashMap<CollateralType, u32>,
    /// Minimum collateral amounts
    min_collateral: HashMap<CollateralType, u128>,
}

impl VaultController {
    /// Creates a new vault
    pub fn create_vault(
        &mut self,
        owner: String,
        collateral_type: CollateralType,
    ) -> Result<u64, &'static str> {
        let vault = Vault {
            owner,
            collateral_amount: 0,
            collateral_type,
            debt_amount: 0,
            last_updated: ic_cdk::api::time(),
        };
        
        let vault_id = self.next_vault_id;
        self.vaults.insert(vault_id, vault);
        self.next_vault_id += 1;
        
        Ok(vault_id)
    }
    
    /// Deposits collateral into a vault
    pub fn deposit_collateral(
        &mut self,
        vault_id: u64,
        amount: u128,
    ) -> Result<(), &'static str> {
        let vault = self.vaults.get_mut(&vault_id)
            .ok_or("Vault not found")?;
            
        // Verify minimum collateral amount
        let min_amount = self.min_collateral.get(&vault.collateral_type)
            .ok_or("Collateral type not supported")?;
            
        if vault.collateral_amount + amount < *min_amount {
            return Err("Amount below minimum collateral requirement");
        }
        
        vault.collateral_amount += amount;
        vault.last_updated = ic_cdk::api::time();
        
        Ok(())
    }

    // Helper function for getting collateral value
    async fn get_collateral_value(
        &self,
        collateral_type: &CollateralType,
        amount: u128,
    ) -> Result<u128, String> {
        let asset = match collateral_type {
            CollateralType::ICP => "ICP",
            CollateralType::CkBTC => "BTC",
            CollateralType::CkETH => "ETH",
        };
        
        let price_data = price_feed::fetch_prices(asset).await?;
        
        // Convert amount to USD value
        // Note: amount is in base units (e.g., e8s for ICP), so we need to adjust decimals
        let decimals = match collateral_type {
            CollateralType::ICP => 8,
            CollateralType::CkBTC => 8,
            CollateralType::CkETH => 18,
        };
        
        let amount_float = amount as f64 / (10u128.pow(decimals) as f64);
        let value_usd = amount_float * price_data.price;
        
        // Convert to base units (iUSD uses 8 decimals)
        Ok((value_usd * 100_000_000.0) as u128)
    }
    
    /// Withdraws collateral from a vault
    pub async fn withdraw_collateral(
        &mut self,
        vault_id: u64,
        amount: u128,
    ) -> Result<(), String> {
        let vault = self.vaults.get_mut(&vault_id)
            .ok_or("Vault not found")?;
            
        if vault.collateral_amount < amount {
            return Err("Insufficient collateral balance".to_string());
        }
        
        // Get current collateral value in USD
        let remaining_collateral = vault.collateral_amount - amount;
        let collateral_value = self.get_collateral_value(&vault.collateral_type, remaining_collateral).await?;
        
        // Check if withdrawal would break LTV ratio
        let ratio = self.collateral_ratios.get(&vault.collateral_type)
            .ok_or("Collateral type not supported")?;
        
        let max_debt = (collateral_value * (*ratio as u128)) / 10000;
        if vault.debt_amount * 100 > max_debt {
            return Err("Withdrawal would exceed maximum LTV".to_string());
        }
        
        vault.collateral_amount = remaining_collateral;
        vault.last_updated = ic_cdk::api::time();
        
        Ok(())
    }

    async fn mint_iusd_tokens(&self, to: Account, amount: u128) -> Result<(), String> {
        // Call iUSD canister's mint function
        let iusd_canister = Principal::from_text("CANISTER-ID-HERE").unwrap();
        let args = MintArgs { to, amount };
        
        match ic_cdk::call(iusd_canister, "mint", (args,)).await {
            Ok(()) => Ok(()),
            Err((code, msg)) => Err(format!("Failed to mint iUSD: {:?} - {}", code, msg))
        }
    }

    async fn burn_iusd_tokens(&self, from: Account, amount: u128) -> Result<(), String> {
        // Call iUSD canister's burn function
        let iusd_canister = Principal::from_text("CANISTER-ID-HERE").unwrap();
        let args = BurnArgs { from, amount };
        
        match ic_cdk::call(iusd_canister, "burn", (args,)).await {
            Ok(()) => Ok(()),
            Err((code, msg)) => Err(format!("Failed to burn iUSD: {:?} - {}", code, msg))
        }
    }
    
    /// Mints iUSD against vault collateral
    pub async fn mint_iusd(
        &mut self,
        vault_id: u64,
        amount: u128,
    ) -> Result<(), String> {
        let vault = self.vaults.get_mut(&vault_id)
            .ok_or("Vault not found")?;
            
        // Get current collateral value in USD
        let collateral_value = self.get_collateral_value(&vault.collateral_type, vault.collateral_amount).await?;
        
        // Calculate maximum allowed debt
        let ratio = self.collateral_ratios.get(&vault.collateral_type)
            .ok_or("Collateral type not supported")?;
        
        let max_debt = (collateral_value * (*ratio as u128)) / 10000;
        if vault.debt_amount + amount > max_debt {
            return Err("Mint would exceed maximum LTV".to_string());
        }
        
        // Mint tokens
        let to = Account {
            owner: Principal::from_text(&vault.owner).map_err(|e| e.to_string())?,
            subaccount: None,
        };
        self.mint_iusd_tokens(to, amount).await?;
        
        // Update vault state
        vault.debt_amount += amount;
        vault.last_updated = ic_cdk::api::time();
        
        Ok(())
    }
    
    /// Repays iUSD debt
    pub async fn repay_debt(
        &mut self,
        vault_id: u64,
        amount: u128,
    ) -> Result<(), String> {
        let vault = self.vaults.get_mut(&vault_id)
            .ok_or("Vault not found")?;
            
        if vault.debt_amount < amount {
            return Err("Repayment amount exceeds debt".to_string());
        }
        
        // Burn tokens first
        let from = Account {
            owner: Principal::from_text(&vault.owner).map_err(|e| e.to_string())?,
            subaccount: None,
        };
        self.burn_iusd_tokens(from, amount).await?;
        
        // Update vault state
        vault.debt_amount -= amount;
        vault.last_updated = ic_cdk::api::time();
        
        Ok(())
    }
    
    /// Checks if a vault is eligible for liquidation
    pub async fn is_liquidatable(&self, vault_id: u64) -> Result<bool, String> {
        let vault = self.vaults.get(&vault_id)
            .ok_or("Vault not found")?;
            
        // Get current collateral value in USD
        let collateral_value = self.get_collateral_value(&vault.collateral_type, vault.collateral_amount).await?;
        
        // Get liquidation threshold (slightly higher than LTV ratio)
        let ratio = self.collateral_ratios.get(&vault.collateral_type)
            .ok_or("Collateral type not supported")?;
        
        // Liquidation threshold is 5% above the maximum LTV
        let liquidation_threshold = (*ratio as u128) * 95 / 100; // 95% of LTV ratio
        let max_debt = (collateral_value * liquidation_threshold) / 10000;
        
        Ok(vault.debt_amount > max_debt)
    }
    
    /// Get vault health factor
    pub async fn get_health_factor(&self, vault_id: u64) -> Result<f64, String> {
        let vault = self.vaults.get(&vault_id)
            .ok_or("Vault not found")?;
            
        let collateral_value = self.get_collateral_value(&vault.collateral_type, vault.collateral_amount).await?;
        
        if vault.debt_amount == 0 {
            return Ok(f64::INFINITY);
        }
        
        let health_factor = (collateral_value as f64) / (vault.debt_amount as f64);
        Ok(health_factor)
    }
}

// Canister endpoints
#[update]
async fn create_vault(owner: String, collateral_type: CollateralType) -> Result<u64, String> {
    let controller = ic_cdk::storage::get_mut::<VaultController>();
    controller.create_vault(owner, collateral_type)
        .map_err(|e| e.to_string())
}

#[query]
fn get_vault(vault_id: u64) -> Result<Vault, String> {
    let controller = ic_cdk::storage::get::<VaultController>();
    controller.vaults.get(&vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())
}

#[update]
async fn withdraw_collateral(vault_id: u64, amount: u128) -> Result<(), String> {
    let controller = ic_cdk::storage::get_mut::<VaultController>();
    controller.withdraw_collateral(vault_id, amount).await
}

#[update]
async fn mint_iusd(vault_id: u64, amount: u128) -> Result<(), String> {
    let controller = ic_cdk::storage::get_mut::<VaultController>();
    controller.mint_iusd(vault_id, amount).await
}

#[update]
async fn repay_debt(vault_id: u64, amount: u128) -> Result<(), String> {
    let controller = ic_cdk::storage::get_mut::<VaultController>();
    controller.repay_debt(vault_id, amount).await
}

#[update]
async fn check_liquidatable(vault_id: u64) -> Result<bool, String> {
    let controller = ic_cdk::storage::get::<VaultController>();
    controller.is_liquidatable(vault_id).await
}

#[query]
async fn get_health_factor(vault_id: u64) -> Result<f64, String> {
    let controller = ic_cdk::storage::get::<VaultController>();
    controller.get_health_factor(vault_id).await
}