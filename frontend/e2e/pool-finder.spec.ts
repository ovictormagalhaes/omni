import { test, expect } from "@playwright/test";

test.describe("Pool Finder", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    // Navigate to Liquidity Finder tab
    await page
      .locator("button")
      .filter({ hasText: "Liquidity Finder" })
      .first()
      .click();
    await expect(
      page.getByText(/CLMM/i).or(page.getByText(/Token A/i)).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test("should show pool type filters (CLMM and Standard)", async ({
    page,
  }) => {
    await expect(page.getByText(/CLMM/i).first()).toBeVisible();
    await expect(page.getByText(/Standard/i).first()).toBeVisible();
  });

  test("should show chain filters", async ({ page }) => {
    await expect(page.getByText(/Ethereum/i).first()).toBeVisible();
    await expect(page.getByText(/Solana/i).first()).toBeVisible();
  });

  test("should show protocol filters", async ({ page }) => {
    await expect(page.getByText(/Uniswap/i).first()).toBeVisible();
    await expect(page.getByText(/Curve/i).first()).toBeVisible();
  });

  test("should show token pair filter controls", async ({ page }) => {
    await expect(
      page
        .getByText(/Token A/i)
        .or(page.getByText(/Category/i))
        .first(),
    ).toBeVisible();
  });

  test("should make an API call for pools", async ({ page }) => {
    const apiCall = page.waitForResponse(
      (res) =>
        res.url().includes("/api/v1/pools") &&
        !res.url().includes("history") &&
        res.status() === 200,
      { timeout: 15000 },
    );

    // Trigger a search (reload forces the component to re-mount and fetch)
    await page.reload();
    await page
      .locator("button")
      .filter({ hasText: "Liquidity Finder" })
      .first()
      .click();

    const response = await apiCall;
    const body = await response.json();
    expect(body).toHaveProperty("success", true);
    expect(body).toHaveProperty("results");
    expect(Array.isArray(body.results)).toBe(true);
  });

  test("should display results if data is available", async ({ page }) => {
    const responsePromise = page.waitForResponse(
      (res) =>
        res.url().includes("/api/v1/pools") &&
        !res.url().includes("history") &&
        res.status() === 200,
      { timeout: 15000 },
    );

    await page.reload();
    await page
      .locator("button")
      .filter({ hasText: "Liquidity Finder" })
      .first()
      .click();

    const response = await responsePromise;
    const body = await response.json();

    if (body.results.length > 0) {
      // At least one pool row with % value should render
      await expect(page.getByText(/%/).first()).toBeVisible({ timeout: 5000 });
    }
    // Page should remain stable regardless
    await expect(
      page.locator("button").filter({ hasText: "Liquidity Finder" }).first(),
    ).toBeVisible();
  });
});
