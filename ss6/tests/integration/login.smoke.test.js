import { test, expect } from "@playwright/test";

test.describe("Login smoke", () => {
  test("login page loads with form elements", async ({ page }) => {
    await page.goto("/login");

    await expect(page.locator("h1")).toContainText("ss6 Web Login");
    await expect(page.locator("#login")).toBeVisible();
    await expect(page.locator("#password")).toBeVisible();
    await expect(page.locator("#loginForm button")).toContainText("Sign in");
    await expect(page.locator("#status")).toBeVisible();
  });

  test("shows error on invalid credentials", async ({ page }) => {
    await page.route("**/login", async (route) => {
      await route.fulfill({
        status: 401,
        contentType: "application/json",
        body: JSON.stringify({ ok: false, error: "invalid login or password" }),
      });
    });

    await page.goto("/login");
    await page.fill("#login", "baduser");
    await page.fill("#password", "badpass");
    await page.click("#loginForm button");

    await expect(page.locator("#status")).not.toBeEmpty();
  });

  test("redirects to main page on successful login", async ({ page }) => {
    await page.route("**/login", async (route) => {
      const reqBody = JSON.parse(route.request().postData() || "{}");
      if (reqBody.login === "admin" && reqBody.password === "admin") {
        await route.fulfill({
          status: 200,
          contentType: "application/json",
          headers: {
            "Set-Cookie": [
              "ss6_session=test-session-token; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
              "ss6_csrf=test-csrf-token; Path=/; SameSite=Lax; Max-Age=2592000",
            ].join(", "),
          },
          body: JSON.stringify({
            ok: true,
            login: "admin",
            role: "admin",
            csrf_token: "test-csrf-token",
          }),
        });
      } else {
        await route.fulfill({
          status: 401,
          contentType: "application/json",
          body: JSON.stringify({ ok: false, error: "invalid login or password" }),
        });
      }
    });

    await page.goto("/login");
    await page.fill("#login", "admin");
    await page.fill("#password", "admin");
    await page.click("#loginForm button");

    await page.waitForURL("**/");
  });
});
