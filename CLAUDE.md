# OMNI Protocol — Claude Code Guidelines

## Project Overview
Omnichain DeFi rate aggregator: Rust backend (Axum + MongoDB + Redis) + React/TypeScript frontend.
Collects APY/APR data from 40+ protocols across 20+ chains.

## Mandatory Quality Rules

These rules apply AUTOMATICALLY to every code change. Do not skip them.

### Security — Apply on every code change
- NEVER hardcode API keys, secrets, or tokens. Use environment variables via `config.rs`
- NEVER trust external API responses: validate types, ranges, and nullability before using
- All MongoDB queries with user input MUST sanitize parameters (no raw string interpolation)
- HTTP requests to external APIs MUST have timeouts (max 30s)
- CORS: verify changes don't expose internal endpoints
- Rate values: reject APY > 10000% or < -100% as data errors (log warning, skip vault)
- Check for OWASP Top 10 in any route/handler change

### Data Integrity — Apply on indexer or service changes
- APY stored as percentage (5.25 = 5.25%), never basis points in storage
- Supply APY ≠ Borrow APR: never confuse them
- Normalize assets: WETH→ETH, WBTC→BTC (follow existing `normalize_asset` patterns)
- Vault IDs are SHA-256 of (protocol, chain, asset, url, operation_type, action) — changing inputs breaks time-series
- Token decimals: USDC/USDT=6, ETH=18, BTC=8 — verify conversions
- Protocol-specific units: Aave RAY=1e27, Compound mantissa=1e18

### Code Quality — Apply on every code change
- ALL code, comments, commit messages, and documentation MUST be in English. Never write Portuguese or any other language in the codebase.
- Rust: no `.unwrap()` on external data. Use `?`, `match`, or `.unwrap_or_default()`
- Rust: no blocking calls inside async context
- Rust: share `reqwest::Client` instances, don't create per-request
- Frontend: TypeScript strict mode, no `any` types
- No dead code, no commented-out code blocks
- Functions > 50 lines should be evaluated for extraction
- Error messages must include context (protocol name, chain, vault_id)

### Performance — Apply on service or route changes
- MongoDB: verify indexes exist for new query patterns
- No N+1 queries: use batch/`$in` instead of loops
- External API calls should be parallel (`tokio::join!`) when independent
- Use DefiLlama shared cache for protocols that support it
- All list endpoints MUST enforce pagination (max 100 per page)

### Testing — Apply when writing new code
- New indexers: add unit test with mocked API response
- New service methods: add test for happy path + error case
- New routes: add integration test verifying response shape
- Data transformations: add test for edge cases (0 values, missing fields, max values)

## Workflow Rules

### Before writing code
- Read existing related code first
- Check if a similar pattern already exists in the codebase

### After writing code
- Verify it compiles: `cargo check` for Rust, `npm run type-check` for frontend
- Run relevant tests: `cargo test` for backend, `npm run lint` for frontend
- If changing an indexer: verify the Protocol enum, aggregator wiring, and logo mapping are all updated

### On PR review
- Apply ALL rules above
- Check for vault_id stability (any hash input change = breaking change)
- Verify new dependencies are justified
- Check for missing error handling on external boundaries

## Architecture Reference

```
Indexers (40+ protocols) → RateAggregator → Services → MongoDB
                                                          ↓
Frontend (React) ← Axum API ← RealtimeService/HistoricalService
```

Key files:
- Models: backend/src/models.rs
- Routes: backend/src/routes.rs
- Config: backend/src/config.rs
- Aggregator: backend/src/services/aggregator.rs
- Collection: backend/src/services/collection_worker.rs
- API client: frontend/src/lib/api.ts
