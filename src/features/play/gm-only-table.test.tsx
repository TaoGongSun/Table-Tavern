// @vitest-environment happy-dom
// 介面接管桌只走 GM 一條敘事線〔作者裁決 2026-10-27〕：判定綁桌、未就緒不接受回合、發言對象鎖 GM、
// 點名相關按鈕藏起來；一般桌照舊能選角色當對象。
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const pending = new Map<string, (value: boolean) => void>();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(
    (command: string, args: { worldId: string }) =>
      new Promise((resolve) => {
        if (command === "interface_takeover_table") pending.set(args.worldId, resolve);
        else resolve(null);
      }),
  ),
}));

import { useGmOnlyTable } from "./useGmOnlyTable";
import {
  GM_TARGET,
  type WorkspaceNavigationController,
  useWorkspaceNavigationController,
} from "../../controllers/useWorkspaceNavigationController";
import { PlayView } from "./PlayView";
import { t } from "../../i18n";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const characters = {
  player: null,
  refresh: async () => [],
  reloadPlayer: async () => {},
  noteRemoved: async () => null,
  remove: async () => "cancelled" as const,
  removePlayer: async () => true,
};

describe("gm-only table", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let worldId = "w1";
  let gate: ReturnType<typeof useGmOnlyTable> | null = null;
  let nav: WorkspaceNavigationController | null = null;

  function Probe() {
    gate = useGmOnlyTable(worldId);
    nav = useWorkspaceNavigationController({ worldId, gmOnly: gate.gmOnly, characters, onError: () => {} });
    return null;
  }

  async function render() {
    await act(async () => root!.render(createElement(Probe)));
  }

  async function answer(id: string, gmOnly: boolean) {
    await act(async () => pending.get(id)!(gmOnly));
  }

  beforeEach(() => {
    pending.clear();
    worldId = "w1";
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
  });

  it("換桌載入中不接受回合，上一桌的判定不帶到新桌", async () => {
    await render();
    const turn = vi.fn(async () => {});
    await gate!.whenReady(turn)();
    expect(turn).not.toHaveBeenCalled();
    await answer("w1", true);
    expect(gate!.gmOnly).toBe(true);
    await gate!.whenReady(turn)();
    expect(turn).toHaveBeenCalledTimes(1);

    // 換到一般桌：讀回前是未就緒，送出與空 trigger 共用的閘都擋下
    worldId = "w2";
    await render();
    expect(gate!.gmOnly).toBeUndefined();
    expect(nav!.speaker).not.toBe(GM_TARGET);
    await gate!.whenReady(turn)();
    expect(turn).toHaveBeenCalledTimes(1);
    await answer("w2", false);
    await gate!.whenReady(turn)();
    expect(turn).toHaveBeenCalledTimes(2);
  });

  it("只走 GM 的桌：發言對象鎖 GM，點卡不選對象；一般桌照常選", async () => {
    await render();
    await answer("w1", true);
    expect(nav!.speaker).toBe(GM_TARGET);
    expect(nav!.gmTargeted).toBe(true);
    await act(async () => nav!.selectCard("c1"));
    expect(nav!.speaker).toBe(GM_TARGET);

    worldId = "w2";
    await render();
    await answer("w2", false);
    await act(async () => nav!.selectCard("c1"));
    expect(nav!.speaker).toBe("c1");
    expect(nav!.gmOnly).toBe(false);
  });

  it("只走 GM 的桌藏請他發言、GM 推進與清除對象，留 GM 旁白", () => {
    const noop = () => {};
    const props = {
      onboarding: null,
      sceneLabel: "第 1 幕",
      storyKey: "w1\u00000",
      events: [],
      notices: [],
      metaOf: () => undefined,
      generating: null,
      generatingMeta: undefined,
      streamText: "",
      busy: false,
      canRestore: false,
      onRestoreUndone: noop,
      locked: false,
      canUndoScene: false,
      onRegenerateSummary: noop,
      onRevertScene: noop,
      speaker: GM_TARGET,
      gmTargeted: true,
      targetName: "GM",
      targetColor: "#000",
      targetImage: null,
      targetEmoji: "🎭",
      onClearTarget: noop,
      input: "",
      onInputChange: noop,
      castEmpty: false,
      onSubmit: noop,
      canStop: false,
      onStop: noop,
      requestReplyLabel: "GM",
      onUndoLast: noop,
      onRequestReply: noop,
      onGmNarrate: noop,
      onGmAdvance: noop,
      errorNote: null,
      sceneBudget: null,
      onAdvanceScene: noop,
      canAdvanceScene: false,
    } as unknown as Parameters<typeof PlayView>[0];
    const labels = (gmOnly: boolean) => {
      act(() => root!.render(<PlayView {...props} gmOnly={gmOnly} />));
      return [...host!.querySelectorAll("button")].map((button) => button.textContent ?? "");
    };
    const normal = labels(false);
    expect(normal).toContain(t("gmAdvance"));
    expect(normal).toContain(t("requestReplyShort"));
    expect(normal).toContain(t("clearTarget"));
    const gmOnly = labels(true);
    expect(gmOnly).not.toContain(t("gmAdvance"));
    expect(gmOnly).not.toContain(t("requestReplyShort"));
    expect(gmOnly).not.toContain(t("clearTarget"));
    expect(gmOnly).toContain(t("gmNarrate"));
  });
});
