// @vitest-environment happy-dom
// 換幕類入口取得桌級鎖、過了離開守門之後，要用同步的 chat.isBusy() 再看一次：
// 守門的確認框等人作答期間對話可能已經開跑，而 chat.busy（state）要晚一拍才會變 true。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn(async () => 0));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { useRef, useState } from "react";
import { advanceThenReopen, useSceneActions, type SceneActions } from "./useSceneActions";
import { useTableOp } from "../lobby/useTableOp";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("scene actions re-check the synchronous busy flag", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    invokeMock.mockClear();
  });

  function mount(
    isBusy: () => boolean,
    entry: { entered: boolean; writable: boolean } = { entered: true, writable: true },
    onError: (message: string) => void = () => {},
  ) {
    const chat = {
      events: [{ ts: "", speaker_id: "", speaker_name: "", kind: "narration" as const, text: "x" }],
      busy: false,
      isBusy,
      beginNarration: vi.fn(() => "turn-1"),
      endNarration: vi.fn(),
    };
    const enterTable = vi.fn(async () => entry);
    let actions!: SceneActions;
    function Harness() {
      actions = useSceneActions({
        worldId: "w1",
        scene: 1,
        sceneTitles: {},
        sceneLabels: {},
        tableName: "T",
        chat,
        canLeaveEditor: async () => true,
        enterTable,
        runTableOp: async (fn) => fn(),
        closeMainView: () => {},
        onError,
      });
      return null;
    }
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root?.render(<Harness />));
    return { get actions() { return actions; }, chat, enterTable };
  }

  const backendCalls = () => invokeMock.mock.calls.map((call) => (call as unknown[])[0]);

  it.each(["advanceScene", "regenerateSummary", "revertScene", "forkScene"] as const)(
    "%s gives up when a turn is already running",
    async (name) => {
      const mounted = mount(() => true);
      await act(async () => {
        await (name === "forkScene" ? mounted.actions.forkScene(0) : mounted.actions[name]());
      });
      expect(mounted.chat.beginNarration).not.toHaveBeenCalled();
      expect(mounted.enterTable).not.toHaveBeenCalled();
      expect(backendCalls()).toEqual([]);
    },
  );

  it("advanceScene goes ahead when nothing is running and reports success", async () => {
    const mounted = mount(() => false);
    let done: boolean | undefined;
    await act(async () => {
      done = await mounted.actions.advanceScene();
    });
    expect(backendCalls()).toContain("advance_scene");
    expect(done).toBe(true);
  });

  it.each([
    ["busy（沒進桌）", { entered: false, writable: false }],
    ["唯讀／待修復（進了但不可玩）", { entered: true, writable: false }],
  ])("advanceScene：換幕成功但重進桌%s，不回報成功", async (_label, entry) => {
    const mounted = mount(() => false, entry);
    let done: boolean | undefined;
    await act(async () => {
      done = await mounted.actions.advanceScene();
    });
    expect(backendCalls()).toContain("advance_scene");
    expect(mounted.enterTable).toHaveBeenCalled();
    expect(done).toBe(false);
  });

  it("advanceScene reports false when skipped for a running turn", async () => {
    const mounted = mount(() => true);
    let done: boolean | undefined;
    await act(async () => {
      done = await mounted.actions.advanceScene();
    });
    expect(done).toBe(false);
  });

  it("advance hands the stop turn id to the backend; a user stop is not an error", async () => {
    const errors: string[] = [];
    const mounted = mount(
      () => false,
      undefined,
      (message) => {
        if (message) errors.push(message);
      },
    );
    invokeMock.mockImplementationOnce(async () => {
      throw 'TTMSG:{"code":"scene_summary_stopped"}';
    });
    await act(async () => {
      await mounted.actions.advanceScene();
    });
    const call = invokeMock.mock.calls.find(
      (entry) => (entry as unknown[])[0] === "advance_scene",
    ) as unknown[];
    expect(call[1]).toEqual({ worldId: "w1", turnId: "turn-1" });
    expect(errors).toEqual([]);
    expect(mounted.chat.endNarration).toHaveBeenCalled();
  });
});

describe("advanceThenReopen", () => {
  it.each([
    [true, 1],
    [false, 0],
  ])("advance 回 %s → 重開介面 %i 次", async (done, times) => {
    const reopen = vi.fn();
    await advanceThenReopen(async () => done, reopen);
    expect(reopen).toHaveBeenCalledTimes(times);
  });
});

// 重寫提要後重進桌還沒完成時：補救鈕停用（busy 含桌級鎖）、連按不派送；重進桌完成後第一下就有效。
// 接線照 AppWorkspace／PlayView：disabled={chat.busy || tableOp.busy}、桌級鎖用真的 useTableOp
describe("重寫提要後的退回前幕", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    invokeMock.mockClear();
  });

  it("重進桌卡住期間按鈕停用、連按不派送，解鎖後首次點擊有效", async () => {
    let release!: () => void;
    const enterTable = vi
      .fn<(id: string) => Promise<{ entered: boolean; writable: boolean }>>()
      .mockImplementationOnce(
        () => new Promise((resolve) => (release = () => resolve({ entered: true, writable: true }))),
      )
      .mockImplementation(async () => ({ entered: true, writable: true }));
    function Harness() {
      const tableOp = useTableOp();
      const [busy, setBusy] = useState(false);
      const busyRef = useRef(false);
      const chat = {
        events: [{ ts: "", speaker_id: "", speaker_name: "GM", kind: "narration" as const, text: "摘要" }],
        busy,
        isBusy: () => busyRef.current,
        beginNarration: () => {
          busyRef.current = true;
          setBusy(true);
          return "turn-1";
        },
        endNarration: () => {
          busyRef.current = false;
          setBusy(false);
        },
      };
      const actions = useSceneActions({
        worldId: "w1",
        scene: 1,
        sceneTitles: {},
        sceneLabels: {},
        tableName: "T",
        chat,
        canLeaveEditor: async () => true,
        enterTable,
        runTableOp: tableOp.run,
        closeMainView: () => {},
        onError: () => {},
      });
      const disabled = chat.busy || tableOp.busy;
      return (
        <>
          <button id="regen" disabled={disabled} onClick={() => void actions.regenerateSummary()} />
          <button id="revert" disabled={disabled} onClick={() => void actions.revertScene()} />
        </>
      );
    }
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root?.render(<Harness />));
    const button = (id: string) => host!.querySelector<HTMLButtonElement>(`#${id}`)!;
    const calls = () => invokeMock.mock.calls.map((call) => (call as unknown[])[0]);

    await act(async () => button("regen").click());
    expect(calls()).toEqual(["regenerate_scene_summary"]);
    expect(button("revert").disabled).toBe(true);
    await act(async () => {
      button("revert").click();
      button("revert").click();
    });
    expect(calls()).toEqual(["regenerate_scene_summary"]);

    await act(async () => release());
    expect(button("revert").disabled).toBe(false);
    await act(async () => button("revert").click());
    expect(calls()).toEqual(["regenerate_scene_summary", "revert_scene"]);
  });
});

// tableOpBusy 單獨生效：對話沒在跑、但桌級鎖被別的換桌級操作持有時，補救鈕照樣停用、按了也不派送
describe("桌級鎖持有中的補救鈕", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    invokeMock.mockClear();
  });

  it("chat 不忙、桌鎖持有：停用；鎖放開後可按", async () => {
    let hold!: (fn: () => Promise<void>) => Promise<unknown>;
    function Harness() {
      const tableOp = useTableOp();
      hold = tableOp.run;
      const chat = {
        events: [{ ts: "", speaker_id: "", speaker_name: "GM", kind: "narration" as const, text: "摘要" }],
        busy: false,
        isBusy: () => false,
        beginNarration: () => "turn-1",
        endNarration: () => {},
      };
      const actions = useSceneActions({
        worldId: "w1",
        scene: 1,
        sceneTitles: {},
        sceneLabels: {},
        tableName: "T",
        chat,
        canLeaveEditor: async () => true,
        enterTable: async () => ({ entered: true, writable: true }),
        runTableOp: tableOp.run,
        closeMainView: () => {},
        onError: () => {},
      });
      return <button id="revert" disabled={chat.busy || tableOp.busy} onClick={() => void actions.revertScene()} />;
    }
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root?.render(<Harness />));
    const button = () => host!.querySelector<HTMLButtonElement>("#revert")!;
    expect(button().disabled).toBe(false);

    let release!: () => void;
    let held!: Promise<unknown>;
    await act(async () => {
      held = hold(() => new Promise<void>((resolve) => (release = resolve)));
    });
    expect(button().disabled).toBe(true);
    await act(async () => button().click());
    expect(invokeMock).not.toHaveBeenCalled();

    await act(async () => {
      release();
      await held;
    });
    expect(button().disabled).toBe(false);
  });
});
