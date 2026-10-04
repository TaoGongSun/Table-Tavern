// @vitest-environment happy-dom
// 捲動：只換了變數表（手改狀態欄、卡片寫入）不動閱讀位置；多一則正文、換幕照舊跳到底。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import type { TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { PlayView } from "./PlayView";
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
      awayTooLong={false}
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
