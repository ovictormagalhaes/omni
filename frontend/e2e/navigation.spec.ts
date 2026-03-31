import { test, expect } from "@playwright/test";

test.describe("Tab Navigation", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should switch to Liquidity Finder tab", async ({ page }) => {
    await page
      .locator("button")
      .filter({ hasText: "Liquidity Finder" })
      .first()
      .click();

    // Pool Finder content should appear
    await expect(
      page.getByText(/CLMM/i).or(page.getByText(/Token A/i)).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test("should switch to Strategy Builder tab", async ({ page }) => {
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

  test("should switch back to Earn Finder tab from another tab", async ({
    page,
  }) => {
    // Go to pools
    await page
      .locator("button")
      .filter({ hasText: "Liquidity Finder" })
      .first()
      .click();
    await page.waitForTimeout(300);

    // Go back to rates
    await page
      .locator("button")
      .filter({ hasText: "Earn Finder" })
      .first()
      .click();

    await expect(
      page.getByText(/Supply/i).or(page.getByText(/Borrow/i)).first(),
    ).toBeVisible({ timeout: 5000 });
  });
});

test.describe("Header Navigation", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("Earn button scrolls to rates tab", async ({ page }) => {
    await page
      .locator("header")
      .getByRole("button", { name: /Earn/i })
      .click();
    await expect(
      page.locator("button").filter({ hasText: "Earn Finder" }).first(),
    ).toBeVisible();
  });

  test("Liquidity button navigates to pools", async ({ page }) => {
    await page
      .locator("header")
      .getByRole("button", { name: /Liquidity/i })
      .click();
    await expect(
      page.getByText(/CLMM/i).or(page.getByText(/Token A/i)).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test("Strategy button navigates to strategy builder", async ({ page }) => {
    await page
      .locator("header")
      .getByRole("button", { name: /Strategy/i })
      .click();
    await expect(
      page
        .getByText(/Carry/i)
        .or(page.getByText(/Strategy Type/i))
        .first(),
    ).toBeVisible({ timeout: 5000 });
  });
});

test.describe("Hero CTA Navigation", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("Earn Finder CTA scrolls to rates tab", async ({ page }) => {
    await page
      .locator("section")
      .getByRole("button", { name: /Earn Finder/i })
      .click();
    await expect(
      page.locator("button").filter({ hasText: "Earn Finder" }).first(),
    ).toBeVisible();
  });

  test("Liquidity Finder CTA navigates to pools", async ({ page }) => {
    await page
      .locator("section")
      .getByRole("button", { name: /Liquidity Finder/i })
      .click();
    await expect(
      page.getByText(/CLMM/i).or(page.getByText(/Token A/i)).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test("Strategy Builder CTA navigates to strategy", async ({ page }) => {
    await page
      .locator("section")
      .getByRole("button", { name: /Strategy Builder/i })
      .click();
    await expect(
      page
        .getByText(/Carry/i)
        .or(page.getByText(/Strategy Type/i))
        .first(),
    ).toBeVisible({ timeout: 5000 });
  });
});

test.describe("Footer Navigation", () => {
  test("footer Liquidity link navigates to pools tab", async ({ page }) => {
    await page.goto("/");
    await page
      .locator("footer")
      .getByRole("button", { name: /Liquidity/i })
      .click();
    await expect(
      page.getByText(/CLMM/i).or(page.getByText(/Token A/i)).first(),
    ).toBeVisible({ timeout: 5000 });
  });
});
