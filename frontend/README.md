# OMNI Frontend

Next.js 14 dashboard for finding DeFi lending/borrowing rates.

## Setup

```bash
npm install
cp .env.example .env
npm run dev
```

App runs at `http://localhost:3000`

## Scripts

| Script | Description |
|---|---|
| `npm run dev` | Start dev server |
| `npm run build` | Build for production |
| `npm run lint` | Run ESLint |
| `npm run type-check` | TypeScript validation |
| `npm run test:e2e` | Run Playwright E2E tests |
| `npm run test:e2e:ui` | E2E tests with Playwright UI |

## E2E Tests (Playwright)

```bash
npx playwright install
npm run test:e2e
```

Tests are in `e2e/` directory. Config in `playwright.config.ts`.

## Tech Stack

- Next.js 14 (App Router, static export)
- TypeScript
- Tailwind CSS
- Recharts (charts)
- Axios (HTTP)
- Playwright (E2E)

## License

MIT
