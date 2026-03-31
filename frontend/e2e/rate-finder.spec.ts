import { test, expect } from "@playwright/test";

test.describe("Rate Finder", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    // Earn Finder is the default tab — wait for it to render
    await expect(
      page.locator("button").filter({ hasText: "Earn Finder" }).first(),
    ).toBeVisible();
  });

  test("should show supply/borrow toggle", async ({ page }) => {
    await expect(page.getByText(/Supply/i).first()).toBeVisible();
    await expect(page.getByText(/Borrow/i).first()).toBeVisible();
  });

  test("should be able to switch to borrow mode", async ({ page }) => {
    await page.getByText(/Borrow/i).first().click();
    // Should not crash — page should remain interactive
    await expect(page.getByText(/Supply/i).first()).toBeVisible();
  });

  test("should show operation type filters", async ({ page }) => {
    await expect(page.getByText(/Lending/i).first()).toBeVisible();
    await expect(page.getByText(/Vault/i).first()).toBeVisible();
    await expect(page.getByText(/Staking/i).first()).toBeVisible();
  });

  test("should show chain filter options", async ({ page }) => {
    await expect(page.getByText(/Ethereum/i).first()).toBeVisible();
    await expect(page.getByText(/Solana/i).first()).toBeVisible();
  });

  test("should show protocol filter options", async ({ page }) => {
    await expect(page.getByText(/Aave/i).first()).toBeVisible();
    await expect(page.getByText(/Morpho/i).first()).toBeVisible();
  });

  test("should have asset category or token filter", async ({ page }) => {
    await expect(
      page
        .getByText(/Stablecoin/i)
        .or(page.getByText(/Category/i))
        .first(),
    ).toBeVisible();
  });

  test("should make an API call when loaded", async ({ page }) => {
    // Intercept the rates API call to verify it fires
    const apiCall = page.waitForResponse(
      (res) => res.url().includes("/api/v1/rates") && res.status() === 200,
      { timeout: 15000 },
    );

    await page.reload();

    const response = await apiCall;
    const body = await response.json();
    expect(body).toHaveProperty("success", true);
    expect(body).toHaveProperty("results");
    expect(Array.isArray(body.results)).toBe(true);
  });

  test("should display results if data is available", async ({ page }) => {
    // Wait for the API response
    const responsePromise = page.waitForResponse(
      (res) => res.url().includes("/api/v1/rates") && res.status() === 200,
      { timeout: 15000 },
    );

    await page.reload();
    const response = await responsePromise;
    const body = await response.json();

    if (body.results.length > 0) {
      // If there is data, at least one result row should render
      await expect(page.getByText(/%/).first()).toBeVisible({ timeout: 5000 });
    }
    // If empty, the page should still not crash
    await expect(
      page.locator("button").filter({ hasText: "Earn Finder" }).first(),
    ).toBeVisible();
  });
});
