import { test, expect } from "@playwright/test";

test.describe("Home Page", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should load and show the OMNI title", async ({ page }) => {
    await expect(page).toHaveTitle(/OMNI/i);
  });

  test("should display the hero section", async ({ page }) => {
    await expect(page.getByText("Find the best")).toBeVisible();
    await expect(page.getByText("DeFi yields")).toBeVisible();
  });

  test("should show stats in hero section", async ({ page }) => {
    await expect(page.getByText("40+")).toBeVisible();
    await expect(page.getByText("Protocols")).toBeVisible();
    await expect(page.getByText("20+")).toBeVisible();
    await expect(page.getByText("Chains")).toBeVisible();
    await expect(page.getByText("24/7")).toBeVisible();
    await expect(page.getByText("Monitoring")).toBeVisible();
  });

  test("should show DeFi Yield Intelligence tag", async ({ page }) => {
    await expect(page.getByText("DeFi Yield Intelligence")).toBeVisible();
  });

  test("should display hero CTA buttons", async ({ page }) => {
    await expect(
      page.locator("section").getByRole("button", { name: /Earn Finder/i }),
    ).toBeVisible();
    await expect(
      page
        .locator("section")
        .getByRole("button", { name: /Liquidity Finder/i }),
    ).toBeVisible();
    await expect(
      page
        .locator("section")
        .getByRole("button", { name: /Strategy Builder/i }),
    ).toBeVisible();
  });

  test("should display the header with nav links", async ({ page }) => {
    await expect(page.locator("header")).toBeVisible();
    await expect(
      page.locator("header").getByRole("button", { name: /Earn/i }),
    ).toBeVisible();
    await expect(
      page.locator("header").getByRole("button", { name: /Liquidity/i }),
    ).toBeVisible();
    await expect(
      page.locator("header").getByRole("button", { name: /Strategy/i }),
    ).toBeVisible();
  });

  test("should display the footer with DYOR disclaimer", async ({ page }) => {
    await expect(page.locator("footer")).toBeVisible();
    await expect(
      page.getByText("Data is informational only. DYOR."),
    ).toBeVisible();
  });

  test("should default to Earn Finder tab", async ({ page }) => {
    await expect(
      page.locator("button").filter({ hasText: "Earn Finder" }).first(),
    ).toBeVisible();
  });
});
