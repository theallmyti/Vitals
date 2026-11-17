// Push updates: the release workflow calls `publish` after the GitHub release is live, and every
// subscriber of `latest` hears about it at once (agents within a minute, the app on next open).
//
// publish is internal, so only someone holding the deploy key can call it:
//   npx convex run --prod releases:publish '{"agentVersion":"0.2.6", ...}'
// latest is public: a version number and a download link are not secrets.
import { internalMutation, query, type QueryCtx } from "./_generated/server";
import { v, ConvexError } from "convex/values";
import { LATEST_AGENT_VERSION, compareVersions } from "./versions";

// The app may only be pointed at an APK on this repo's releases (defence in depth: the
// download is also checked against apkSha256 on the phone).
export const APK_URL_PREFIX = "https://github.com/theallmyti/Vitals/releases/download/";
const VERSION = /^\d+\.\d+\.\d+$/;

async function row(ctx: QueryCtx) {
  return await ctx.db.query("releases").first();
}

/// The newest agent version: the published release, or the built-in one if that is newer
/// (a backend deployed with a higher LATEST_AGENT_VERSION before CI published).
export async function currentAgentVersion(ctx: QueryCtx) {
  const r = await row(ctx);
  if (r && compareVersions(r.agentVersion, LATEST_AGENT_VERSION) > 0) {
    return r.agentVersion;
  }
  return LATEST_AGENT_VERSION;
}

export const latest = query({
  args: {},
  handler: async (ctx) => {
    const r = await row(ctx);
    return {
      agentVersion: await currentAgentVersion(ctx),
      appVersion: r?.appVersion ?? null,
      appVersionCode: r?.appVersionCode ?? null,
      apkUrl: r?.apkUrl ?? null,
      apkSha256: r?.apkSha256 ?? null,
      publishedAt: r?.publishedAt ?? null,
    };
  },
});

export const publish = internalMutation({
  args: {
    agentVersion: v.string(),
    appVersion: v.optional(v.string()),
    appVersionCode: v.optional(v.number()),
    apkUrl: v.optional(v.string()),
    apkSha256: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    if (!VERSION.test(args.agentVersion)) {
      throw new ConvexError("agentVersion must be x.y.z");
    }
    const hasApp = args.appVersion !== undefined;
    if (hasApp) {
      if (!VERSION.test(args.appVersion!)) throw new ConvexError("appVersion must be x.y.z");
      if (!Number.isInteger(args.appVersionCode) || args.appVersionCode! < 1) {
        throw new ConvexError("appVersionCode must be a positive integer");
      }
      if (!args.apkUrl?.startsWith(APK_URL_PREFIX)) throw new ConvexError(`apkUrl must start with ${APK_URL_PREFIX}`);
      if (!/^[0-9a-f]{64}$/.test(args.apkSha256 ?? "")) throw new ConvexError("apkSha256 must be 64 lowercase hex");
    }
    const existing = await row(ctx);
    // Never roll the fleet backwards by accident (a re-run of an old tag's workflow).
    if (existing && compareVersions(args.agentVersion, existing.agentVersion) < 0) {
      throw new ConvexError(`agentVersion ${args.agentVersion} is older than published ${existing.agentVersion}`);
    }
    const next = {
      agentVersion: args.agentVersion,
      // An agent-only release keeps the last published app.
      appVersion: hasApp ? args.appVersion : existing?.appVersion,
      appVersionCode: hasApp ? args.appVersionCode : existing?.appVersionCode,
      apkUrl: hasApp ? args.apkUrl : existing?.apkUrl,
      apkSha256: hasApp ? args.apkSha256 : existing?.apkSha256,
      publishedAt: Date.now(),
    };
    if (existing) {
      await ctx.db.replace(existing._id, next);
    } else {
      await ctx.db.insert("releases", next);
    }
    return next;
  },
});
