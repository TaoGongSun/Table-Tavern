// @vitest-environment happy-dom
// 狀態欄手改：存成功並重讀狀態樹後通知呼叫端（變數模式下表在逐字稿事件上，要重讀事件卡片介面才換得到新值）。
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { type TableStateController, useTableStateController } from "./useTableStateController";

const calls: string[] = [];
// 預設：讀狀態固定回金幣 9；競態測試換成受控 promise
const handler = vi.hoisted(() => ({ current: null as null | ((command: string) => Promise<unknown>) }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    calls.push(command);
    if (handler.current) return handler.current(command);
    if (command === "read_state") return { state: { table: {}, tree: { 玩家: { 金幣: "9" } } } };
    return [];
  }),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => (resolve = done));
  return { promise, resolve };
}

describe("useTableStateController 手改", () => {
  it("樹模式欄位存成功後依序：set_state_path → read_state → onEdited；值沒變不通知", async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    const onEdited = vi.fn(() => calls.push("onEdited"));
    let controller: TableStateController | null = null;
    function Probe() {
      controller = useTableStateController({ worldId: "w1", onError: () => {}, onEdited });
      return null;
    }
    const root = createRoot(document.createElement("div"));
    await act(async () => root.render(createElement(Probe)));
    calls.length = 0;
    await act(async () => {
      await controller!.save(["玩家", "金幣"], true, "9");
    });
    expect(calls.slice(0, 2)).toEqual(["set_state_path", "read_state"]);
    expect(calls).toContain("onEdited");
    expect(calls.indexOf("onEdited")).toBeGreaterThan(calls.indexOf("read_state"));
    expect(onEdited).toHaveBeenCalledTimes(1);
    await act(async () => root.unmount());
  });
});

describe("useTableStateController 數字欄顯示模板", () => {
  it("照模板輸入「第 8 天」存成數字 8〔作者裁決 2026-10-27〕", async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    let controller: TableStateController | null = null;
    function Probe() {
      controller = useTableStateController({ worldId: "w1", onError: () => {} });
      return null;
    }
    const root = createRoot(document.createElement("div"));
    await act(async () => root.render(createElement(Probe)));
    act(() => controller!.hydrate({ table: {}, tree: { 驻守天数: "7" } }, [], { 驻守天数: "第 {} 天" }));
    vi.mocked(invoke).mockClear();
    await act(async () => {
      await controller!.save(["驻守天数"], true, "第 8 天");
    });
    expect(vi.mocked(invoke).mock.calls[0]).toEqual([
      "set_state_path",
      { worldId: "w1", path: ["驻守天数"], value: "8" },
    ]);
    await act(async () => root.unmount());
  });
});

describe("useTableStateController 顯示前綴", () => {
  it("輸入帶前綴先剝掉、再照數字模板解析", async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    let controller: TableStateController | null = null;
    function Probe() {
      controller = useTableStateController({ worldId: "w1", onError: () => {} });
      return null;
    }
    const root = createRoot(document.createElement("div"));
    await act(async () => root.render(createElement(Probe)));
    // 每次存完會重讀（假後端回的狀態沒有前綴表），所以每筆存之前重新 hydrate
    const hydrate = () =>
      act(() =>
        controller!.hydrate(
          { table: {}, tree: { 驻守天数: "7", 人物: { 林远: { 身份: "驿丞" } } } },
          [],
          { 驻守天数: "第 {} 天" },
          {},
          { 驻守天数: "📅 ", "人物.*.身份": "🪪 " },
        ),
      );
    hydrate();
    vi.mocked(invoke).mockClear();
    await act(async () => {
      await controller!.save(["驻守天数"], true, "📅 第 8 天");
    });
    hydrate();
    await act(async () => {
      await controller!.save(["人物", "林远", "身份"], true, "🪪 新任驿丞");
    });
    const writes = vi.mocked(invoke).mock.calls.filter(([command]) => command === "set_state_path");
    expect(writes).toEqual([
      ["set_state_path", { worldId: "w1", path: ["驻守天数"], value: "8" }],
      ["set_state_path", { worldId: "w1", path: ["人物", "林远", "身份"], value: "新任驿丞" }],
    ]);
    await act(async () => root.unmount());
  });
});

describe("useTableStateController 重讀競態", () => {
  afterEach(() => {
    handler.current = null;
  });

  it("W1 讀到舊值卡在綁定清單、W2 在途期間要求：不另起一趟，W1 回來後補讀，最後停在新值", async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    const states = [deferred<unknown>(), deferred<unknown>()];
    const bindings = [deferred<unknown>(), deferred<unknown>()];
    let stateCalls = 0;
    let bindingCalls = 0;
    handler.current = (command) =>
      command === "read_state" ? states[stateCalls++].promise : bindings[bindingCalls++].promise;
    let controller: TableStateController | null = null;
    const shown: string[] = [];
    function Probe() {
      controller = useTableStateController({ worldId: "w1", onError: () => {} });
      const gold = (controller.tree as { 玩家?: { 金幣?: string } }).玩家?.金幣;
      if (gold && shown[shown.length - 1] !== gold) shown.push(gold);
      return null;
    }
    const root = createRoot(document.createElement("div"));
    await act(async () => root.render(createElement(Probe)));

    let first = false;
    let second = false;
    await act(async () => {
      void controller!.refresh().then(() => (first = true));
    });
    await act(async () => states[0].resolve({ state: { table: {}, tree: { 玩家: { 金幣: "1" } } } }));
    // W1 讀完狀態、卡在綁定清單；W2（卡片又寫了一筆）
    await act(async () => {
      void controller!.refresh().then(() => (second = true));
    });
    expect(stateCalls).toBe(1);
    await act(async () => bindings[0].resolve([]));
    // W1 的結果比畫面新（沒有更新的已套用），照套；接著補讀
    expect(stateCalls).toBe(2);
    expect(first || second).toBe(false);
    await act(async () => states[1].resolve({ state: { table: {}, tree: { 玩家: { 金幣: "2" } } } }));
    await act(async () => bindings[1].resolve([]));
    expect(first && second).toBe(true);
    expect(shown).toEqual(["1", "2"]);
    await act(async () => root.unmount());
  });

  it("換桌（hydrate）後，舊世代在途的重讀晚回不蓋掉新桌的值", async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    const state = deferred<unknown>();
    handler.current = (command) => (command === "read_state" ? state.promise : Promise.resolve([]));
    let controller: TableStateController | null = null;
    function Probe() {
      controller = useTableStateController({ worldId: "w1", onError: () => {} });
      return null;
    }
    const root = createRoot(document.createElement("div"));
    await act(async () => root.render(createElement(Probe)));
    await act(async () => {
      void controller!.refresh();
    });
    act(() => controller!.hydrate({ table: {}, tree: { 玩家: { 金幣: "新" } } }, []));
    await act(async () => state.resolve({ state: { table: {}, tree: { 玩家: { 金幣: "舊" } } } }));
    expect(controller!.tree).toEqual({ 玩家: { 金幣: "新" } });
    await act(async () => root.unmount());
  });
});

describe("useTableStateController 在場名單集合", () => {
  it("hydrate 與重讀都從集合形狀取出 present_scoped 的路徑", async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    handler.current = async (command) =>
      command === "read_state"
        ? {
            state: { table: {}, tree: {} },
            mechanism: { collections: { 人物: { present_scoped: true }, 选项: {} } },
          }
        : [];
    let controller: TableStateController | null = null;
    function Probe() {
      controller = useTableStateController({ worldId: "w1", onError: () => {} });
      return null;
    }
    const root = createRoot(document.createElement("div"));
    await act(async () => root.render(createElement(Probe)));
    act(() =>
      controller!.hydrate({ table: {}, tree: {} }, [], undefined, undefined, undefined, {
        甲: { present_scoped: true },
        乙: {},
      }),
    );
    expect(controller!.presentScoped).toEqual(["甲"]);
    await act(async () => controller!.refresh());
    expect(controller!.presentScoped).toEqual(["人物"]);
    handler.current = null;
    await act(async () => root.unmount());
  });
});
