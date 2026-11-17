/// <reference types="vite/client" />
import { convexTest } from "convex-test";
import { expect, test } from "vitest";
import { api, internal } from "./_generated/api";
import schema from "./schema";
import { APK_URL_PREFIX } from "./releases";
import { LATEST_AGENT_VERSION } from "./versions";

const modules = import.meta.glob("./**/*.ts");
const HASH = "e".repeat(64);
const SHA = "a".repeat(64);
const app = (version: string, code: number) => ({
  appVersion: version,
  appVersionCode: code,
  apkUrl: `${APK_URL_PREFIX}v${version}/vitals-app-${version}.apk`,
  apkSha256: SHA,
});

test("before any publish, latest reports the built-in agent version and no app", async () => {
  const t = convexTest(schema, modules);
  expect(await t.query(api.releases.latest, {})).toMatchObject({
    agentVersion: LATEST_AGENT_VERSION,
    appVersion: null,
    apkUrl: null,
  });
});

test("publish pushes the new version to latest, hello and the dashboard", async () => {
  const t = convexTest(schema, modules);
  const { token } = await t.action(api.auth.signUp, { turnstileToken: "test-token", email: "r@example.com", password: "password one" });
  await t.run(async (ctx) => {
    const [user] = await ctx.db.query("users").collect();
    await ctx.db.insert("devices", { userId: user._id, name: "pc", secretHash: HASH, lastSeenAt: Date.now(), isFake: false, agentVersion: "9.0.0" });
  });

  await t.mutation(internal.releases.publish, { agentVersion: "9.1.0", ...app("9.1.0", 7) });

  expect(await t.query(api.releases.latest, {})).toMatchObject({ agentVersion: "9.1.0", appVersionCode: 7 });
  expect(await t.mutation(api.agent.hello, { secretHash: HASH, agentVersion: "9.0.0" })).toMatchObject({ latestVersion: "9.1.0" });
  const dash = await t.query(api.dashboard.overview, { token });
  expect(dash.latestAgentVersion).toBe("9.1.0");
  expect(dash.devices[0].updateAvailable).toBe(true);
});

test("an agent-only release keeps the last published app", async () => {
  const t = convexTest(schema, modules);
  await t.mutation(internal.releases.publish, { agentVersion: "9.1.0", ...app("9.1.0", 7) });
  await t.mutation(internal.releases.publish, { agentVersion: "9.1.1" });
  expect(await t.query(api.releases.latest, {})).toMatchObject({ agentVersion: "9.1.1", appVersion: "9.1.0", appVersionCode: 7 });
});

test("publish refuses rollbacks, bad versions and APKs from anywhere else", async () => {
  const t = convexTest(schema, modules);
  await t.mutation(internal.releases.publish, { agentVersion: "9.1.0" });
  await expect(t.mutation(internal.releases.publish, { agentVersion: "9.0.9" })).rejects.toThrow(/older/);
  await expect(t.mutation(internal.releases.publish, { agentVersion: "v9.2" })).rejects.toThrow(/x\.y\.z/);
  await expect(
    t.mutation(internal.releases.publish, { agentVersion: "9.2.0", ...app("9.2.0", 8), apkUrl: "https://evil.example/app.apk" }),
  ).rejects.toThrow(/apkUrl/);
  await expect(
    t.mutation(internal.releases.publish, { agentVersion: "9.2.0", ...app("9.2.0", 8), apkSha256: "nope" }),
  ).rejects.toThrow(/apkSha256/);
});
