// @vitest-environment happy-dom
// 排在進行中回合後面的操作：等整個回合結束（isTurnRunning() 回 false）才送後端；等待中不重複送出，
// 有回合在跑才亮等待提示，後端回來就收；等待中換桌或卸載直接終止、不送出；已送出的照原桌完成但
// live() 為 false，新桌不被它擋住。
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { type TurnWaitOp, useTurnWait } from "./useTurnWait";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root | null = null;
let wait: ReturnType<typeof useTurnWait> | null = null;
const turn = { running: false };

function Harness({ scope }: { scope: string }) {
  wait = useTurnWait(() => turn.running, scope);
  return null;
}

async function mount(scope = "A") {
  root = createRoot(document.createElement("div"));
  await act(async () => root?.render(createElement(Harness, { scope })));
}

async function rerender(scope: string) {
  await act(async () => root?.render(createElement(Harness, { scope })));
}

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => (resolve = done));
  return { promise, resolve };
}

afterEach(() => {
  act(() => root?.unmount());
  root = null;
  turn.running = false;
});

describe("useTurnWait", () => {
  it("回合進行中送出：回合結束前不送後端，等待提示亮到後端回來，期間再按不會送第二次", async () => {
    await mount();
    turn.running = true;
    const backend = deferred();
    const call = vi.fn(() => backend.promise);
    let first!: Promise<unknown>;
    await act(async () => {
      first = wait!.run((op) => op.backend(call));
    });
    expect(wait!.busy).toBe(true);
    expect(wait!.waiting).toBe(true);
    let second: unknown = "pending";
    await act(async () => {
      second = await wait!.run((op) => op.backend(call));
    });
    expect(second).toBeUndefined();
    await new Promise((resolve) => setTimeout(resolve, 120));
    expect(call).not.toHaveBeenCalled();
    turn.running = false;
    await vi.waitFor(() => expect(call).toHaveBeenCalledTimes(1));
    expect(wait!.waiting).toBe(true);
    await act(async () => {
      backend.resolve();
      await first;
    });
    expect(call).toHaveBeenCalledTimes(1);
    expect(wait!.busy).toBe(false);
    expect(wait!.waiting).toBe(false);
  });

  it("沒有回合在跑：照樣防重複，但不亮等待提示", async () => {
    await mount();
    const backend = deferred();
    let first!: Promise<unknown>;
    await act(async () => {
      first = wait!.run((op) => op.backend(() => backend.promise));
    });
    expect(wait!.busy).toBe(true);
    expect(wait!.waiting).toBe(false);
    await act(async () => {
      backend.resolve();
      await first;
    });
  });

  it("後端失敗也收掉提示與鎖", async () => {
    await mount();
    turn.running = true;
    let first!: Promise<unknown>;
    await act(async () => {
      first = wait!.run((op) => op.backend(() => Promise.reject(new Error("x")))).catch(() => {});
    });
    turn.running = false;
    await act(async () => {
      await first;
    });
    expect(wait!.busy).toBe(false);
    expect(wait!.waiting).toBe(false);
  });

  it("等待回合中換桌：舊操作直接終止、不送後端，新桌立刻能送", async () => {
    await mount("A");
    turn.running = true;
    const call = vi.fn(async () => "A");
    let first!: Promise<unknown>;
    await act(async () => {
      first = wait!.run((op) => op.backend(call));
    });
    await rerender("B");
    expect(wait!.busy).toBe(false);
    expect(wait!.waiting).toBe(false);
    turn.running = false;
    await expect(first).resolves.toBeUndefined();
    expect(call).not.toHaveBeenCalled();
    const fresh = vi.fn(async () => "B");
    let second: unknown;
    await act(async () => {
      second = await wait!.run((op) => op.backend(fresh));
    });
    expect(second).toBe("B");
    expect(fresh).toHaveBeenCalledTimes(1);
  });

  it("等待回合中卸載：直接終止、不送後端、不報錯", async () => {
    await mount();
    turn.running = true;
    const call = vi.fn(async () => {});
    let first!: Promise<unknown>;
    await act(async () => {
      first = wait!.run((op) => op.backend(call));
    });
    act(() => root?.unmount());
    root = null;
    await expect(first).resolves.toBeUndefined();
    turn.running = false;
    await new Promise((resolve) => setTimeout(resolve, 120));
    expect(call).not.toHaveBeenCalled();
  });

  it("已送出後換桌：舊操作照原桌完成但 live() 變 false，新桌立刻能送、狀態不被舊操作回寫", async () => {
    await mount("A");
    const backend = deferred();
    let op!: TurnWaitOp;
    let first!: Promise<unknown>;
    await act(async () => {
      first = wait!.run((given) => {
        op = given;
        return given.backend(() => backend.promise);
      });
    });
    expect(op.live()).toBe(true);
    await rerender("B");
    expect(op.live()).toBe(false);
    expect(wait!.busy).toBe(false);
    expect(wait!.waiting).toBe(false);
    turn.running = true;
    const fresh = deferred();
    const freshCall = vi.fn(() => fresh.promise);
    let second!: Promise<unknown>;
    await act(async () => {
      second = wait!.run((given) => given.backend(freshCall));
    });
    expect(wait!.busy).toBe(true);
    await act(async () => {
      backend.resolve();
      await first;
    });
    expect(wait!.busy).toBe(true);
    expect(wait!.waiting).toBe(true);
    turn.running = false;
    await vi.waitFor(() => expect(freshCall).toHaveBeenCalledTimes(1));
    await act(async () => {
      fresh.resolve();
      await second;
    });
    expect(wait!.busy).toBe(false);
  });

  it("已送出後卸載：舊操作 live() 變 false，回來時不報錯", async () => {
    await mount();
    const backend = deferred();
    let op!: TurnWaitOp;
    let first!: Promise<unknown>;
    await act(async () => {
      first = wait!.run((given) => {
        op = given;
        return given.backend(() => backend.promise);
      });
    });
    act(() => root?.unmount());
    root = null;
    expect(op.live()).toBe(false);
    backend.resolve();
    await expect(first).resolves.toBeUndefined();
  });
});
