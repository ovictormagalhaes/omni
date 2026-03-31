import { test, expect } from "@playwright/test";

test.describe("Mobile Responsive", () => {
  test.skip(
    ({ browserName }) => browserName !== "chromium",
    "Mobile tests only on chromium",
  );

  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should show mobile hamburger menu", async ({ page, isMobile }) => {
    test.skip(!isMobile, "Only runs on mobile viewport");
    const menuButton = page.locator("header button").last();
    await expect(menuButton).toBeVisible();
  });

  test("should open mobile menu on hamburger click", async ({
    page,
    isMobile,
  }) => {
    test.skip(!isMobile, "Only runs on mobile viewport");

    await page.locator("header button").last().click();

    await expect(
      page.locator("nav").getByRole("button", { name: /Earn/i }),
    ).toBeVisible();
    await expect(
      page.locator("nav").getByRole("button", { name: /Liquidity/i }),
    ).toBeVisible();
    await expect(
      page.locator("nav").getByRole("button", { name: /Strategy/i }),
    ).toBeVisible();
  });

  test("should navigate from mobile menu", async ({ page, isMobile }) => {
    test.skip(!isMobile, "Only runs on mobile viewport");

    await page.locator("header button").last().click();
    await page
      .locator("nav")
      .getByRole("button", { name: /Liquidity/i })
      .click();

    // Should navigate to pools
    await expect(
      page.getByText(/CLMM/i).or(page.getByText(/Token A/i)).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test("should display hero correctly on mobile", async ({
    page,
    isMobile,
  }) => {
    test.skip(!isMobile, "Only runs on mobile viewport");

    await expect(page.getByText("Find the best")).toBeVisible();
    await expect(page.getByText("DeFi yields")).toBeVisible();
    await expect(page.getByText("40+")).toBeVisible();
  });
});

test.describe("Desktop Layout", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should show desktop nav links in header", async ({
    page,
    isMobile,
  }) => {
    test.skip(!!isMobile, "Only runs on desktop viewport");

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

  test("should display all three main tab buttons", async ({
    page,
    isMobile,
  }) => {
    test.skip(!!isMobile, "Only runs on desktop viewport");

    await expect(
      page.locator("button").filter({ hasText: "Earn Finder" }).first(),
    ).toBeVisible();
    await expect(
      page.locator("button").filter({ hasText: "Liquidity Finder" }).first(),
    ).toBeVisible();
    await expect(
      page.locator("button").filter({ hasText: "Strategy Builder" }).first(),
    ).toBeVisible();
  });
});
