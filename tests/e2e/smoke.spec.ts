import { expect, test, type Page } from "@playwright/test";

const API_ORIGIN = "http://127.0.0.1:8080";
const NAV_LABELS = ["Home", "Movies", "TV Shows", "Library", "My List", "Queue"];

function noHorizontalOverflow(page: Page): Promise<boolean> {
  return page.evaluate(
    () => document.documentElement.scrollWidth <= window.innerWidth + 1,
  );
}

test("boot renders the real gate, with no unexpected console errors or overflow", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() !== "error") return;
    // The offline gate deliberately calls the backend; a failed request to it
    // is expected when no local service is running.
    if (msg.location().url.startsWith(API_ORIGIN)) return;
    errors.push(`${msg.text()} @ ${msg.location().url}`);
  });
  page.on("pageerror", (err) => errors.push(String(err)));

  await page.goto("/");

  const offline = page.getByRole("heading", {
    name: "The library service is not answering",
  });
  const profileGate = page.getByRole("heading", { name: "Who's watching?" });
  await expect(offline.or(profileGate)).toBeVisible({ timeout: 15_000 });

  if (await offline.isVisible()) {
    await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
    await expect(page.getByText(`Expected at ${API_ORIGIN}`)).toBeVisible();
  } else {
    await page.locator('button[aria-label^="Watch as "]').first().click();
    const nav = page.locator('nav[aria-label="Primary"]');
    await expect(nav).toBeVisible();
    for (const label of NAV_LABELS) {
      await expect(nav.getByRole("link", { name: label, exact: true })).toBeVisible();
    }
    await expect(page.getByRole("link", { name: "MEDIA HAVEN home" })).toBeVisible();
    await expect(page.getByRole("link", { name: "Search the library" })).toBeVisible();
  }

  expect(errors, `console errors: ${errors.join(" | ")}`).toEqual([]);
  expect(await noHorizontalOverflow(page), "horizontal overflow").toBe(true);
});
