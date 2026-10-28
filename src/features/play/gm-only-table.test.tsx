// @vitest-environment happy-dom
// 介面接管桌只走 GM 一條敘事線〔作者裁決 2026-10-27〕：判定綁桌、未就緒不接受回合、發言對象鎖 GM、
// 點名相關按鈕藏起來；一般桌照舊能選角色當對象。
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const pending = new Map<string, (value: { gm_only: boolean; nominates: boolean }) => void>();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(
    (command: string, args: { worldId: string }) =>
      new Promise((resolve) => {
        if (command === "gm_table_mode") pending.set(args.worldId, resolve);
        else resolve(null);
      }),
  ),
}));

import { invoke } from "@tauri-apps/api/core";
import { useGmOnlyTable } from "./useGmOnlyTable";
import {
  GM_TARGET,
  type WorkspaceNavigationController,
  useWorkspaceNavigationController,
} from "../../controllers/useWorkspaceNavigationController";
import { PlayView } from "./PlayView";
import { t } from "../../i18n";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// 哪些卡是舞台卡（有效發言對象的判斷）
const stageIds = new Set<string>();
const characters = {
  player: null,
  metaOf: (id: string) => (stageIds.has(id) ? { stage: true } : undefined),
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

  let cast = {
    active: [{ id: "c1" }],
    list: [
      { id: "c1", archived: false, auto_hidden: false },
      { id: "c2", archived: false, auto_hidden: true },
    ],
  };

  function Probe() {
    gate = useGmOnlyTable(worldId, cast);
    nav = useWorkspaceNavigationController({ worldId, gmOnly: gate.gmOnly, characters, onError: () => {} });
    return null;
  }

  async function render() {
    await act(async () => root!.render(createElement(Probe)));
  }

  async function answer(id: string, gmOnly: boolean, nominates = !gmOnly) {
    await act(async () => pending.get(id)!({ gm_only: gmOnly, nominates }));
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
    expect(gate!.nominates).toBe(false);
    await gate!.whenReady(turn)();
    expect(turn).toHaveBeenCalledTimes(1);

    // 換到一般桌：讀回前是未就緒，送出與空 trigger 共用的閘都擋下
    worldId = "w2";
    await render();
    expect(gate!.gmOnly).toBeUndefined();
    expect(gate!.nominates).toBe(false);
    expect(nav!.speaker).not.toBe(GM_TARGET);
    await gate!.whenReady(turn)();
    expect(turn).toHaveBeenCalledTimes(1);
    await answer("w2", false);
    expect(gate!.nominates).toBe(true);
    await gate!.whenReady(turn)();
    expect(turn).toHaveBeenCalledTimes(2);
    // 卡片自帶格式的桌：一般桌但不點名
    worldId = "w3";
    await render();
    await answer("w3", false, false);
    expect(gate!.gmOnly).toBe(false);
    expect(gate!.nominates).toBe(false);
  });

  it("主區 id 不變、只翻轉某張卡的 auto_hidden 旗標也重讀會不會點名", async () => {
    const reads = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === "gm_table_mode").length;
    await render();
    await answer("w1", false);
    const before = reads();
    await render();
    expect(reads()).toBe(before);
    cast = { ...cast, list: [cast.list[0], { id: "c2", archived: false, auto_hidden: false }] };
    await render();
    expect(reads()).toBe(before + 1);
    await answer("w1", false, false);
    expect(gate!.nominates).toBe(false);
  });

  it("某張卡切成舞台卡（主區 id 不變）也重讀會不會點名", async () => {
    const reads = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === "gm_table_mode").length;
    await render();
    await answer("w1", false);
    const before = reads();
    cast = { ...cast, list: [{ ...cast.list[0], stage: true } as (typeof cast.list)[number], cast.list[1]] };
    await render();
    expect(reads()).toBe(before + 1);
  });

  it("舞台卡點了不當發言對象（一般桌）", async () => {
    await render();
    await answer("w1", false);
    await act(async () => nav!.selectCard("c1", false));
    expect(nav!.speaker).toBe("");
    await act(async () => nav!.selectCard("c1"));
    expect(nav!.speaker).toBe("c1");
  });

  it("選中的對象後來變成舞台卡：對外的發言對象（畫面與送出共用）變成沒有對象", async () => {
    await render();
    await answer("w1", false);
    await act(async () => nav!.selectCard("c1"));
    expect(nav!.speaker).toBe("c1");
    stageIds.add("c1");
    await render();
    expect(nav!.speaker).toBe("");
    expect(nav!.gmTargeted).toBe(false);
    stageIds.clear();
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
