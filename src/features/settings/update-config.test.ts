import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AppConfig } from "../../shared/contracts/backend-contracts";
import { updateConfig } from "./update-config";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function snapshot(caller: string): AppConfig {
  return { api_keys: {}, tier_models: {}, preferences: { caller } };
}

function gate(caller: string) {
  let open: () => void = () => {};
  const ready = new Promise<AppConfig>((resolve) => {
    open = () => resolve(snapshot(caller));
  });
  return { ready, open };
}

function callerOf(args: unknown): string {
  const patch = (args as { patch?: { caller?: string } } | undefined)?.patch;
  return String(patch?.caller ?? "");
}

describe("updateConfig", () => {
  it("兩個呼叫端交錯、後端故意逆序完成時，invoke 與 resolve 都照呼叫順序", async () => {
    const release = { preference: gate("preference"), settings: gate("settings") };
    const started: string[] = [];
    vi.mocked(invoke).mockImplementation((_cmd, args) => {
      const caller = callerOf(args);
      started.push(caller);
      return release[caller as "preference" | "settings"].ready;
    });

    const resolved: string[] = [];
    // 偏好 controller 先寫，設定頁隨後存檔。兩邊都不互相等待。
    const preferenceWrite = updateConfig({ caller: "preference" }).then((saved) => {
      resolved.push(String(saved.preferences.caller));
    });
    const settingsSave = updateConfig({ caller: "settings" }).then((saved) => {
      resolved.push(String(saved.preferences.caller));
    });

    try {
      await Promise.resolve();
      expect(started).toEqual(["preference"]);

      // 設定頁的後端若已能完成，也不准先送出。
      release.settings.open();
      await Promise.resolve();
      expect(started).toEqual(["preference"]);
      expect(resolved).toEqual([]);

      release.preference.open();
      await preferenceWrite;
      await settingsSave;
      expect(started).toEqual(["preference", "settings"]);
      expect(resolved).toEqual(["preference", "settings"]);
      expect(vi.mocked(invoke).mock.calls.map((call) => call[0])).toEqual([
        "update_config",
        "update_config",
      ]);
    } finally {
      release.preference.open();
      release.settings.open();
      await Promise.allSettled([preferenceWrite, settingsSave]);
    }
  });

  it("前一筆失敗，下一筆仍送出", async () => {
    const started: string[] = [];
    vi.mocked(invoke).mockImplementation((_cmd, args) => {
      const caller = callerOf(args);
      started.push(caller);
      if (caller === "preference") return Promise.reject(new Error("preference 失敗"));
      return Promise.resolve(snapshot(caller));
    });

    const settled: string[] = [];
    const preferenceWrite = updateConfig({ caller: "preference" }).then(
      () => settled.push("preference:ok"),
      () => settled.push("preference:fail"),
    );
    const settingsSave = updateConfig({ caller: "settings" }).then(
      () => settled.push("settings:ok"),
      () => settled.push("settings:fail"),
    );

    await preferenceWrite;
    await settingsSave;
    expect(started).toEqual(["preference", "settings"]);
    expect(settled).toEqual(["preference:fail", "settings:ok"]);
  });
});
