use candid::{CandidType, Deserialize, Principal};
use ic_cdk::api::call::CallResult;
use ic_cdk_macros::*;
use std::collections::HashMap;

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Metadata {
    name: String,
    symbol: String,
    decimals: u8,
    total_supply: u128,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Account {
    owner: Principal,
    subaccount: Option<[u8; 32]>,
}

#[derive(Default)]
pub struct TokenState {
    /// Token metadata
    metadata: Metadata,
    /// Balances for each account
    balances: HashMap<Account, u128>,
    /// Authorized minters (vault canister)
    authorized_minters: Vec<Principal>,
    /// Transaction history
    transactions: Vec<Transaction>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Transaction {
    from: Option<Account>,
    to: Account,
    amount: u128,
    timestamp: u64,
    transaction_type: TransactionType,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum TransactionType {
    Mint,
    Burn,
    Transfer,
}

impl TokenState {
    pub fn new() -> Self {
        Self {
            metadata: Metadata {
                name: "Internet Computer USD".to_string(),
                symbol: "iUSD".to_string(),
                decimals: 8,
                total_supply: 0,
            },
            balances: HashMap::new(),
            authorized_minters: Vec::new(),
            transactions: Vec::new(),
        }
    }

    /// Mint new tokens (only callable by authorized minters)
    pub fn mint(&mut self, to: Account, amount: u128) -> Result<(), String> {
        let caller = ic_cdk::caller();
        if !self.authorized_minters.contains(&caller) {
            return Err("Unauthorized minter".to_string());
        }

        let current_balance = self.balances.get(&to).unwrap_or(&0);
        self.balances.insert(to.clone(), current_balance + amount);
        self.metadata.total_supply += amount;

        // Record transaction
        self.transactions.push(Transaction {
            from: None,
            to,
            amount,
            timestamp: ic_cdk::api::time(),
            transaction_type: TransactionType::Mint,
        });

        Ok(())
    }

    /// Burn tokens (only callable by authorized minters)
    pub fn burn(&mut self, from: Account, amount: u128) -> Result<(), String> {
        let caller = ic_cdk::caller();
        if !self.authorized_minters.contains(&caller) {
            return Err("Unauthorized minter".to_string());
        }

        let current_balance = self.balances.get(&from).unwrap_or(&0);
        if *current_balance < amount {
            return Err("Insufficient balance".to_string());
        }

        self.balances.insert(from.clone(), current_balance - amount);
        self.metadata.total_supply -= amount;

        // Record transaction
        self.transactions.push(Transaction {
            from: Some(from),
            to: Account {
                owner: Principal::anonymous(),
                subaccount: None,
            },
            amount,
            timestamp: ic_cdk::api::time(),
            transaction_type: TransactionType::Burn,
        });

        Ok(())
    }

    /// Transfer tokens between accounts
    pub fn transfer(
        &mut self,
        from: Account,
        to: Account,
        amount: u128,
    ) -> Result<(), String> {
        // Verify caller owns the source account
        let caller = ic_cdk::caller();
        if from.owner != caller {
            return Err("Unauthorized transfer".to_string());
        }

        let from_balance = self.balances.get(&from).unwrap_or(&0);
        if *from_balance < amount {
            return Err("Insufficient balance".to_string());
        }

        let to_balance = self.balances.get(&to).unwrap_or(&0);

        // Update balances
        self.balances.insert(from.clone(), from_balance - amount);
        self.balances.insert(to.clone(), to_balance + amount);

        // Record transaction
        self.transactions.push(Transaction {
            from: Some(from),
            to,
            amount,
            timestamp: ic_cdk::api::time(),
            transaction_type: TransactionType::Transfer,
        });

        Ok(())
    }
}

// Canister endpoints
#[init]
fn init() {
    ic_cdk::storage::stable_save((TokenState::new(),)).unwrap();
}

#[query]
fn metadata() -> Metadata {
    let state = ic_cdk::storage::stable_restore::<(TokenState,)>().unwrap().0;
    state.metadata
}

#[query]
fn balance_of(account: Account) -> u128 {
    let state = ic_cdk::storage::stable_restore::<(TokenState,)>().unwrap().0;
    *state.balances.get(&account).unwrap_or(&0)
}

#[update]
fn transfer(to: Account, amount: u128) -> Result<(), String> {
    let mut state = ic_cdk::storage::stable_restore::<(TokenState,)>().unwrap().0;
    let from = Account {
        owner: ic_cdk::caller(),
        subaccount: None,
    };
    let result = state.transfer(from, to, amount);
    ic_cdk::storage::stable_save((state,)).unwrap();
    result
}

// Admin functions
#[update]
fn add_minter(minter: Principal) -> Result<(), String> {
    let mut state = ic_cdk::storage::stable_restore::<(TokenState,)>().unwrap().0;
    if ic_cdk::caller() != ic_cdk::id() {
        return Err("Unauthorized".to_string());
    }
    state.authorized_minters.push(minter);
    ic_cdk::storage::stable_save((state,)).unwrap();
    Ok(())
}

// Minter functions
#[update]
fn mint(to: Account, amount: u128) -> Result<(), String> {
    let mut state = ic_cdk::storage::stable_restore::<(TokenState,)>().unwrap().0;
    let result = state.mint(to, amount);
    ic_cdk::storage::stable_save((state,)).unwrap();
    result
}

#[update]
fn burn(from: Account, amount: u128) -> Result<(), String> {
    let mut state = ic_cdk::storage::stable_restore::<(TokenState,)>().unwrap().0;
    let result = state.burn(from, amount);
    ic_cdk::storage::stable_save((state,)).unwrap();
    result
}