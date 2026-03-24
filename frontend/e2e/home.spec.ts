import { test, expect } from "@playwright/test";

test.describe("Home Page", () => {
  test("should load and show header", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle(/OMNI/i);
  });

  test("should display rate finder tab", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText(/Rate Finder/i).first()).toBeVisible();
  });

  test("health check API should respond", async ({ request }) => {
    const apiUrl = process.env.API_URL || "http://localhost:8080";
    const response = await request.get(`${apiUrl}/health`);
    expect(response.ok()).toBeTruthy();
    expect(await response.text()).toBe("OK");
  });
});
