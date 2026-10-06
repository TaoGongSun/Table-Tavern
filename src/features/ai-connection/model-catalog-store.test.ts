// @vitest-environment happy-dom
// 模型清單快取：背景重抓失敗時留住上次的清單，重開 app 也還在。

import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  cached: {} as Record<string, unknown>,
  writes: 0,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    if (command === "read_model_catalog") return backend.cached;
    if (command === "write_model_catalog") {
      backend.writes += 1;
      backend.cached = args.catalog as Record<string, unknown>;
      return null;
    }
    if (command === "list_cli_models") return [];
    return null;
  }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const IMAGE_LIST = [{ id: "google/gemini-3.1-flash-image", label: "Gemini Flash Image" }];

/** 每次重新載入模組＝重開 app（模組級的清單與預熱旗標歸零） */
async function launch() {
  vi.resetModules();
  const store = await import("./model-catalog-store");
  await store.prefetchModelCatalogs();
  let snapshot: Record<string, unknown> = {};
  function Probe() {
    snapshot = store.useModelCatalogs();
    return null;
  }
  const root = createRoot(document.createElement("div"));
  await act(async () => root.render(createElement(Probe)));
  act(() => root.unmount());
  return snapshot;
}

beforeEach(() => {
  backend.cached = { "api-image": IMAGE_LIST };
  backend.writes = 0;
  vi.stubGlobal("fetch", vi.fn(async () => Promise.reject(new Error("offline"))));
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("model-catalog-store", () => {
  it("已有快取時背景抓取失敗：清單仍在、快取檔不被覆寫，重開後也還在", async () => {
    expect((await launch())["api-image"]).toEqual(IMAGE_LIST);
    expect(backend.writes).toBe(0);
    expect((await launch())["api-image"]).toEqual(IMAGE_LIST);
  });
});
