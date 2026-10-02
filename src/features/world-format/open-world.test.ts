import { describe, expect, it } from "vitest";
import { gateOf, listBadge, looseTranscript, readOnlyBannerVersion, type OpenWorld } from "./open-world";

const readOnly = (app: string | null, format: number | null, backup = false): OpenWorld => ({
  status: "read_only",
  app_version: app,
  format_version: format,
  backup_available: backup,
});

describe("gateOf", () => {
  it("可玩、轉換完成、唯讀、修復、忙碌各走各的", () => {
    expect(gateOf({ status: "ready" })).toBe("play");
    expect(gateOf({ status: "migrated", from: 1, to: 2 })).toBe("play");
    expect(gateOf(readOnly("0.2.0", 2))).toBe("readonly");
    expect(gateOf({ status: "needs_repair", message: "壞了", directory: "/tmp/w" })).toBe("repair");
    expect(gateOf({ status: "busy" })).toBe("busy");
  });
});

describe("readOnlyBannerVersion", () => {
  it("有 app_version 就用它，空白與缺值省略，格式數字不算版本", () => {
    expect(readOnlyBannerVersion({ app_version: " 0.9.0 ", format_version: 4 })).toBe("0.9.0");
    expect(readOnlyBannerVersion({ app_version: null, format_version: 4 })).toBeNull();
    expect(readOnlyBannerVersion({ app_version: "   ", format_version: 4 })).toBeNull();
    expect(readOnlyBannerVersion({ format_version: 9 })).toBeNull();
  });
});

describe("looseTranscript", () => {
  it("認得出的 kind 保留，其餘當成系統訊息，不補 id 與時間", () => {
    const events = looseTranscript([
      { speaker_name: "艾", text: "你好", kind: "dialogue" },
      { speaker_name: "", text: "壞行", kind: "aside" },
    ]);
    expect(events).toEqual([
      { ts: "", speaker_id: "", speaker_name: "艾", kind: "dialogue", text: "你好" },
      { ts: "", speaker_id: "", speaker_name: "", kind: "system", text: "壞行" },
    ]);
  });
});

describe("listBadge", () => {
  it("需要修復優先於唯讀", () => {
    expect(listBadge({ read_only: true, needs_repair: true })).toBe("repair");
    expect(listBadge({ read_only: true, needs_repair: false })).toBe("readonly");
    expect(listBadge({ read_only: false, needs_repair: false })).toBeNull();
  });
});
