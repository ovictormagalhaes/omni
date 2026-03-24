# OMNI Protocol

Omnichain Lending Intelligence -- real-time DeFi lending/borrowing rate aggregator.

## Architecture

```
omni/
├── backend/          # Rust (Axum)
│   ├── src/
│   │   ├── bin/
│   │   │   ├── api.rs        # HTTP API server
│   │   │   └── worker.rs     # Daily collection worker
│   │   ├── indexers/          # Protocol integrations (12 protocols)
│   │   ├── services/          # Aggregator, cache, historical, realtime
│   │   ├── models.rs
│   │   ├── routes.rs
│   │   └── config.rs
│   └── Cargo.toml
├── frontend/         # Next.js 14 + TypeScript + Tailwind
│   ├── app/
│   ├── components/
│   ├── e2e/                   # Playwright E2E tests
│   └── playwright.config.ts
├── render.yaml       # Render deploy config (API + Worker cron)
└── docker-compose.yml
```

**Binaries:**
- `omni-api` -- HTTP API server (port 8080)
- `omni-worker` -- Daily collection cron job

**Protocols:** Aave, Kamino, Morpho, Fluid, SparkLend, JustLend, Euler, Jupiter, Lido, Marinade, Jito, RocketPool

**Chains:** Ethereum, Solana, Arbitrum, Base, Polygon, Optimism, Avalanche, Tron, and more

## Quick Start

```bash
# Backend API
cd backend
cp .env.example .env
cargo run --bin omni-api

# Worker (one-time collection)
cargo run --bin omni-worker -- collect

# Frontend
cd frontend
npm install
npm run dev

# Full stack (Docker)
docker compose up -d
```

## API

| Endpoint | Description |
|---|---|
| `GET /health` | Health check |
| `GET /api/v1/rates` | Find best rates (`?action=supply&assets=USDC&chains=arbitrum`) |
| `GET /api/v1/rates/history` | Vault APY time-series |
| `GET /api/v1/historical/backtest` | Backtest analysis |
| `GET /api/v1/assets` | List available assets |

## Deploy (Render)

The `render.yaml` defines:
- **omni-api**: Web Service (always on)
- **omni-worker**: Cron Job (daily at 03:00 UTC)

Set env vars `MONGODB_URL`, `REDIS_URL`, `THE_GRAPH_API_KEY`, `TRONGRID_API_KEY` in Render dashboard.

## Testing

```bash
# Backend unit tests
cd backend && cargo test

# Frontend lint + type check
cd frontend && npm run lint && npm run type-check

# E2E tests (Playwright)
cd frontend && npx playwright install && npm run test:e2e
```

## License

MIT
