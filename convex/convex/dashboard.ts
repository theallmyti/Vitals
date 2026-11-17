// Endpoints for the site (and the app): every call needs a signed-in session and only ever
// touches the caller's own devices (docs/Security.md §2, FR-13, FR-14, T-12).
import { mutation, query } from "./_generated/server";
import { v, ConvexError } from "convex/values";
import { LIMITS, isLimited, recordEvent } from "./rateLimit";
import { isOutdated } from "./versions";
import { currentAgentVersion } from "./releases";
import {
  viewableDevice,
  visibleDevicesOf,
  cleanText,
  isDeviceOnline,
  deviceOs,
  shareDevice,
  leaveDevice,
  deviceBySecretHash,
  ownedDevice,
  pairingByCode,
  pairingState,
  removeDeviceData,
  requireUser,
  requireUserAndTouch,
} from "./model";

// FR-33: the viewer sets now + 30 s and repeats every 15 s while a screen is open.
const VIEWER_WINDOW_MS = 30 * 1000;

export const overview = query({
  args: { token: v.string() },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx, args.token);
    const now = Date.now();
    const devices = await visibleDevicesOf(ctx, user._id);
    const latestList = await Promise.all(
      devices.map((d) =>
        ctx.db
          .query("latest")
          .withIndex("by_device", (q) => q.eq("deviceId", d._id))
          .unique(),
      ),
    );
    const latestAgent = await currentAgentVersion(ctx);
    const out = devices.map((d, i) => {
      const latest = latestList[i] ?? null;
      return {
        id: d._id,
        name: d.name,
        os: deviceOs(d),
        lastSeenAt: d.lastSeenAt,
        online: isDeviceOnline(d, latest, now),
        agentVersion: d.agentVersion ?? null,
        capabilities: d.capabilities ?? null,
        updateAvailable: isOutdated(d.agentVersion, latestAgent),
        latest,
        // Shared with this account by its owner (view only: rename/remove-for-all are owner-only).
        shared: d.userId !== user._id,
      };
    });
    return { now, latestAgentVersion: latestAgent, devices: out };
  },
});

// Per-minute history for one device from `sinceMs` (oldest first), optionally up to `untilMs`
// (exclusive). The site asks for whole past hours separately from the current hour: a query whose
// range ends before "now" is not re-run when a new summary arrives, so it is read once an hour
// instead of every minute (about 25x fewer rows read; Convex bills per row read).
export const history = query({
  args: { token: v.string(), deviceId: v.id("devices"), sinceMs: v.number(), untilMs: v.optional(v.number()) },
  handler: async (ctx, args) => {
    if (!Number.isFinite(args.sinceMs) || args.sinceMs < 0) {
      throw new ConvexError("invalid sinceMs");
    }
    if (args.untilMs !== undefined && (!Number.isFinite(args.untilMs) || args.untilMs < args.sinceMs)) {
      throw new ConvexError("invalid untilMs");
    }
    const user = await requireUser(ctx, args.token);
    await viewableDevice(ctx, user, args.deviceId);
    const { untilMs } = args;
    return await ctx.db
      .query("summaries")
      .withIndex("by_device_time", (q) => {
        const from = q.eq("deviceId", args.deviceId).gte("minuteTs", args.sinceMs);
        return untilMs === undefined ? from : from.lt("minuteTs", untilMs);
      })
      .collect();
  },
});

// Keep live data flowing for this device for the next 30 s (docs/Flow.md §4).
export const setViewer = mutation({
  args: { token: v.string(), deviceId: v.id("devices") },
  handler: async (ctx, args) => {
    const user = await requireUserAndTouch(ctx, args.token);
    await viewableDevice(ctx, user, args.deviceId);
    await ctx.db.patch(args.deviceId, { viewerUntil: Date.now() + VIEWER_WINDOW_MS });
  },
});

// The same for all of the user's devices in one call (the site uses this: one call per 15 s
// instead of one per device).
export const setViewerAll = mutation({
  args: { token: v.string() },
  handler: async (ctx, args) => {
    const user = await requireUserAndTouch(ctx, args.token);
    const until = Date.now() + VIEWER_WINDOW_MS;
    for (const d of await visibleDevicesOf(ctx, user._id)) {
      await ctx.db.patch(d._id, { viewerUntil: until });
    }
  },
});

// FR-14: rename a device.
export const renameDevice = mutation({
  args: { token: v.string(), deviceId: v.id("devices"), name: v.string() },
  handler: async (ctx, args) => {
    const user = await requireUserAndTouch(ctx, args.token);
    await ownedDevice(ctx, user, args.deviceId);
    const name = cleanText(args.name);
    if (name.length < 1 || name.length > 64) {
      throw new ConvexError("Use 1 to 64 characters for the name.");
    }
    await ctx.db.patch(args.deviceId, { name });
  },
});

// FR-14: remove a device. Its secret hash is deleted, so that computer can no longer write.
export const removeDevice = mutation({
  args: { token: v.string(), deviceId: v.id("devices") },
  handler: async (ctx, args) => {
    const user = await requireUserAndTouch(ctx, args.token);
    const device = await viewableDevice(ctx, user, args.deviceId);
    if (device.userId !== user._id) {
      await leaveDevice(ctx, device._id, user._id); // shared: only off this account
      return;
    }
    await removeDeviceData(ctx, device);
  },
});

// Pairing: show which computer is asking before confirming (docs/Security.md §2).
const TOO_MANY_WRONG = "Too many wrong codes. Wait 10 minutes and try again.";

// Looks up a pairing code before confirming, so the user sees which computer it is.
// A mutation, not a query, so wrong codes can be counted: 10 per 10 minutes per account stops
// guessing someone else's open code (docs/Security.md §2). Not found returns null (a thrown
// error would roll back the count).
export const checkPairingCode = mutation({
  args: { token: v.string(), code: v.string() },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx, args.token);
    const key = `wrongCode:${user._id}`;
    if (await isLimited(ctx, key, LIMITS.wrongCode)) {
      throw new ConvexError(TOO_MANY_WRONG);
    }
    const r = await pairingByCode(ctx, args.code);
    if (!r) {
      await recordEvent(ctx, key);
      return null;
    }
    return { name: r.name, os: r.os, state: pairingState(r, Date.now()) };
  },
});

const MAX_DEVICES_PER_USER = 50;

// The signed-in user who confirms becomes the device's owner (FR-11). A computer that is
// paired again (new owner, or a device from before accounts) moves to the confirming user.
export const confirmPairing = mutation({
  args: { token: v.string(), code: v.string() },
  handler: async (ctx, args) => {
    const user = await requireUserAndTouch(ctx, args.token);
    const key = `wrongCode:${user._id}`;
    if (await isLimited(ctx, key, LIMITS.wrongCode)) {
      throw new ConvexError(TOO_MANY_WRONG);
    }
    const r = await pairingByCode(ctx, args.code);
    if (!r) {
      await recordEvent(ctx, key);
      return null; // not found (counted as a wrong code)
    }
    if (pairingState(r, Date.now()) !== "pending") {
      throw new ConvexError("code not found or expired");
    }
    const now = Date.now();
    const existing = await deviceBySecretHash(ctx, r.secretHash);
    if (existing && existing.userId && existing.userId !== user._id) {
      // Already someone else's computer: share it with this account instead.
      await shareDevice(ctx, existing, user._id);
      await ctx.db.patch(r._id, { status: "confirmed", deviceId: existing._id });
      return { deviceId: existing._id, name: existing.name, os: existing.os ?? "windows", shared: true };
    }
    if (!existing) {
      const userDevices = await ctx.db
        .query("devices")
        .withIndex("by_user", (q) => q.eq("userId", user._id))
        .collect();
      if (userDevices.length >= MAX_DEVICES_PER_USER) {
        throw new ConvexError(`Device limit reached (max ${MAX_DEVICES_PER_USER} devices per account).`);
      }
    }
    let deviceId;
    const os = r.os ?? "windows";
    if (existing) {
      await ctx.db.patch(existing._id, { userId: user._id, isFake: false, os });
      deviceId = existing._id;
    } else {
      deviceId = await ctx.db.insert("devices", {
        userId: user._id,
        name: r.name,
        secretHash: r.secretHash,
        lastSeenAt: now,
        isFake: false,
        os,
      });
    }
    await ctx.db.patch(r._id, { status: "confirmed", deviceId });
    return { deviceId, name: r.name, os };
  },
});
