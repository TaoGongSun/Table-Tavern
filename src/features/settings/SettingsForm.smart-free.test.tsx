// @vitest-environment happy-dom
// 穩定免費區：最多 4 支（可能少列）、「目前使用」標在實際在用的那支、降級說明、第 2–4 名的備援說明。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

const backend = vi.hoisted(() => ({
  stable: [] as { model: string; label: string }[],
  diversified: true,
  current: "",
  gated: false,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    switch (cmd) {
      case "detect_clis":
        return [];
      case "smart_free_recommendations":
        return {
          limited: [],
          stable: backend.stable.map((row) => ({
            ...row,
            expiresAt: null,
            showExpiryDate: false,
            expiringSoon: false,
            reason: "stable_available",
            provider: "x",
            recent: false,
            contextLength: 32000,
            longContext: false,
          })),
          diversified: backend.diversified,
        };
      case "smart_free_status":
        if (backend.gated) {
          return new Promise((resolve) =>
            bus.statusGates.push((model) => resolve({ model, freeDaily: null })),
          );
        }
        return { model: backend.current, freeDaily: null };
      default:
        return null;
    }
  }),
}));
const bus = vi.hoisted(() => ({
  handlers: new Map<string, Set<() => void>>(),
  statusGates: [] as ((model: string) => void)[],
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: () => void) => {
    const set = bus.handlers.get(name) ?? new Set();
    set.add(handler);
    bus.handlers.set(name, set);
    return () => set.delete(handler);
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { Settings } from "./SettingsForm";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const ROWS = ["a/one:free", "b/two:free", "c/three:free", "d/four:free"].map((model, index) => ({
  model,
  label: `模型${index + 1}`,
}));

function config(mode: string, best?: string): AppConfig {
  return {
    api_keys: { openrouter: "sk-or-test" },
    tier_models: best ? { best, balanced: best, fast: best } : {},
    preferences: { transport: "api", api_model_mode: mode, language: "zh-TW" },
  };
}

describe("設定頁穩定免費區", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  beforeEach(() => {
    backend.stable = ROWS;
    backend.diversified = true;
    backend.current = "";
    backend.gated = false;
    bus.handlers.clear();
    bus.statusGates.length = 0;
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
  });

  async function render(cfg: AppConfig) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => {
      root!.render(
        <Settings config={cfg} onSaved={() => {}} onDirty={() => {}} onBack={() => {}} onBlockingChange={() => {}} />,
      );
      for (let i = 0; i < 10; i += 1) await Promise.resolve();
    });
    return host;
  }

  const rows = (h: HTMLElement) => Array.from(h.querySelectorAll("label.smart-free-recommendation"));

  it("列到 4 支；第 2–4 名各有「第一名擁擠時自動改用」，第 1 名沒有", async () => {
    const h = await render(config("stable_free"));
    const list = rows(h);
    expect(list.map((row) => row.querySelector("strong")?.textContent)).toEqual(["模型1", "模型2", "模型3", "模型4"]);
    const hint = t("smartFreeBackupHint");
    expect(list.map((row) => (row.textContent ?? "").includes(hint))).toEqual([false, true, true, true]);
  });

  it("名單少於 4 支就只列有的，不補空列", async () => {
    backend.stable = ROWS.slice(0, 2);
    const h = await render(config("stable_free"));
    expect(rows(h)).toHaveLength(2);
  });

  it("穩定免費模式：「目前使用」只標在後端回報的目前模型那一列，沒有「預測」字樣", async () => {
    backend.current = "c/three:free";
    const h = await render(config("stable_free"));
    const badge = t("smartFreeCurrentLabel");
    expect(rows(h).map((row) => (row.textContent ?? "").includes(badge))).toEqual([false, false, true, false]);
    expect(h.textContent).not.toContain("預測");
  });

  it("推薦（固定）模式：「目前使用」標在固定的那支，不看後端的穩定目前模型", async () => {
    backend.current = "a/one:free";
    const h = await render(config("recommended", "b/two:free"));
    const badge = t("smartFreeCurrentLabel");
    expect(rows(h).map((row) => (row.textContent ?? "").includes(badge))).toEqual([false, true, false, false]);
  });

  it("diversified=false 且有名單時顯示降級說明；diversified=true 或沒有名單不顯示", async () => {
    backend.diversified = false;
    let h = await render(config("stable_free"));
    expect(h.textContent).toContain(t("smartFreeNotDiversified"));
    act(() => root!.unmount());
    h.remove();
    root = null;

    backend.diversified = true;
    h = await render(config("stable_free"));
    expect(h.textContent).not.toContain(t("smartFreeNotDiversified"));
    act(() => root!.unmount());
    h.remove();
    root = null;

    backend.diversified = false;
    backend.stable = [];
    h = await render(config("stable_free"));
    expect(h.textContent).not.toContain(t("smartFreeNotDiversified"));
  });

  const fire = (name: string) =>
    act(async () => {
      bus.handlers.get(name)?.forEach((handler) => handler());
      for (let i = 0; i < 10; i += 1) await Promise.resolve();
    });

  it("設定頁開著時換模事件會重查狀態，「目前使用」跟著換", async () => {
    backend.current = "a/one:free";
    const h = await render(config("stable_free"));
    const badge = t("smartFreeCurrentLabel");
    const marks = () => rows(h).map((row) => (row.textContent ?? "").includes(badge));
    expect(marks()).toEqual([true, false, false, false]);
    backend.current = "b/two:free";
    await fire("smart-free-failover");
    expect(marks()).toEqual([false, true, false, false]);
    backend.current = "c/three:free";
    await fire("smart-free-model-switched");
    expect(marks()).toEqual([false, false, true, false]);
  });

  it("舊的狀態查詢晚回來，不蓋掉新的結果", async () => {
    backend.gated = true;
    const h = await render(config("stable_free"));
    await fire("smart-free-failover");
    expect(bus.statusGates).toHaveLength(2);
    const [older, newer] = bus.statusGates;
    await act(async () => {
      newer("b/two:free");
      for (let i = 0; i < 10; i += 1) await Promise.resolve();
    });
    await act(async () => {
      older("a/one:free");
      for (let i = 0; i < 10; i += 1) await Promise.resolve();
    });
    const badge = t("smartFreeCurrentLabel");
    expect(rows(h).map((row) => (row.textContent ?? "").includes(badge))).toEqual([false, true, false, false]);
  });
});
