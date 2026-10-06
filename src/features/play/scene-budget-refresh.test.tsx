// @vitest-environment happy-dom
// 換幕容量的取得：切換（忙／不忙、換桌、換幕、改設定）時撤銷在途請求，晚回的不覆蓋；
// 只收「目前設定下發出」的請求（回應帶回的設定指紋要等於目前設定的），不是只排除上一份。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const invokeMock = vi.hoisted(() => vi.fn<Invoke>());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { configTag, useSceneBudget } from "./useSceneBudget";
import type { SceneBudgetReply } from "./scene-budget";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

interface Props {
  worldId: string;
  scene: number;
  count: number;
  busy: boolean;
  config: string;
}

describe("useSceneBudget", () => {
  let root: Root | null = null;
  afterEach(() => {
    act(() => root?.unmount());
    root = null;
    invokeMock.mockReset();
  });

  function setup() {
    const pending: { args: Record<string, unknown>; resolve: (reply: SceneBudgetReply) => void }[] = [];
    invokeMock.mockImplementation(
      (_command, args = {}) => new Promise((resolve) => pending.push({ args, resolve: resolve as never })),
    );
    const seen: { value: SceneBudgetReply | null } = { value: null };
    function Harness(props: Props) {
      seen.value = useSceneBudget(props.worldId, props.scene, props.count, props.busy, { name: props.config });
      return null;
    }
    root = createRoot(document.createElement("div"));
    const render = (props: Props) => act(() => root!.render(<Harness {...props} />));
    // 回應照實帶回該請求送出的設定指紋（後端就是這樣做的）；override 可模擬別份設定下的回應
    const reply = (index: number, worldId: string, scene: number, override?: { seq?: number; tag?: string }) =>
      act(async () =>
        pending[index].resolve({
          worldId,
          scene,
          configGen: "g",
          configTag: override?.tag ?? (pending[index].args.configTag as string),
          requestSeq: override?.seq ?? (pending[index].args.requestSeq as number),
          summary: null,
          chatHint: true,
        }),
      );
    return { pending, seen, render, reply };
  }

  const base: Props = { worldId: "w1", scene: 0, count: 1, busy: false, config: "A" };

  it("newer request wins; an older reply landing late is dropped", async () => {
    const { pending, seen, render, reply } = setup();
    render(base);
    render({ ...base, count: 2 });
    expect(pending).toHaveLength(2);
    await reply(0, "w1", 0);
    expect(seen.value).toBeNull();
    await reply(1, "w1", 0);
    expect(seen.value).not.toBeNull();
  });

  it("going busy cancels the in-flight request; no request while busy", async () => {
    const { pending, seen, render, reply } = setup();
    render(base);
    render({ ...base, busy: true });
    expect(pending).toHaveLength(1);
    await reply(0, "w1", 0);
    expect(seen.value).toBeNull();
  });

  it("switching table while a request is in flight drops the old table's reply", async () => {
    const { seen, render, reply } = setup();
    render(base);
    render({ ...base, busy: true });
    render({ ...base, worldId: "w2" });
    await reply(0, "w1", 0);
    expect(seen.value).toBeNull();
    await reply(1, "w2", 0);
    expect(seen.value!.worldId).toBe("w2");
  });

  it("backend measured a different snapshot (no tag echoed): dropped", async () => {
    const { pending, seen, render, reply } = setup();
    render(base);
    expect(pending[0].args.config).toEqual({ name: "A" });
    await reply(0, "w1", 0, { tag: "" });
    expect(seen.value).toBeNull();
  });

  it("A→B→C with B never answered: a late A-settings reply is still dropped", async () => {
    const { pending, seen, render, reply } = setup();
    render(base);
    render({ ...base, config: "B" });
    render({ ...base, config: "C" });
    expect(pending).toHaveLength(3);
    // A 的請求晚回，即使序號被竄成最新也不收：指紋不是目前設定的
    const latest = pending[2].args.requestSeq as number;
    await reply(0, "w1", 0, { seq: latest });
    expect(seen.value).toBeNull();
    await reply(1, "w1", 0, { seq: latest });
    expect(seen.value).toBeNull();
    await reply(2, "w1", 0);
    expect(seen.value!.configTag).toBe(configTag(JSON.stringify({ name: "C" })));
    // 回到 A：A 下發出的新請求照常接受
    render({ ...base, config: "A" });
    await reply(3, "w1", 0);
    expect(seen.value!.configTag).toBe(configTag(JSON.stringify({ name: "A" })));
  });
});
