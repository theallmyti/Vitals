import { expect, test } from "vitest";
import { handlePair, handleTurnstile, apkDownloadUrl, GITHUB_RELEASES_URL } from "./http";
import { TURNSTILE_SITEKEY } from "./turnstile";

const APK = "https://github.com/theallmyti/Vitals/releases/download/v1.0.0/vitals-app-1.0.0.apk";

test("GET /pair opens the app, or downloads the latest APK when the app is missing", async () => {
  const request = new Request("https://tidy-newt-65.convex.site/pair?code=ABC234");
  const response = await handlePair(request, APK);
  expect(response.status).toBe(200);
  expect(response.headers.get("Content-Type")).toContain("text/html");

  const html = await response.text();
  expect(html).toContain("ABC 234");
  expect(html).toContain("vitals://pair?code=ABC234");
  // One Android intent link: the app if installed, otherwise straight to the APK file.
  expect(html).toContain(
    `intent://pair?code=ABC234#Intent;scheme=vitals;S.browser_fallback_url=${encodeURIComponent(APK)};end`,
  );
  expect(html).toContain(`href="${APK}"`);
});

test("GET /pair without a published APK falls back to the releases page", async () => {
  const html = await (await handlePair(new Request("https://x.convex.site/pair?code=ABC234"))).text();
  expect(html).toContain(encodeURIComponent(GITHUB_RELEASES_URL));
});

test("the APK link only ever points at this repo's release files", () => {
  expect(apkDownloadUrl(APK)).toBe(APK);
  expect(apkDownloadUrl(null)).toBe(GITHUB_RELEASES_URL);
  expect(apkDownloadUrl("https://evil.example/vitals.apk")).toBe(GITHUB_RELEASES_URL);
  expect(apkDownloadUrl(`${APK}" onclick="x`)).toBe(GITHUB_RELEASES_URL);
  expect(apkDownloadUrl("https://github.com/theallmyti/Vitals/releases/download/v1/notes.txt")).toBe(GITHUB_RELEASES_URL);
});

test("GET /turnstile renders the sign-up widget with a per-response script nonce", async () => {
  const a = handleTurnstile();
  const b = handleTurnstile();
  const html = await a.text();
  expect(html).toContain(TURNSTILE_SITEKEY);
  expect(html).toContain('action: "signup"');
  const csp = a.headers.get("Content-Security-Policy") ?? "";
  const nonce = /'nonce-([0-9a-f]+)'/.exec(csp)?.[1];
  expect(nonce).toBeTruthy();
  expect(html).toContain(`nonce="${nonce}"`);
  expect(csp).not.toContain("'unsafe-inline' https://challenges"); // scripts only by nonce or from Cloudflare
  expect(b.headers.get("Content-Security-Policy")).not.toBe(csp);
});
