import { test, expect } from "@playwright/test";

const API_URL = process.env.API_URL || "http://localhost:8080";

test.describe("API Integration", () => {
  test("health check should respond OK", async ({ request }) => {
    const response = await request.get(`${API_URL}/health`);
    expect(response.ok()).toBeTruthy();
    expect(await response.text()).toBe("OK");
  });

  test("GET /api/v1/rates should return valid response shape", async ({
    request,
  }) => {
    const response = await request.get(`${API_URL}/api/v1/rates`, {
      params: { action: "supply", page: 1, page_size: 5 },
    });
    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body).toHaveProperty("success", true);
    expect(body).toHaveProperty("results");
    expect(body).toHaveProperty("count");
    expect(body).toHaveProperty("page");
    expect(body).toHaveProperty("pageSize");
    expect(body).toHaveProperty("totalCount");
    expect(body).toHaveProperty("totalPages");
    expect(Array.isArray(body.results)).toBe(true);
  });

  test("GET /api/v1/rates with filters should return valid response", async ({
    request,
  }) => {
    const response = await request.get(`${API_URL}/api/v1/rates`, {
      params: {
        action: "supply",
        chains: "ethereum",
        protocols: "aave",
        page: 1,
        page_size: 5,
      },
    });
    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body.success).toBe(true);
    expect(Array.isArray(body.results)).toBe(true);

    // If results exist, verify shape
    for (const r of body.results) {
      expect(r).toHaveProperty("protocol");
      expect(r).toHaveProperty("chain");
      expect(r).toHaveProperty("asset");
      expect(r).toHaveProperty("apy");
      expect(r).toHaveProperty("netApy");
      expect(r).toHaveProperty("liquidity");
    }
  });

  test("GET /api/v1/rates borrow action should work", async ({ request }) => {
    const response = await request.get(`${API_URL}/api/v1/rates`, {
      params: { action: "borrow", page: 1, page_size: 5 },
    });
    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body.success).toBe(true);
    expect(Array.isArray(body.results)).toBe(true);
  });

  test("GET /api/v1/pools should return valid response shape", async ({
    request,
  }) => {
    const response = await request.get(`${API_URL}/api/v1/pools`, {
      params: { page: 1, page_size: 5 },
    });
    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body).toHaveProperty("success", true);
    expect(body).toHaveProperty("results");
    expect(body).toHaveProperty("count");
    expect(body).toHaveProperty("page");
    expect(body).toHaveProperty("totalCount");
    expect(Array.isArray(body.results)).toBe(true);

    // If results exist, verify shape
    for (const r of body.results) {
      expect(r).toHaveProperty("protocol");
      expect(r).toHaveProperty("chain");
      expect(r).toHaveProperty("pair");
      expect(r).toHaveProperty("tvlUsd");
      expect(r).toHaveProperty("feeApr24h");
    }
  });

  test("GET /api/v1/pools with filters should work", async ({ request }) => {
    const response = await request.get(`${API_URL}/api/v1/pools`, {
      params: {
        chains: "ethereum",
        protocols: "uniswap",
        page: 1,
        page_size: 5,
      },
    });
    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body.success).toBe(true);
    expect(Array.isArray(body.results)).toBe(true);
  });

  test("pagination should be enforced", async ({ request }) => {
    const response = await request.get(`${API_URL}/api/v1/rates`, {
      params: { action: "supply", page: 1, page_size: 100 },
    });
    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    // pageSize should not exceed 100
    expect(body.pageSize).toBeLessThanOrEqual(100);
  });
});
