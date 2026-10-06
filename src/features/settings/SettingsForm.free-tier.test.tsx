// @vitest-environment happy-dom
// 免費層 key 不顯示生圖模型選單：只有確定 free 才藏；查詢中、paid、unknown、失敗都照常顯示。
// 藏著時草稿保留，但不算未儲存、不寫進存檔。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

interface Gate {
  apiKey: string;
  resolve: (tier: string) => void;
  reject: () => void;
}

const backend = vi.hoisted(() => ({
  gates: [] as Gate[],
  patches: [] as Record<string, unknown>[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    switch (cmd) {
      case "openrouter_key_tier":
        return new Promise((resolve, reject) =>
          backend.gates.push({
            apiKey: String(args?.apiKey),
            resolve: (tier) => resolve({ tier, base: "http://saved" }),
            reject: () => reject(new Error("offline")),
          }),
        );
      case "update_config": {
        const patch = args?.patch as Record<string, unknown>;
        backend.patches.push(patch);
        return { api_keys: {}, tier_models: {}, preferences: {} };
      }
      case "detect_clis":
        return [];
      case "smart_free_recommendations":
        return { limited: [], stable: [], diversified: true };
      default:
        return null;
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { Settings } from "./SettingsForm";
import { resetKeyTierCacheForTest } from "./key-tier";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function config(over: { key?: string; base?: string; transport?: string; imageModel?: string } = {}): AppConfig {
  const preferences: Record<string, unknown> = {
    transport: over.transport ?? "api",
    api_model_mode: "manual",
    language: "zh-TW",
    image_model: over.imageModel ?? "a/saved-image",
  };
  if (over.base !== undefined) preferences.base_url = over.base;
  return { api_keys: { openrouter: over.key ?? "sk-or-free" }, tier_models: {}, preferences };
}

async function flush() {
  for (let i = 0; i < 10; i += 1) await Promise.resolve();
}

async function tick(ms = 0) {
  await act(async () => {
    vi.advanceTimersByTime(ms);
    await flush();
  });
}

function labelled(host: HTMLElement, label: string) {
  return Array.from(host.querySelectorAll("label")).find((node) =>
    (node.textContent ?? "").startsWith(label),
  );
}

const imageField = (host: HTMLElement) => labelled(host, t("imageModelLabel"));

function setValue(element: HTMLInputElement | HTMLSelectElement, value: string) {
  const proto = element instanceof HTMLSelectElement ? HTMLSelectElement.prototype : HTMLInputElement.prototype;
  Object.getOwnPropertyDescriptor(proto, "value")!.set!.call(element, value);
  element.dispatchEvent(new Event(element instanceof HTMLSelectElement ? "change" : "input", { bubbles: true }));
}

describe("免費層不顯示生圖模型選單", () => {
  const mounted: { root: Root; host: HTMLDivElement }[] = [];
  let dirty = 0;

  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    backend.gates.length = 0;
    backend.patches.length = 0;
    resetKeyTierCacheForTest();
    dirty = 0;
  });

  afterEach(() => {
    for (const { root, host } of mounted.splice(0)) {
      act(() => root.unmount());
      host.remove();
    }
    vi.useRealTimers();
  });

  async function render(cfg: AppConfig) {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    const page = { root, host };
    mounted.push(page);
    await act(async () => {
      root.render(
        <Settings
          config={cfg}
          onSaved={() => {}}
          onDirty={(count) => (dirty = count)}
          onBack={() => {}}
          onBlockingChange={() => {}}
        />,
      );
      await flush();
    });
    return page;
  }

  async function rerender(page: { root: Root }, cfg: AppConfig) {
    await act(async () => {
      page.root.render(
        <Settings
          config={cfg}
          onSaved={() => {}}
          onDirty={(count) => (dirty = count)}
          onBack={() => {}}
          onBlockingChange={() => {}}
        />,
      );
      await flush();
    });
  }

  async function close(page: { root: Root; host: HTMLDivElement }) {
    act(() => page.root.unmount());
    page.host.remove();
    mounted.splice(mounted.indexOf(page as never), 1);
  }

  async function answer(gate: Gate, tier: string) {
    await act(async () => {
      gate.resolve(tier);
      await flush();
    });
  }

  it("確定 free 才藏；查詢中、paid、unknown、失敗都顯示", async () => {
    for (const outcome of ["free", "paid", "unknown", "reject"]) {
      resetKeyTierCacheForTest();
      backend.gates.length = 0;
      const page = await render(config());
      expect(imageField(page.host)).toBeDefined();
      await tick();
      expect(backend.gates.map((gate) => gate.apiKey)).toEqual(["sk-or-free"]);
      expect(imageField(page.host)).toBeDefined();
      if (outcome === "reject") {
        await act(async () => {
          backend.gates[0].reject();
          await flush();
        });
      } else {
        await answer(backend.gates[0], outcome);
      }
      expect(Boolean(imageField(page.host)), outcome).toBe(outcome !== "free");
      await close(page);
    }
  });

  it("CLI 傳輸、沒填 key 都不查", async () => {
    await render(config({ transport: "claude" }));
    await render(config({ key: "" }));
    await tick(1000);
    expect(backend.gates).toHaveLength(0);
  });

  it("改草稿 key：立刻停用舊結果、500ms 後才重查、晚回的舊結果丟掉", async () => {
    const page = await render(config());
    await tick();
    const first = backend.gates[0];
    const keyInput = labelled(page.host, t("apiKeyLabel"))!.querySelector("input")!;
    await act(async () => setValue(keyInput, "sk-or-new"));
    await tick(499);
    expect(backend.gates).toHaveLength(1);
    await tick(1);
    expect(backend.gates.map((gate) => gate.apiKey)).toEqual(["sk-or-free", "sk-or-new"]);
    await answer(backend.gates[1], "paid");
    await answer(first, "free");
    expect(imageField(page.host)).toBeDefined();
    // 已是 free 時換 key：不等 debounce 就恢復顯示
    await act(async () => setValue(keyInput, "sk-or-third"));
    await tick(500);
    await answer(backend.gates[2], "free");
    expect(imageField(page.host)).toBeUndefined();
    await act(async () => setValue(keyInput, "sk-or-fourth"));
    expect(imageField(page.host)).toBeDefined();
  });

  it("改草稿 base URL 不重查", async () => {
    const page = await render(config());
    await tick();
    const baseInput = labelled(page.host, t("baseUrlLabel"))!.querySelector("input")!;
    await act(async () => setValue(baseInput, "https://half"));
    await tick(1000);
    expect(backend.gates).toHaveLength(1);
  });

  it("快取：key 與存檔 base 相符就首屏直接藏，背景照樣重查；存檔 base 一變立即作廢重查", async () => {
    const page = await render(config({ base: "http://saved" }));
    await tick();
    await answer(backend.gates[0], "free");
    await close(page);

    const again = await render(config({ base: "http://saved" }));
    expect(imageField(again.host)).toBeUndefined();
    await tick();
    expect(backend.gates).toHaveLength(2);

    await rerender(again, config({ base: "http://other" }));
    expect(imageField(again.host)).toBeDefined();
    await tick();
    expect(backend.gates).toHaveLength(3);
    await close(again);

    const otherKey = await render(config({ key: "sk-or-other", base: "http://saved" }));
    expect(imageField(otherKey.host)).toBeDefined();
  });

  it("舊頁關閉 → 新頁拿到 paid → 舊頁的 free 才晚回：不影響畫面也不寫進快取", async () => {
    const old = await render(config());
    await tick();
    const late = backend.gates[0];
    await close(old);
    const fresh = await render(config());
    await tick();
    await answer(backend.gates[1], "paid");
    await answer(late, "free");
    expect(imageField(fresh.host)).toBeDefined();
    await close(fresh);
    const third = await render(config());
    expect(imageField(third.host)).toBeDefined();
  });

  it("同一把 key 儲值後：快取 free，重查 paid 就出現", async () => {
    const page = await render(config());
    await tick();
    await answer(backend.gates[0], "free");
    await close(page);
    const again = await render(config());
    expect(imageField(again.host)).toBeUndefined();
    await tick();
    await answer(backend.gates[1], "paid");
    expect(imageField(again.host)).toBeDefined();
  });

  it("快取 free，重查 unknown 或失敗都恢復顯示", async () => {
    for (const outcome of ["unknown", "reject"]) {
      resetKeyTierCacheForTest();
      backend.gates.length = 0;
      const page = await render(config());
      await tick();
      await answer(backend.gates[0], "free");
      await close(page);
      const again = await render(config());
      expect(imageField(again.host)).toBeUndefined();
      await tick();
      if (outcome === "reject") {
        await act(async () => {
          backend.gates[1].reject();
          await flush();
        });
      } else {
        await answer(backend.gates[1], outcome);
      }
      expect(imageField(again.host), outcome).toBeDefined();
      await close(again);
    }
  });

  it("查詢中改了生圖模型、接著回 free 被藏：存其他設定不寫 image_model，草稿留到欄位再出現", async () => {
    const page = await render(config());
    await tick();
    const select = imageField(page.host)!.querySelector("select")!;
    await act(async () => setValue(select, ""));
    expect(dirty).toBe(1);
    await answer(backend.gates[0], "free");
    expect(imageField(page.host)).toBeUndefined();
    expect(dirty).toBe(0);
    const round = labelled(page.host, t("maxRoundLabel"))!.querySelector("input")!;
    await act(async () => setValue(round, "5"));
    expect(dirty).toBe(1);
    const form = page.host.querySelector("form")!;
    await act(async () => {
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      await flush();
    });
    expect(backend.patches).toHaveLength(1);
    const prefs = backend.patches[0].preferences as Record<string, unknown>;
    expect(prefs.max_round_speakers).toBe(5);
    expect(prefs).not.toHaveProperty("image_model");

    // 換成付費 key 後欄位回來，剛才的草稿還在且重新算未儲存
    const keyInput = labelled(page.host, t("apiKeyLabel"))!.querySelector("input")!;
    await act(async () => setValue(keyInput, "sk-or-paid"));
    expect(imageField(page.host)!.querySelector("select")!.value).toBe("");
    expect(dirty).toBe(3);
  });
});
