import { Page } from "@playwright/test";

/**
 * Seed data helpers for e2e tests.
 *
 * When running against a fresh database the API returns empty results.
 * These helpers insert minimal seed documents via the MongoDB wire protocol
 * so that UI tests have real data to exercise.
 *
 * For now, the tests are designed to be resilient to empty results:
 * they verify the UI structure and controls work, not specific data values.
 */

const API_URL = process.env.API_URL || "http://localhost:8080";

/** Wait until the backend health check responds. */
export async function waitForApi(
  page: Page,
  { timeoutMs = 30_000 } = {},
): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const res = await page.request.get(`${API_URL}/health`);
      if (res.ok()) return;
    } catch {
      // not ready yet
    }
    await page.waitForTimeout(1000);
  }
  throw new Error(`API at ${API_URL} did not become healthy within ${timeoutMs}ms`);
}
