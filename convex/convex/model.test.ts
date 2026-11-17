import { expect, test } from "vitest";
import { cleanText, deviceOs, isDeviceOnline } from "./model";

test("cleanText drops control characters, trims and cuts to the limit", () => {
  expect(cleanText("\u0007 ab \n", 1)).toBe("a");
  expect(cleanText(" x ")).toBe("x");
});

test("deviceOs uses the stored OS, then the capabilities, then linux", () => {
  expect(deviceOs({ os: "macos" } as any)).toBe("macos");
  expect(deviceOs({ capabilities: { windowsMode: true } } as any)).toBe("windows");
  expect(deviceOs({ capabilities: { isPi: true } } as any)).toBe("pi");
  expect(deviceOs({} as any)).toBe("linux");
});

test("a device is online for 3 minutes after its last sign of life, or while live data is under 45 s old", () => {
  const now = 1_700_000_000_000;
  expect(isDeviceOnline({ lastSeenAt: now - 2 * 60_000 } as any, null, now)).toBe(true);
  expect(isDeviceOnline({ lastSeenAt: now - 4 * 60_000 } as any, null, now)).toBe(false);
  expect(isDeviceOnline({ lastSeenAt: now - 4 * 60_000 } as any, { ts: now - 10_000 } as any, now)).toBe(true);
});
