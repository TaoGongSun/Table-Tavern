// @vitest-environment happy-dom
// 卡片介面 controller：面板開關跟著殼走；讀訊息快照、doc 與 key 只在該變的時候變。

import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { type CardInterface } from "./interface-card";
import { type CardInterfaceController, useCardInterfaceController } from "./useCardInterfaceController";
import { type StateNode, type TranscriptEvent } from "../../shared/contracts/backend-contracts";

const backend = vi.hoisted(() => ({
  shell: null as string | null,
  mode: null as string | null,
  mvu: false,
}));

// 不用 $1 的殼：每一樓產出的殼字串都一樣，跟讀本樓的狀態欄殼同型
const fixedCard: CardInterface = {
  character_id: "c1",
  character_name: "卡",
  scripts: [
    {
      name: "殼",
      find_regex: "/<UI>([\\s\\S]*?)<\\/UI>/s",
      replace_string: "```html\n<!DOCTYPE html><head></head><body>固定殼</body>\n```",
      trim_strings: [],
      min_depth: null,
      max_depth: null,
    },
  ],
  unsupported: null,
  opening: "<UI>開場白</UI>",
};

// MVU 卡：占位殼，GM 樓比照 MVU 補占位才畫得出來
const mvuCard: CardInterface = {
  character_id: "c2",
  character_name: "迷宮",
  scripts: [
    {
      name: "狀態欄",
      find_regex: "<StatusPlaceHolderImpl/>",
      replace_string: "```html\n<!DOCTYPE html><head></head><body>MVU 殼</body>\n```",
      trim_strings: [],
      min_depth: null,
      max_depth: null,
    },
  ],
  unsupported: null,
  opening: null,
  mvu: true,
};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "card_interfaces") return [backend.mvu ? mvuCard : fixedCard];
    if (command === "refactor_interface_shell") return backend.shell;
    if (command === "refactor_table_mode") return backend.mode;
    return null;
  }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const gm = (raw: string): TranscriptEvent => ({
  ts: "t",
  speaker_id: "gm",
  speaker_name: "GM",
  kind: "narration",
  text: "正文",
  raw,
});
const player = (text: string): TranscriptEvent => ({ ts: "t", speaker_id: "", speaker_name: "玩家", kind: "player", text });
const submitText = async () => {};

describe("useCardInterfaceController", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let controller: CardInterfaceController | null = null;
  let props = { worldId: "w1", events: [] as TranscriptEvent[], tree: {} as Record<string, StateNode> };

  function Probe() {
    controller = useCardInterfaceController({
      worldId: props.worldId,
      events: props.events,
      tree: props.tree,
      userName: "阿濤",
      submitText,
    });
    return null;
  }

  async function settle() {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }

  async function render(next: Partial<typeof props> = {}) {
    props = { ...props, ...next };
    if (root === null) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () => root?.render(createElement(Probe)));
    await settle();
  }

  beforeEach(() => {
    backend.mode = null;
    backend.shell = null;
    backend.mvu = false;
    props = { worldId: "w1", events: [gm("<UI>第一樓</UI>")], tree: {} };
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => void store.set(key, String(value)),
      removeItem: (key: string) => void store.delete(key),
      clear: () => store.clear(),
    });
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
    controller = null;
    vi.unstubAllGlobals();
  });

  it("closes the panel when the shell goes away and stays closed when it comes back", async () => {
    backend.mode = "interface";
    backend.shell = "<UI>{{世界.時間}}</UI>";
    await render({ events: [{ ...gm("沒有標籤"), raw: undefined, state: { table: {}, tree: { 世界: { 時間: "黃昏" } } } }] });
    expect(controller!.shellReady).toBe(true);

    await act(async () => controller!.open());
    expect(controller!.uiOpen).toBe(true);

    backend.shell = null;
    await act(async () => {
      await controller!.refreshShell("w1");
    });
    expect(controller!.shellReady).toBe(false);
    expect(controller!.uiOpen).toBe(false);

    backend.shell = "<UI>{{世界.時間}}</UI>";
    await act(async () => {
      await controller!.refreshShell("w1");
    });
    expect(controller!.shellReady).toBe(true);
    expect(controller!.uiOpen).toBe(false);
  });

  it("same shell, same floor, new text: key changes so the iframe remounts", async () => {
    await render();
    const before = controller!.shellKey;
    await render({ events: [gm("<UI>第一樓改過</UI>")] });
    expect(controller!.chat?.currentId).toBe(0);
    expect(controller!.shellKey).not.toBe(before);
  });

  it("switching tables with the same shell and text still changes the key", async () => {
    await render();
    const before = controller!.shellKey;
    await render({ worldId: "w2" });
    expect(controller!.shellKey).not.toBe(before);
  });

  it("a new player floor updates the snapshot without remounting", async () => {
    await render();
    await act(async () => controller!.open());
    const key = controller!.shellKey;
    const doc = controller!.shellDoc;
    await render({ events: [gm("<UI>第一樓</UI>"), player("新的一句")] });
    expect(controller!.shellKey).toBe(key);
    expect(controller!.shellDoc).toBe(doc);
    expect(controller!.chat?.floors.map((floor) => floor.message)).toEqual(["<UI>第一樓</UI>", "新的一句"]);
  });

  it("unrelated renders keep doc and key", async () => {
    await render();
    await act(async () => controller!.open());
    const key = controller!.shellKey;
    const doc = controller!.shellDoc;
    // 同一份逐字稿換了新陣列參照（例如其他狀態重讀）：殼與本樓都沒變
    await render({ events: [...props.events] });
    expect(controller!.shellKey).toBe(key);
    expect(controller!.shellDoc).toBe(doc);
  });

  it("reopening after other floors changed embeds the latest snapshot before the card runs", async () => {
    await render();
    await act(async () => controller!.open());
    expect(controller!.shellDoc).not.toContain("面板關著時新增");
    await act(async () => controller!.close());
    expect(controller!.shellDoc).toBeNull();
    await render({ events: [gm("<UI>第一樓</UI>"), player("面板關著時新增")] });
    await act(async () => controller!.open());
    expect(controller!.shellDoc).toContain("面板關著時新增");
  });

  it("popping the latest floor moves the current floor back", async () => {
    await render({ events: [gm("<UI>第一樓</UI>"), player("x"), gm("<UI>第三樓</UI>")] });
    expect(controller!.chat?.currentId).toBe(2);
    const key = controller!.shellKey;
    await render({ events: [gm("<UI>第一樓</UI>"), player("x")] });
    expect(controller!.chat?.currentId).toBe(0);
    expect(controller!.shellKey).not.toBe(key);
  });

  it("skeleton tables keep the synthesized text on the current floor after a push", async () => {
    backend.mode = "interface";
    backend.shell = "<UI>骨架 {{世界.時間}}</UI>";
    const noTag = { ...gm("沒有標籤"), raw: undefined, state: { table: {}, tree: { 世界: { 時間: "黃昏" } } } };
    await render({ events: [noTag] });
    expect(controller!.chat?.floors[0].message).toBe("<UI>骨架 黃昏</UI>");
    await render({ events: [noTag, player("新的一句")] });
    expect(controller!.chat?.currentId).toBe(0);
    expect(controller!.chat?.floors[0].message).toBe("<UI>骨架 黃昏</UI>");
  });

  describe("MVU 卡", () => {
    // 會改狀態的樓是 speaker_id 空字串的 narration（開場與 GM 回覆）
    const gmTurn = (text: string, gold: string): TranscriptEvent => ({
      ts: "t",
      speaker_id: "",
      speaker_name: "GM",
      kind: "narration",
      text,
      state: { table: {}, tree: { 玩家: { 金幣: gold } } },
    });
    const playerAt = (text: string, gold: string): TranscriptEvent => ({
      ...player(text),
      state: { table: {}, tree: { 玩家: { 金幣: gold } } },
    });
    const stat = (id: number) => {
      const mvu = controller!.mvu!;
      return mvu.states[mvu.floorState[id]].stat_data;
    };

    beforeEach(() => {
      backend.mvu = true;
    });

    it("GM 樓補占位才有殼；殼後面接玩家句仍讀活狀態（含手改值）", async () => {
      await render({
        events: [gmTurn("開場白", "1"), playerAt("我進去", "1"), gmTurn("第二回合正文", "5"), playerAt("再一句", "5")],
        tree: { 玩家: { 金幣: "5" } },
      });
      expect(controller!.shellReady).toBe(true);
      expect(controller!.chat?.currentId).toBe(2);
      expect(controller!.chat?.floors[2].message).toBe("第二回合正文\n\n<StatusPlaceHolderImpl/>");
      expect(controller!.chat?.floors[0].message).toBe("開場白");
      expect(controller!.mvu?.currentId).toBe(2);
      expect(stat(2)).toEqual({ 玩家: { 金幣: 5 } });
      expect(stat(0)).toEqual({ 玩家: { 金幣: 1 } });
    });

    it("手改值：不重掛（doc、key 不變），只換推送的快照", async () => {
      const events = [gmTurn("開場白", "1"), playerAt("我進去", "1"), gmTurn("第二回合正文", "5"), playerAt("再一句", "5")];
      await render({ events, tree: { 玩家: { 金幣: "5" } } });
      await act(async () => controller!.open());
      const key = controller!.shellKey;
      const doc = controller!.shellDoc;
      expect(doc).toContain("getAllVariables");
      await render({ tree: { 玩家: { 金幣: "99" } } });
      expect(controller!.shellKey).toBe(key);
      expect(controller!.shellDoc).toBe(doc);
      expect(stat(2)).toEqual({ 玩家: { 金幣: 99 } });
      // 歷史樓不吃手改值
      expect(stat(0)).toEqual({ 玩家: { 金幣: 1 } });
    });

    it("先手改值再送玩家句：玩家句快照是改後值、GM 事件是改前值，GM 殼仍顯示改後值", async () => {
      await render({
        events: [gmTurn("開場白", "1"), playerAt("我進去", "1"), gmTurn("第二回合正文", "5"), playerAt("改完才送", "77")],
        tree: { 玩家: { 金幣: "77" } },
      });
      expect(controller!.chat?.currentId).toBe(2);
      expect(stat(2)).toEqual({ 玩家: { 金幣: 77 } });
    });

    it("收回最新 GM 樓：本樓與 MVU 快照一起退回，前一樓變成活樓", async () => {
      const events = [gmTurn("開場白", "1"), gmTurn("第二回合正文", "5"), playerAt("我進去", "5"), gmTurn("第三回合正文", "9")];
      await render({ events, tree: { 玩家: { 金幣: "9" } } });
      expect(controller!.mvu?.currentId).toBe(3);
      const key = controller!.shellKey;
      await render({ events: events.slice(0, 3), tree: { 玩家: { 金幣: "5" } } });
      expect(controller!.mvu?.currentId).toBe(1);
      expect(controller!.shellKey).not.toBe(key);
      expect(stat(1)).toEqual({ 玩家: { 金幣: 5 } });
    });

    it("切桌：快照換成新桌的狀態，key 跟著換", async () => {
      const events = [gmTurn("開場白", "1"), gmTurn("第二回合正文", "5")];
      await render({ events, tree: { 玩家: { 金幣: "5" } } });
      const key = controller!.shellKey;
      await render({ worldId: "w2", tree: { 玩家: { 金幣: "42" } } });
      expect(controller!.shellKey).not.toBe(key);
      expect(stat(1)).toEqual({ 玩家: { 金幣: 42 } });
    });

    it("沒有 MVU 卡的桌不給 MVU 快照、不補占位", async () => {
      backend.mvu = false;
      await render({ events: [gm("<UI>第一樓</UI>")], tree: { 玩家: { 金幣: "5" } } });
      expect(controller!.mvu).toBeNull();
      await act(async () => controller!.open());
      expect(controller!.shellDoc).not.toContain("getAllVariables");
    });
  });
});
