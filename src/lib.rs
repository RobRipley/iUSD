use ic_cdk_macros::*;
mod vault_system;

// Re-export types that need to be public
pub use vault_system::{Vault, CollateralType, VaultController};

// Initialize the canister's state
thread_local! {
    static STATE: ic_cdk::storage::Storage<VaultController> = ic_cdk::storage::Storage::init(
        VaultController::default()
    );
}

#[init]
fn init() {
    STATE.with(|state| {
        let mut controller = state.borrow_mut();
        // Initialize default collateral ratios (75% LTV = 7500 basis points)
        controller.collateral_ratios.insert(CollateralType::ICP, 7500);
        controller.collateral_ratios.insert(CollateralType::CkBTC, 7500);
        controller.collateral_ratios.insert(CollateralType::CkETH, 7500);
        
        // Initialize minimum collateral amounts (example values)
        controller.min_collateral.insert(CollateralType::ICP, 1_000_000_000);    // 1 ICP
        controller.min_collateral.insert(CollateralType::CkBTC, 100_000);        // 0.001 ckBTC
        controller.min_collateral.insert(CollateralType::CkETH, 1_000_000);      // 0.01 ckETH
    });
}

// Export the candid interface
ic_cdk::export_candid!();