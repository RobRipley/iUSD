# iUSD Protocol

A decentralized finance (DeFi) application built on the Internet Computer (ICP) blockchain that introduces a stablecoin called iUSD. The protocol allows users to lock collateral assets (ICP, ckBTC, and ckETH) and mint iUSD against them, similar to MakerDAO's DAI but leveraging ICP's unique capabilities.

## Core Features

- Multi-collateral lending platform
- 75% loan-to-value (LTV) ratio
- Real-time price feeds using ICP's HTTPS outcalls
- Automated liquidation system
- Decentralized governance (planned)

## Project Structure

```
iUSD/
├── src/
│   ├── lib.rs                 # Main canister entry point
│   ├── vault_system.rs        # Core vault management system
│   ├── iusd_token.rs         # iUSD token implementation (ICRC-2 compatible)
│   ├── price_feed.rs         # Price oracle system
│   ├── liquidation.rs        # Liquidation mechanism
│   └── bin/
│       └── liquidator_bot.rs # Off-chain liquidation bot
├── Cargo.toml                # Project dependencies
└── dfx.json                  # Internet Computer project config
```

## Component Details

### Vault System (`vault_system.rs`)
- Manages user vaults and collateral
- Handles minting and burning of iUSD
- Implements safety checks for collateral ratios
- **TODO:** Update `CANISTER-ID-HERE` placeholder with actual iUSD canister ID

### iUSD Token (`iusd_token.rs`)
- ICRC-2 compatible token implementation
- Implements minting/burning mechanics
- Includes transfer functionality
- Maintains transaction history

### Price Feed System (`price_feed.rs`)
- Fetches prices from multiple sources:
  - CoinGecko
  - Binance
  - Kraken
- Implements median price calculation
- Includes staleness checks
- Price deviation monitoring

### Liquidation System (`liquidation.rs`)
- Monitors vault health
- Executes liquidations when collateral ratio drops
- Handles collateral auctions
- **TODO:** Update canister IDs for:
  - ICP Ledger
  - ckBTC
  - ckETH

### Liquidator Bot (`liquidator_bot.rs`)
- Off-chain monitoring system
- Automated liquidation execution
- Profit calculation
- **TODO:** Implement `setup_identity()` function for key management

## Setup Requirements

1. Internet Computer SDK (dfx)
2. Rust toolchain
3. Required canister IDs
4. Access to price feed APIs

## Configuration

### Required Environment Variables
```bash
PROTOCOL_CANISTER_ID="your-protocol-canister-id"
IUSD_CANISTER_ID="your-iusd-canister-id"
```

### Collateral Settings
- ICP: 75% LTV ratio
- ckBTC: 75% LTV ratio
- ckETH: 75% LTV ratio

## Remaining Tasks

1. **Price Feed System**
   - Implement emergency price feed fallbacks
   - Add more sophisticated price aggregation
   - Set up monitoring for API health

2. **Liquidation System**
   - Add monitoring and alerting
   - Implement parallel liquidation execution
   - Add configuration file system
   - Complete keeper incentive mechanism

3. **Governance**
   - Implement governance token
   - Create proposal system
   - Set up parameter adjustment mechanism

4. **Security**
   - Complete security audit
   - Implement emergency shutdown mechanism
   - Add rate limiting

5. **Testing**
   - Write comprehensive test suite
   - Set up integration tests
   - Perform load testing

## Development Progress

- [x] Basic vault system implementation
- [x] iUSD token implementation
- [x] Price feed system
- [x] Liquidation mechanism
- [x] Liquidator bot framework
- [ ] Governance system
- [ ] Emergency mechanisms
- [ ] Testing framework
- [ ] Production deployment

## Security Considerations

- Price feed manipulation resistance
- Liquidation race conditions
- Flash loan attack prevention
- Oracle failure scenarios
- Collateral volatility management

## Deployment Checklist

1. Deploy token canister
2. Deploy vault system
3. Configure price feeds
4. Set up liquidation system
5. Test liquidator bot
6. Configure governance parameters

## Important Notes

- All monetary values use 8 decimal places
- Minimum collateral requirements vary by asset
- Liquidation bonus is configurable (default 10%)
- Price feeds require 2/3 sources to agree within 5%
- System uses weighted median for final prices

## Contribution Guidelines

1. Fork the repository
2. Create feature branch
3. Submit pull request
4. Ensure tests pass
5. Update documentation

## License

[Add chosen license]

## Contact

[Add contact information]
