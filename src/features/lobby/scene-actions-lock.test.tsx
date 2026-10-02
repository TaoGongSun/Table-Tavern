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

import { useSceneActions, type SceneActions } from "../../controllers/useSceneActions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("scene actions re-check the synchronous busy flag", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    invokeMock.mockClear();
  });

  function mount(isBusy: () => boolean) {
    const chat = {
      events: [{ ts: "", speaker_id: "", speaker_name: "", kind: "narration" as const, text: "x" }],
      busy: false,
      isBusy,
      beginNarration: vi.fn(),
      endNarration: vi.fn(),
      noteTurnDone: vi.fn(),
    };
    const enterTable = vi.fn(async () => ({ entered: true, writable: true }));
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
        onError: () => {},
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

  it("advanceScene goes ahead when nothing is running", async () => {
    const mounted = mount(() => false);
    await act(async () => {
      await mounted.actions.advanceScene();
    });
    expect(backendCalls()).toContain("advance_scene");
  });
});
