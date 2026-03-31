import { test, expect } from "@playwright/test";

test.describe("Strategy Builder", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page
      .locator("button")
      .filter({ hasText: "Strategy Builder" })
      .first()
      .click();
    await expect(
      page
        .getByText(/Carry/i)
        .or(page.getByText(/Strategy Type/i))
        .first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test("should display strategy type options", async ({ page }) => {
    await expect(page.getByText(/Carry/i).first()).toBeVisible();
  });

  test("should show step labels", async ({ page }) => {
    await expect(page.getByText(/Chains/i).first()).toBeVisible();
  });

  test("should show chain and protocol selection", async ({ page }) => {
    await expect(page.getByText(/Ethereum/i).first()).toBeVisible();
    await expect(page.getByText(/Aave/i).first()).toBeVisible();
  });

  test("should have step navigation button", async ({ page }) => {
    await expect(
      page
        .getByRole("button", { name: /Next/i })
        .or(page.getByRole("button", { name: /Continue/i }))
        .first(),
    ).toBeVisible();
  });

  test("should not crash when navigating steps", async ({ page }) => {
    const nextBtn = page
      .getByRole("button", { name: /Next/i })
      .or(page.getByRole("button", { name: /Continue/i }))
      .first();

    if (await nextBtn.isVisible()) {
      await nextBtn.click();
      // Page should remain stable
      await expect(page.locator("body")).toBeVisible();
    }
  });
});
