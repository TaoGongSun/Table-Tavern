// @vitest-environment happy-dom
// 中國供應商：選那一列＝API 通道換成矽基流動 base_url，金鑰存自己的槽、檔位存 siliconflow:<檔位>；
// 切回 API 直連清掉 base_url，OpenRouter 金鑰原封不動。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

const patches = vi.hoisted(() => [] as Record<string, unknown>[]);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "update_config") {
      patches.push(args?.patch as Record<string, unknown>);
      return { api_keys: {}, tier_models: {}, preferences: {} };
    }
    if (cmd === "detect_clis") return [];
    if (cmd === "smart_free_recommendations") return { limited: [], stable: [], diversified: true };
    return null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { Settings } from "./SettingsForm";
import { SILICONFLOW_BASE_URL } from "../ai-connection/siliconflow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mounted: { root: Root; host: HTMLDivElement }[] = [];

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i += 1) await Promise.resolve();
  });
}

async function mount(preferences: Record<string, unknown>) {
  const config: AppConfig = {
    api_keys: { openrouter: "sk-or-keep" },
    tier_models: {},
    preferences: { transport: "api", api_model_mode: "manual", language: "zh-TW", ...preferences },
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  mounted.push({ root, host });
  act(() =>
    root.render(
      <Settings config={config} onSaved={() => {}} onDirty={() => {}} onBack={() => {}} onBlockingChange={() => {}} />,
    ),
  );
  await flush();
  return host;
}

function radio(host: HTMLElement, label: string) {
  return Array.from(host.querySelectorAll("label")).find((node) => node.textContent?.startsWith(label))!
    .querySelector("input")!;
}

function input(host: HTMLElement, label: string) {
  return Array.from(host.querySelectorAll("label")).find((node) => node.textContent?.startsWith(label))
    ?.querySelector("input");
}

async function save(host: HTMLElement) {
  act(() => host.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
  await flush();
}

afterEach(() => {
  for (const { root, host } of mounted.splice(0)) {
    act(() => root.unmount());
    host.remove();
  }
  patches.length = 0;
});

describe("中國供應商（矽基流動）", () => {
  it("選了就換 base_url、金鑰存 siliconflow 槽，OpenRouter 欄與 base URL 欄藏起來", async () => {
    const host = await mount({});
    act(() => radio(host, t("transportCn")).click());
    await flush();
    expect(input(host, t("apiKeyLabel"))).toBeUndefined();
    expect(input(host, t("baseUrlLabel"))).toBeUndefined();
    expect(host.textContent).toContain(t("cnRiskNote"));
    const key = input(host, t("cnKeyLabel"))!;
    act(() => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(key, "sk-sf");
      key.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await save(host);
    expect(patches[0]).toMatchObject({
      preferences: { base_url: SILICONFLOW_BASE_URL },
      api_keys: { siliconflow: "sk-sf" },
    });
    expect((patches[0].api_keys as Record<string, string>).openrouter).toBeUndefined();
  });

  it("從矽基流動切回 API 直連會清掉 base_url", async () => {
    const host = await mount({ base_url: SILICONFLOW_BASE_URL });
    expect(radio(host, t("transportCn")).checked).toBe(true);
    act(() => radio(host, t("transportApi")).click());
    await flush();
    await save(host);
    expect(patches[0]).toMatchObject({ preferences: { base_url: "" } });
  });
});
