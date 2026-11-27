"use node";
import { internalAction, action } from "./_generated/server";
import { internal } from "./_generated/api";
import { v } from "convex/values";
import crypto from "crypto";

// Cached Google OAuth2 token to avoid re-authenticating on every alert
let cachedToken: { token: string; expiresAt: number } | null = null;

function getServiceAccount() {
  const raw = process.env.FCM_SERVICE_ACCOUNT_JSON;
  if (!raw) return null;
  try {
    const text = raw.trim().startsWith("{") ? raw : Buffer.from(raw, "base64").toString("utf-8");
    return JSON.parse(text);
  } catch (e) {
    console.error("Failed to parse FCM_SERVICE_ACCOUNT_JSON:", e);
    return null;
  }
}

async function getAccessToken(sa: any): Promise<string> {
  const now = Math.floor(Date.now() / 1000);
  if (cachedToken && cachedToken.expiresAt > now + 60) {
    return cachedToken.token;
  }

  const header = { alg: "RS256", typ: "JWT" };
  const payload = {
    iss: sa.client_email,
    sub: sa.client_email,
    aud: "https://oauth2.googleapis.com/token",
    iat: now,
    exp: now + 3600,
    scope: "https://www.googleapis.com/auth/firebase.messaging",
  };

  const encode = (obj: any) => Buffer.from(JSON.stringify(obj)).toString("base64url");
  const unsignedToken = `${encode(header)}.${encode(payload)}`;

  const signer = crypto.createSign("RSA-SHA256");
  signer.update(unsignedToken);
  signer.end();
  const signature = signer.sign(sa.private_key, "base64url");

  const jwt = `${unsignedToken}.${signature}`;

  const resp = await fetch("https://oauth2.googleapis.com/token", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion=${jwt}`,
  });

  if (!resp.ok) {
    const err = await resp.text();
    throw new Error(`Google OAuth2 error (${resp.status}): ${err}`);
  }

  const data = await resp.json();
  cachedToken = {
    token: data.access_token,
    expiresAt: now + (data.expires_in ?? 3600),
  };
  return cachedToken.token;
}

export async function sendFcmMessage(
  sa: any,
  accessToken: string,
  targetToken: string,
  title: string,
  body: string,
  dataPayload: Record<string, string>
): Promise<{ ok: boolean; status: number; unregistered?: boolean }> {
  const endpoint = `https://fcm.googleapis.com/v1/projects/${sa.project_id}/messages:send`;
  const message = {
    message: {
      token: targetToken,
      notification: {
        title,
        body,
      },
      data: {
        ...dataPayload,
        title,
        message: body,
      },
      android: {
        priority: "high",
        notification: {
          channel_id: "pc_monitor_hardware_alerts",
          sound: "default",
          default_vibrate_timings: true,
        },
      },
    },
  };

  try {
    const res = await fetch(endpoint, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(message),
    });

    if (res.ok) {
      return { ok: true, status: res.status };
    }

    const errJson = (await res.json().catch(() => null)) as any;
    const errStatus = errJson?.error?.status || "";
    const isUnregistered = errStatus === "UNREGISTERED" || errStatus === "NOT_FOUND" || res.status === 404;

    console.warn(`FCM send failed (${res.status}):`, errJson?.error?.message ?? "Unknown error");
    return { ok: false, status: res.status, unregistered: isUnregistered };
  } catch (err) {
    console.error("FCM network error:", err);
    return { ok: false, status: 0 };
  }
}

// Background action to dispatch FCM push notification for a triggered alert
export const dispatchAlertPush = internalAction({
  args: { alertId: v.id("alerts") },
  handler: async (ctx, args) => {
    const sa = getServiceAccount();
    if (!sa) {
      console.warn("FCM push skipped: FCM_SERVICE_ACCOUNT_JSON is not configured in Convex environment");
      return;
    }

    const details = await ctx.runQuery(internal.alerts.getAlertPushDetails, { alertId: args.alertId });
    if (!details || details.tokens.length === 0) {
      return;
    }

    const alert = details.alert;
    const deviceName = details.deviceName;

    let title = "Vitals Alert";
    let body = "Threshold exceeded on your computer";

    switch (alert.type) {
      case "cpuTemp":
        title = `${deviceName} · CPU Temperature High`;
        body = `CPU reached ${Math.round(alert.value ?? 0)}°C (limit: ${Math.round(alert.threshold ?? 0)}°C)`;
        break;
      case "gpuTemp":
        title = `${deviceName} · GPU Temperature High`;
        body = `GPU reached ${Math.round(alert.value ?? 0)}°C (limit: ${Math.round(alert.threshold ?? 0)}°C)`;
        break;
      case "ram":
        title = `${deviceName} · RAM Usage High`;
        body = `Memory use reached ${Math.round(alert.value ?? 0)}% (limit: ${Math.round(alert.threshold ?? 0)}%)`;
        break;
      case "disk":
        title = `${deviceName} · Low Disk Space (${alert.drive ?? "Drive"})`;
        body = `Only ${Math.round(alert.value ?? 0)}% free. You get this alert when free space is below ${Math.round(alert.threshold ?? 0)}% (change it in Alert settings).`;
        break;
    }

    let accessToken: string;
    try {
      accessToken = await getAccessToken(sa);
    } catch (e) {
      console.error("Failed to acquire FCM OAuth2 token:", e);
      return;
    }

    const dataPayload = {
      deviceId: String(alert.deviceId),
      type: alert.type,
      value: String(alert.value ?? 0),
    };

    let anySent = false;
    for (const fcmToken of details.tokens) {
      const res = await sendFcmMessage(sa, accessToken, fcmToken, title, body, dataPayload);
      if (res.ok) {
        anySent = true;
      } else if (res.unregistered) {
        // Token was uninstalled or expired, clean up database row
        await ctx.runMutation(internal.alerts.removeInvalidPushToken, { fcmToken });
      }
    }

    if (anySent) {
      await ctx.runMutation(internal.alerts.markAlertPushed, { alertId: args.alertId });
    }
  },
});

// Test action to send a ping push notification to the user's phone right now
export const testPushToAll = action({
  args: {},
  handler: async (ctx) => {
    const sa = getServiceAccount();
    if (!sa) {
      throw new Error("FCM_SERVICE_ACCOUNT_JSON is not configured in Convex environment");
    }

    const allTokens = await ctx.runQuery(internal.alerts.getAllPushTokens, {});
    if (!allTokens || allTokens.length === 0) {
      return { success: false, message: "No registered phone tokens found in database. Make sure you open the app once while logged in." };
    }

    const accessToken = await getAccessToken(sa);
    const title = "Vitals · Hardware Alert Test";
    const body = `FCM Push is live! Background delivery test passed at ${new Date().toLocaleTimeString()}.`;
    const dataPayload = {
      type: "test",
      deviceId: "test-device",
    };

    let delivered = 0;
    for (const item of allTokens) {
      const res = await sendFcmMessage(sa, accessToken, item.fcmToken, title, body, dataPayload);
      if (res.ok) delivered++;
      else if (res.unregistered) {
        await ctx.runMutation(internal.alerts.removeInvalidPushToken, { fcmToken: item.fcmToken });
      }
    }

    return {
      success: delivered > 0,
      deliveredTo: delivered,
      totalDevices: allTokens.length,
      message: `Delivered test notification to ${delivered} of ${allTokens.length} device(s)!`,
    };
  },
});
export const sendTestPush = action({
  args: {
    token: v.string(), // user session token
  },
  handler: async (ctx, args) => {
    const sa = getServiceAccount();
    if (!sa) {
      throw new Error("FCM_SERVICE_ACCOUNT_JSON is not configured in Convex environment");
    }

    const userDetails = await ctx.runQuery(internal.alerts.getUserPushTokens, { sessionToken: args.token });
    if (!userDetails || userDetails.tokens.length === 0) {
      return { success: false, message: "No registered phone tokens found for your user account. Open the Vitals app once while signed in to register your device." };
    }

    const accessToken = await getAccessToken(sa);
    const title = "Vitals Push Test";
    const body = `Test alert delivered at ${new Date().toLocaleTimeString()}! Real-time alerts are now fully wired.`;
    const dataPayload = {
      type: "test",
      deviceId: "test-device",
    };

    let deliveredCount = 0;
    for (const fcmToken of userDetails.tokens) {
      const res = await sendFcmMessage(sa, accessToken, fcmToken, title, body, dataPayload);
      if (res.ok) deliveredCount++;
      else if (res.unregistered) {
        await ctx.runMutation(internal.alerts.removeInvalidPushToken, { fcmToken });
      }
    }

    return {
      success: deliveredCount > 0,
      deliveredTo: deliveredCount,
      totalDevices: userDetails.tokens.length,
      message: deliveredCount > 0 ? `Sent to ${deliveredCount} phone(s)!` : "Delivery failed to all registered tokens",
    };
  },
});
