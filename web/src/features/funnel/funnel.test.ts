import { describe, expect, it } from "vitest";
import { freeDailyFromBody } from "../openrouter/openrouter-api";
import { nextQuota, quotaBlocksSending, type QuotaState } from "./quota";
import { FALLBACK_RELEASE, fetchLatestRelease, parseRelease } from "./releases";

describe("quota state machine", () => {
  const unknown: QuotaState = { kind: "unknown" };

  it("follows /key and blocks sending only when exhausted", () => {
    const counted = nextQuota(unknown, { type: "key-info", daily: { kind: "counted", limit: 50, remaining: 3 } });
    expect(counted).toEqual({ kind: "counted", limit: 50, remaining: 3 });
    expect(quotaBlocksSending(counted)).toBe(false);
    const out = nextQuota(counted, { type: "key-info", daily: { kind: "counted", limit: 50, remaining: 0 } });
    expect(out).toEqual({ kind: "exhausted", limit: 50 });
    expect(quotaBlocksSending(out)).toBe(true);
  });

  it("marks exhausted from a daily-limit error and keeps it when /key is unreachable", () => {
    const counted: QuotaState = { kind: "counted", limit: 50, remaining: 2 };
    const out = nextQuota(counted, { type: "daily-exhausted-error" });
    expect(out).toEqual({ kind: "exhausted", limit: 50 });
    expect(nextQuota(out, { type: "key-info", daily: null })).toBe(out);
  });

  it("treats accounts without a daily counter as unlimited and resets on logout", () => {
    expect(nextQuota(unknown, { type: "key-info", daily: { kind: "unlimited" } })).toEqual({ kind: "unlimited" });
    expect(nextQuota({ kind: "exhausted", limit: 50 }, { type: "logout" })).toEqual(unknown);
  });

  it("parses /key bodies in all the shapes the desktop app accepts", () => {
    expect(freeDailyFromBody({ data: { free_model_daily_requests: { limit: 50, remaining: 47 } } })).toEqual({
      kind: "counted",
      limit: 50,
      remaining: 47,
    });
    expect(freeDailyFromBody({ data: { free_model_daily_requests: { limit: 50, used: 12 } } })).toEqual({
      kind: "counted",
      limit: 50,
      remaining: 38,
    });
    expect(freeDailyFromBody({ data: {} })).toEqual({ kind: "unlimited" });
    expect(freeDailyFromBody({ data: { free_model_daily_requests: "17" } })).toBeNull();
  });
});

describe("desktop release lookup", () => {
  it("falls back to the releases page when there is no release yet (404)", async () => {
    const fetch404 = (async () => new Response("{}", { status: 404 })) as typeof fetch;
    expect(await fetchLatestRelease(fetch404)).toEqual(FALLBACK_RELEASE);
  });

  it("picks the fixed asset names for each platform", () => {
    const info = parseRelease({
      tag_name: "v0.3.0",
      html_url: "https://github.com/TaoGongSun/Table-Tavern/releases/tag/v0.3.0",
      assets: [
        { name: "TableTavern_0.3.0_x64-setup.exe", browser_download_url: "https://github.com/x/win.exe" },
        { name: "TableTavern_0.3.0_aarch64.dmg", browser_download_url: "https://github.com/x/mac.dmg" },
        { name: "latest.json", browser_download_url: "https://github.com/x/latest.json" },
      ],
    });
    expect(info).toEqual({
      version: "0.3.0",
      pageUrl: "https://github.com/TaoGongSun/Table-Tavern/releases/tag/v0.3.0",
      windows: "https://github.com/x/win.exe",
      mac: "https://github.com/x/mac.dmg",
    });
  });

  it("refuses download links that do not point at github.com", () => {
    const info = parseRelease({ tag_name: "v1", html_url: "https://evil.example/", assets: [{ name: "a_x64-setup.exe", browser_download_url: "https://evil.example/a.exe" }] });
    expect(info.pageUrl).toBe(FALLBACK_RELEASE.pageUrl);
    expect(info.windows).toBeNull();
  });
});
