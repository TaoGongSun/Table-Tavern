// @vitest-environment happy-dom
// 捲動：只換了變數表（手改狀態欄、卡片寫入）不動閱讀位置；多一則正文、換幕照舊跳到底。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { PlayView } from "./PlayView";
import { t } from "../../i18n";
import type { SceneBudgetReply } from "./scene-budget";
import type { ChatNotice } from "./useChatController";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const noop = () => {};

function view(events: TranscriptEvent[], storyKey = "w1\u00000", notices: ChatNotice[] = []) {
  return (
    <PlayView
      onboarding={null}
      sceneLabel="第 1 幕"
      storyKey={storyKey}
      events={events}
      notices={notices}
      metaOf={() => undefined}
      generating={null}
      generatingMeta={undefined}
      streamText=""
      busy={false}
      canRestore={false}
      onRestoreUndone={noop}
      canUndoScene={false}
      onRegenerateSummary={noop}
      onRevertScene={noop}
      speaker=""
      gmTargeted={false}
      targetName=""
      targetColor="#888888"
      targetImage={null}
      targetEmoji="🎭"
      onClearTarget={noop}
      input=""
      onInputChange={noop}
      castEmpty={false}
      onSubmit={noop}
      canStop={false}
      onStop={noop}
      requestReplyLabel=""
      onUndoLast={noop}
      onRequestReply={noop}
      onGmNarrate={noop}
      onGmAdvance={noop}
    />
  );
}

const story = (rev: string): TranscriptEvent[] => [
  { id: "a", ts: "t1", speaker_id: "", speaker_name: "", kind: "narration", text: "開場" },
  {
    id: "b",
    ts: "t2",
    speaker_id: "",
    speaker_name: "",
    kind: "narration",
    text: "第二段",
    message_vars: { hp: rev },
    vars_rev: rev,
  },
];

describe("PlayView 捲動", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
  });

  it("只換變數表保留閱讀位置；多一則正文跳到底", () => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root!.render(view(story("r1"))));
    const list = host.querySelector("section.messages") as HTMLElement;
    // happy-dom 不排版：內容高度自己給，scrollTop 是普通屬性
    Object.defineProperty(list, "scrollHeight", { get: () => 1000, configurable: true });
    list.scrollTop = 200;

    // 重讀換來全新物件、只有變數表與版本不同
    act(() => root!.render(view(story("r2"))));
    expect(list.scrollTop).toBe(200);

    const longer = [
      ...story("r2"),
      { ts: "t3", speaker_id: "", speaker_name: "", kind: "narration" as const, text: "新的一段" },
    ];
    act(() => root!.render(view(longer)));
    expect(list.scrollTop).toBe(1000);
  });

  it("正文改了（換幕、重讀到不同內容）照舊跳到底", () => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root!.render(view(story("r1"))));
    const list = host.querySelector("section.messages") as HTMLElement;
    Object.defineProperty(list, "scrollHeight", { get: () => 1000, configurable: true });
    list.scrollTop = 200;
    const otherScene = story("r1").map((event) => ({ ...event, ts: `${event.ts}-next` }));
    act(() => root!.render(view(otherScene)));
    expect(list.scrollTop).toBe(1000);
  });

  it("換桌或換幕（桌／幕 key 變了）即使事件一模一樣也跳到底", () => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root!.render(view(story("r1"))));
    const list = host.querySelector("section.messages") as HTMLElement;
    Object.defineProperty(list, "scrollHeight", { get: () => 1000, configurable: true });
    list.scrollTop = 200;
    act(() => root!.render(view(story("r1"), "w2\u00000")));
    expect(list.scrollTop).toBe(1000);
  });
});

describe("PlayView 免費模型換模提示行", () => {
  it("插在 at 指定的那則之前、不算逐字稿；at 超出（收回後變短）排在最後", () => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    const notices: ChatNotice[] = [
      { id: "n1", at: 1, notice: { kind: "failover", from: "甲", to: "乙" } },
      { id: "n2", at: 9, notice: { kind: "switched", model: "丙" } },
    ];
    act(() => root.render(view(story("r1"), "w1\u00000", notices)));
    const lines = Array.from(host.querySelectorAll(".messages > .message")).map((node) => node.textContent ?? "");
    expect(lines).toHaveLength(4);
    expect(lines[0]).toContain("開場");
    expect(lines[1]).toBe("甲 目前擁擠，已改用 乙。");
    expect(lines[2]).toContain("第二段");
    expect(lines[3]).toBe("免費模型已自動切換為 丙。");
    expect(host.querySelectorAll(".message-system")).toHaveLength(2);
    act(() => root.unmount());
    host.remove();
  });
});

describe("PlayView 換幕容量", () => {
  const budget = (used: number, cap: number, gReply: number, hint: boolean, chatHint = false): SceneBudgetReply => ({
    worldId: "w1",
    configGen: "g",
    configTag: "t",
    requestSeq: 1,
    scene: 0,
    summary: { unit: "bytes", used, cap, hint, ratio: 1, reliable: true, lockable: true, gReply, over: false },
    chatHint,
  });

  function render(sceneBudget: SceneBudgetReply | null, input: string, onAdvanceScene = vi.fn()) {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    const base = view(story("r1"));
    act(() =>
      root.render(
        <PlayView
          {...base.props}
          speaker="c1"
          targetName="甲"
          input={input}
          sceneBudget={sceneBudget}
          onAdvanceScene={onAdvanceScene}
          canAdvanceScene
        />,
      ),
    );
    const button = (label: string) =>
      Array.from(host.querySelectorAll("button")).find((node) => node.textContent?.includes(label));
    return { host, root, button, onAdvanceScene };
  }

  it("換幕觸發的提醒帶「現在換幕」鈕，按下就換幕；不擋任何動作", () => {
    const { host, root, button, onAdvanceScene } = render(budget(800, 1000, 50, true), "");
    expect(host.querySelector(".scene-capacity-hint")?.textContent).toContain(t("sceneCapacitySummaryHint"));
    act(() => button(t("sceneAdvanceNow"))!.click());
    expect(onAdvanceScene).toHaveBeenCalledTimes(1);
    expect(button(t("gmNarrate"))!.hasAttribute("disabled")).toBe(false);
    act(() => root.unmount());
    host.remove();
  });

  it("只因聊天觸發用中性文案", () => {
    const { host, root } = render(budget(10, 1000, 50, false, true), "");
    expect(host.querySelector(".scene-capacity-hint")?.textContent).toContain(t("sceneCapacityChatHint"));
    act(() => root.unmount());
    host.remove();
  });

  it("打字跨過門檻當下鎖送出、刪字解鎖；無玩家句的動作看本句 0", () => {
    // 900＋36＝936；本句「ab」＝66 → 1002 > 1000 鎖送出；空輸入 936 ≤ 1000 動作照常
    const send = () =>
      Array.from(document.querySelectorAll("button[type=submit]")).pop() as HTMLButtonElement;
    const typed = render(budget(900, 1000, 36, true), "ab");
    expect(send().disabled).toBe(true);
    expect(typed.host.querySelector(".scene-capacity-hint.is-full")?.textContent).toContain(t("sceneCapacityFull"));
    expect(typed.button(t("gmNarrate"))!.hasAttribute("disabled")).toBe(false);
    act(() => typed.root.unmount());
    typed.host.remove();
    const cleared = render(budget(900, 1000, 36, true), "");
    expect(send().disabled).toBe(false);
    act(() => cleared.root.unmount());
    cleared.host.remove();
    // 連無玩家句的動作都塞不下：旁白、推進、請某某發言全停，換幕鈕照常
    const full = render(budget(990, 1000, 36, true), "");
    expect(full.button(t("gmNarrate"))!.hasAttribute("disabled")).toBe(true);
    expect(full.button(t("gmAdvance"))!.hasAttribute("disabled")).toBe(true);
    expect(full.button(t("requestReplyShort"))!.hasAttribute("disabled")).toBe(true);
    expect(full.button(t("sceneAdvance"))!.hasAttribute("disabled")).toBe(false);
    act(() => full.root.unmount());
    full.host.remove();
  });
});
