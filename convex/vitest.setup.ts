// Test stand-in for Cloudflare Turnstile siteverify (convex/convex/turnstile.ts). Lives outside
// convex/convex/ so it is never pushed to the deployment.
import { beforeEach, vi } from "vitest";

const SITEVERIFY = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

// Test tokens: "test-token" passes; the others fail in one specific way each.
const ANSWERS: Record<string, object> = {
  "test-token": { success: true, action: "signup", hostname: "tidy-newt-65.convex.site" },
  "wrong-action": { success: true, action: "login", hostname: "tidy-newt-65.convex.site" },
  "wrong-host": { success: true, action: "signup", hostname: "evil.example.com" },
  "localhost-token": { success: true, action: "signup", hostname: "localhost" },
};

// Emails "sent" through Resend in the current test (email.ts).
export const sentEmails: { to: string; subject: string; text: string }[] = [];

beforeEach(() => {
  sentEmails.length = 0;
  process.env.RESEND_API_KEY = "re_test_key";
  process.env.EMAIL_FROM = "noreply@example.com";
  process.env.TURNSTILE_SECRET = "test-secret";
  process.env.TURNSTILE_HOSTNAMES = "tidy-newt-65.convex.site, pc-monitor.pages.dev";
  const realFetch = globalThis.fetch;
  vi.stubGlobal("fetch", async (input: RequestInfo | URL, init?: RequestInit) => {
    if (String(input) === "https://api.resend.com/emails") {
      const auth = new Headers(init?.headers).get("Authorization");
      if (auth !== "Bearer re_test_key") {
        return Response.json({ statusCode: 401, name: "validation_error", message: "API key is invalid" }, { status: 401 });
      }
      const mail = JSON.parse(String(init?.body));
      sentEmails.push({ to: mail.to, subject: mail.subject, text: mail.text });
      return Response.json({ id: "49a3999c-0ce1-4ea6-ab68-afcd6dc2e794" });
    }
    if (String(input) !== SITEVERIFY) return realFetch(input, init);
    const form = new URLSearchParams(String(init?.body ?? ""));
    const token = form.get("response") ?? "";
    if (token === "network-fail") throw new TypeError("fetch failed");
    if (token === "server-error") return new Response("oops", { status: 500 });
    if (form.get("secret") !== "test-secret") {
      return Response.json({ success: false, "error-codes": ["invalid-input-secret"] });
    }
    return Response.json(ANSWERS[token] ?? { success: false, "error-codes": ["invalid-input-response"] });
  });
});
