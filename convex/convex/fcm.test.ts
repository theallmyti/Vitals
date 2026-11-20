/// <reference types="vite/client" />
import { convexTest } from "convex-test";
import { expect, test } from "vitest";
import { api, internal } from "./_generated/api";
import schema from "./schema";

const modules = import.meta.glob("./**/*.ts");

test("alerts table records are readable by getAlertPushDetails", async () => {
  const t = convexTest(schema, modules);
  const { token } = await t.action(api.auth.signUp, {
    turnstileToken: "test-token",
    email: "alertuser@example.com",
    password: "password123",
  });
  
  let alertId: any;
  await t.run(async (ctx) => {
    const [user] = await ctx.db.query("users").collect();
    const devId = await ctx.db.insert("devices", {
      name: "Desktop PC",
      secretHash: "a".repeat(64),
      lastSeenAt: Date.now(),
      isFake: false,
      userId: user._id,
    });

    await ctx.db.insert("pushTokens", {
      userId: user._id,
      fcmToken: "mock_fcm_token_xyz_123",
      phoneModel: "Pixel 8",
      updatedAt: Date.now(),
    });

    alertId = await ctx.db.insert("alerts", {
      userId: user._id,
      deviceId: devId,
      type: "cpuTemp",
      value: 95,
      threshold: 85,
      createdAt: Date.now(),
      pushed: false,
    });
  });

  const details = await t.query(internal.alerts.getAlertPushDetails, { alertId });
  expect(details).not.toBeNull();
  expect(details?.deviceName).toBe("Desktop PC");
  expect(details?.tokens).toEqual(["mock_fcm_token_xyz_123"]);
  expect(details?.alert.type).toBe("cpuTemp");
  expect(details?.alert.value).toBe(95);

  // Mark pushed
  await t.mutation(internal.alerts.markAlertPushed, { alertId });
  const after = await t.query(internal.alerts.getAlertPushDetails, { alertId });
  expect(after?.alert.pushed).toBe(true);

  // Remove invalid token
  await t.mutation(internal.alerts.removeInvalidPushToken, { fcmToken: "mock_fcm_token_xyz_123" });
  const detailsAfterClean = await t.query(internal.alerts.getAlertPushDetails, { alertId });
  expect(detailsAfterClean?.tokens).toEqual([]);
});
