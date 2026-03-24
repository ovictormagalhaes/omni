# OMNI Backend

Rust API server and collection worker for DeFi lending rate aggregation.

## Binaries

| Binary | Description | Command |
|---|---|---|
| `omni-api` | HTTP API server | `cargo run --bin omni-api` |
| `omni-worker` | Daily collection worker | `cargo run --bin omni-worker -- collect` |

## Setup

```bash
cp .env.example .env
cargo build --release
```

## API Endpoints

| Endpoint | Method | Description |
|---|---|---|
| `/health` | GET | Health check |
| `/api/v1/rates` | GET | Find best rates |
| `/api/v1/rates/history` | GET | Vault APY history |
| `/api/v1/historical/backtest` | GET | Backtest analysis |
| `/api/v1/assets` | GET | List assets |

Query parameters for `/api/v1/rates`:
- `action`: supply / borrow
- `assets`: USDC,ETH,SOL...
- `chains`: arbitrum,base,solana...
- `protocols`: aave,morpho,kamino...
- `min_liquidity`: minimum USD (default 1M)

## Worker Commands

```bash
# Daily collection
cargo run --bin omni-worker -- collect

# Backfill only
cargo run --bin omni-worker -- collect --backfill-only

# Reset collections and re-collect
cargo run --bin omni-worker -- reset
```

## Testing

```bash
cargo test
```

## Docker

```bash
docker build -t omni-backend .

# Run API
docker run -p 8080:8080 --env-file .env omni-backend omni-api

# Run Worker
docker run --env-file .env omni-backend omni-worker collect
```

## License

MIT
