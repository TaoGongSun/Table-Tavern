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
  active: false,
  generation: 3,
  varsHold: null as Promise<void> | null,
  writes: [] as Record<string, unknown>[],
  write: null as null | ((args: Record<string, unknown>) => Promise<unknown>),
  layers: [] as { key: string; rev: string | null; vars: string }[],
  layerReads: [] as Record<string, unknown>[],
  cards: null as CardInterface[] | null,
  layersFail: false,
  layersHold: null as Promise<void> | null,
  layerWrites: [] as Record<string, unknown>[],
  layerWrite: null as null | ((args: Record<string, unknown>) => Promise<unknown>),
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
  invoke: vi.fn(async (command: string, args: Record<string, unknown>) => {
    if (command === "card_interfaces") return backend.cards ?? [backend.mvu ? mvuCard : fixedCard];
    if (command === "card_vars_state") {
      if (backend.varsHold) await backend.varsHold;
      return { generation: backend.generation, active: backend.active, scene: 0 };
    }
    if (command === "card_vars_write") {
      backend.writes.push(args);
      return backend.write ? backend.write(args) : null;
    }
    if (command === "card_layers") {
      backend.layerReads.push(args);
      if (backend.layersHold) await backend.layersHold;
      if (backend.layersFail) throw new Error("讀不了");
      return backend.layers;
    }
    if (command === "card_layer_write") {
      backend.layerWrites.push(args);
      return backend.layerWrite ? backend.layerWrite(args) : null;
    }
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

  const updated: [TranscriptEvent, TranscriptEvent][] = [];

  function Probe() {
    controller = useCardInterfaceController({
      worldId: props.worldId,
      events: props.events,
      tree: props.tree,
      userName: "阿濤",
      submitText,
      onEventUpdated: (previous, next) => void updated.push([previous, next]),
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
    backend.active = false;
    backend.generation = 3;
    backend.varsHold = null;
    backend.writes = [];
    backend.write = null;
    backend.layers = [];
    backend.layersFail = false;
    backend.cards = null;
    backend.layersHold = null;
    backend.layerReads = [];
    backend.layerWrites = [];
    backend.layerWrite = null;
    updated.length = 0;
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

    describe("卡片變數模式（包 2a）", () => {
      const tabled = (id: string, rev: string, gold: number, text = "第二回合正文"): TranscriptEvent => ({
        ...gmTurn(text, String(gold)),
        id,
        vars_rev: rev,
        message_vars: { stat_data: { 玩家: { 金幣: gold } } },
      });
      // 沙盒送來的訊息：source 是那支 iframe 的 window，宿主回覆就寄給它
      function fromCard(data: Record<string, unknown>) {
        const frame = { postMessage: vi.fn() };
        window.dispatchEvent(
          new MessageEvent("message", {
            data: { source: "table-tavern-card", token: controller!.shellKey, generation: 3, scene: 0, ...data },
            source: frame as unknown as Window,
          }),
        );
        return frame;
      }

      beforeEach(() => {
        backend.active = true;
      });

      it("每樓讀事件上自己的表、沒有表回 {}；寫入目標是事件 id，舊事件是逐字稿位置", async () => {
        const hidden = { ...player("只給 GM"), gm_only: true };
        await render({ events: [tabled("e0", "r0", 1, "開場白"), hidden, playerAt("我進去", "1"), tabled("e3", "r3", 5)] });
        const mvu = controller!.mvu!;
        expect(mvu.active).toBe(true);
        expect(mvu.states[mvu.floorState[0]]).toEqual({ stat_data: { 玩家: { 金幣: 1 } } });
        expect(mvu.states[mvu.floorState[1]]).toEqual({});
        expect(mvu.targets).toEqual([
          { key: "e0", rev: "r0" },
          { key: "@2", rev: null },
          { key: "e3", rev: "r3" },
        ]);
      });

      it("卡寫：送後端帶世代、場、目標與預期版本；確認後換進逐字稿並回覆沙盒", async () => {
        const events = [tabled("e0", "r0", 1, "開場白"), tabled("e1", "r1", 5)];
        await render({ events });
        await act(async () => controller!.open());
        const committed = { ...events[1], vars_rev: "r2", message_vars: { stat_data: { 玩家: { 金幣: 6 } } } };
        backend.write = async () => ({ status: "ok", event: committed });
        let frame: { postMessage: ReturnType<typeof vi.fn> } | null = null;
        await act(async () => {
          frame = fromCard({
            kind: "mvu-write",
            requestId: "q1",
            target: "e1",
            base: "r1",
            payload: '{"stat_data":{"玩家":{"金幣":6}}}',
          });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        await settle();
        expect(backend.writes).toEqual([
          {
            worldId: "w1",
            generation: 3,
            scene: 0,
            target: { id: "e1" },
            expectedRev: "r1",
            varsJson: '{"stat_data":{"玩家":{"金幣":6}}}',
          },
        ]);
        expect(updated).toEqual([[events[1], committed]]);
        expect(frame!.postMessage).toHaveBeenCalledWith(
          {
            source: "table-tavern-host",
            kind: "mvu-settle",
            token: controller!.shellKey,
            results: [{ requestId: "q1", ok: true, rev: "r2" }],
            authority: undefined,
          },
          { targetOrigin: "*" },
        );
      });

      it("舊事件（沒有 id）首次寫入後換成帶 id 的那則；被拒時權威表與版本也換進逐字稿", async () => {
        const legacy = gmTurn("沒有 id 的舊回合正文", "3");
        const events = [tabled("e0", "r0", 1, "第一回合的正文"), legacy];
        await render({ events });
        await act(async () => controller!.open());
        const committed = { ...legacy, id: "new", vars_rev: "r5", message_vars: { stat_data: { x: 1 } } };
        backend.write = async () => ({ status: "ok", event: committed });
        await act(async () => {
          fromCard({ kind: "mvu-write", requestId: "q1", target: "@1", base: null, payload: '{"stat_data":{"x":1}}' });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        await settle();
        expect(backend.writes[0]).toMatchObject({ target: { index: 1, legacy }, expectedRev: null });
        expect(updated).toEqual([[legacy, committed]]);
        // 被拒：權威表與版本換進宿主逐字稿
        backend.write = async () => ({ status: "stale", found: true, rev: "r9", table: '{"stat_data":{"x":9}}' });
        await act(async () => {
          fromCard({ kind: "mvu-write", requestId: "q2", target: "e0", base: "r0", payload: "{}" });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        await settle();
        expect(updated[1][1]).toMatchObject({ id: "e0", vars_rev: "r9", message_vars: { stat_data: { x: 9 } } });
      });

      it("舊事件連寫第二筆：第一筆配到 id 後，在飛期間排著的那筆改送到新 id（不再送 legacy），沙盒收到搬家通知", async () => {
        const legacy = gmTurn("沒有 id 的舊回合正文", "3");
        const events = [tabled("e0", "r0", 1, "第一回合的正文"), legacy];
        await render({ events });
        await act(async () => controller!.open());
        const committed = { ...legacy, id: "new", vars_rev: "r5", message_vars: { stat_data: { x: 1 } } };
        let finishFirst!: (value: unknown) => void;
        backend.write = () => new Promise((resolve) => (finishFirst = resolve));
        let frame: { postMessage: ReturnType<typeof vi.fn> } | null = null;
        await act(async () => {
          fromCard({ kind: "mvu-write", requestId: "q1", target: "@1", base: null, payload: '{"stat_data":{"x":1}}' });
          frame = fromCard({ kind: "mvu-write", requestId: "q2", target: "@1", base: null, payload: '{"stat_data":{"x":2}}' });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        expect(backend.writes).toHaveLength(1);
        backend.write = async () => ({ status: "ok", event: { ...committed, vars_rev: "r6", message_vars: { stat_data: { x: 2 } } } });
        await act(async () => {
          finishFirst({ status: "ok", event: committed });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        await settle();
        expect(backend.writes).toHaveLength(2);
        expect(backend.writes[1]).toMatchObject({ target: { id: "new" }, expectedRev: "r5", varsJson: '{"stat_data":{"x":2}}' });
        expect(frame!.postMessage).toHaveBeenCalledWith(
          expect.objectContaining({ kind: "mvu-settle", migrate: { from: "@1", to: "new" } }),
          { targetOrigin: "*" },
        );
        expect(updated.map(([, next]) => next.vars_rev)).toEqual(["r5", "r6"]);
      });

      it("在飛時桌世代變了（同 id 備份還原）：晚到的成功結果不換進逐字稿、回 stale；殼換新 key、舊佇列關閉", async () => {
        const events = [tabled("e0", "r0", 1, "開場白"), tabled("e1", "r1", 5)];
        await render({ events });
        await act(async () => controller!.open());
        const oldKey = controller!.shellKey;
        let finish!: (value: unknown) => void;
        backend.write = () => new Promise((resolve) => (finish = resolve));
        let frame: { postMessage: ReturnType<typeof vi.fn> } | null = null;
        await act(async () => {
          frame = fromCard({ kind: "mvu-write", requestId: "q1", target: "e1", base: "r1", payload: "{}" });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        backend.generation = 4;
        await act(async () => {
          finish({ status: "ok", event: { ...events[1], vars_rev: "r2" } });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        await settle();
        expect(updated).toEqual([]);
        expect(frame!.postMessage).toHaveBeenCalledWith(
          expect.objectContaining({ kind: "mvu-settle", results: [{ requestId: "q1", ok: false, error: "stale" }] }),
          { targetOrigin: "*" },
        );
        expect(controller!.mvu!.generation).toBe(4);
        expect(controller!.shellKey).not.toBe(oldKey);
      });

      it("身分核對還在等的時候切桌：核對回來世代沒變也不換進逐字稿（新桌同 id 的那則不受影響）", async () => {
        const events = [tabled("e0", "r0", 1, "開場白"), tabled("e1", "r1", 5)];
        await render({ events });
        await act(async () => controller!.open());
        let release!: () => void;
        backend.varsHold = new Promise<void>((resolve) => (release = resolve));
        backend.write = async () => ({ status: "ok", event: { ...events[1], vars_rev: "r2" } });
        await act(async () => {
          fromCard({ kind: "mvu-write", requestId: "q1", target: "e1", base: "r1", payload: "{}" });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        expect(backend.writes).toHaveLength(1);
        // 核對卡在 card_vars_state：這時切到另一桌（同 id 的事件）
        await render({ worldId: "w2", events: [tabled("e0", "r0", 1, "開場白"), tabled("e1", "r1", 5)] });
        await act(async () => {
          release();
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        await settle();
        expect(updated).toEqual([]);
      });

      describe("非 message 層（包 2b）", () => {
        const events = () => [tabled("e0", "r0", 1, "開場白"), tabled("e1", "r1", 5)];

        it("開面板才讀各層（帶目前殼所屬卡的身分）；讀回前殼文件先不出，讀回後 mvu 帶著各層、關面板清空", async () => {
          backend.layers = [
            { key: "chat", rev: "c1", vars: '{"b":1,"a":2}' },
            { key: "character:c2", rev: null, vars: "{}" },
            { key: "script:s1", rev: "s1", vars: '{"s":1}' },
          ];
          await render({ events: events() });
          expect(backend.layerReads).toEqual([]);
          await act(async () => controller!.open());
          await settle();
          expect(backend.layerReads).toEqual([{ worldId: "w1", characterId: "c2" }]);
          expect(controller!.shellDoc).not.toBeNull();
          expect(controller!.mvu!.characterId).toBe("c2");
          expect(controller!.mvu!.layers).toEqual({
            chat: { rev: "c1", vars: { b: 1, a: 2 } },
            "character:c2": { rev: null, vars: {} },
            "script:s1": { rev: "s1", vars: { s: 1 } },
          });
          // 殼文件把各層嵌進沙盒快照
          expect(controller!.shellDoc).toContain("script:s1");
          await act(async () => controller!.close());
          expect(controller!.mvu!.layers).toEqual({});
        });

        it("寫入：送 card_layer_write（層、ID、世代、預期版本、整張表）；確認後現況換新版並回覆沙盒", async () => {
          backend.layers = [{ key: "global", rev: "g1", vars: '{"n":1}' }];
          await render({ events: events() });
          await act(async () => controller!.open());
          await settle();
          backend.layerWrite = async () => ({ status: "layer_ok", rev: "g2" });
          let frame: { postMessage: ReturnType<typeof vi.fn> } | null = null;
          await act(async () => {
            frame = fromCard({ kind: "mvu-write", requestId: "l1", target: "global", base: "g1", payload: '{"n":2}' });
            await new Promise((resolve) => setTimeout(resolve, 0));
          });
          await settle();
          expect(backend.layerWrites).toEqual([
            { worldId: "w1", layer: "global", id: null, generation: 3, expectedRev: "g1", varsJson: '{"n":2}' },
          ]);
          expect(backend.writes).toEqual([]);
          expect(controller!.mvu!.layers.global).toEqual({ rev: "g2", vars: { n: 2 } });
          expect(frame!.postMessage).toHaveBeenCalledWith(
            expect.objectContaining({ kind: "mvu-settle", results: [{ requestId: "l1", ok: true, rev: "g2" }] }),
            { targetOrigin: "*" },
          );
        });

        it("script／extension 帶原 ID（含冒號）；被拒（stale）時權威表與版本換進現況並推回沙盒", async () => {
          await render({ events: events() });
          await act(async () => controller!.open());
          await settle();
          backend.layerWrite = async () => ({ status: "stale", found: true, rev: "x9", table: '{"v":9}' });
          let frame: { postMessage: ReturnType<typeof vi.fn> } | null = null;
          await act(async () => {
            frame = fromCard({ kind: "mvu-write", requestId: "l1", target: "extension:a:b", base: null, payload: '{"v":1}' });
            await new Promise((resolve) => setTimeout(resolve, 0));
          });
          await settle();
          expect(backend.layerWrites[0]).toMatchObject({ layer: "extension", id: "a:b", expectedRev: null });
          expect(controller!.mvu!.layers["extension:a:b"]).toEqual({ rev: "x9", vars: { v: 9 } });
          expect(frame!.postMessage).toHaveBeenCalledWith(
            expect.objectContaining({
              results: [{ requestId: "l1", ok: false, error: "stale" }],
              authority: { key: "extension:a:b", table: { v: 9 }, rev: "x9" },
            }),
            { targetOrigin: "*" },
          );
        });

        it("讀取失敗不當空表：各層標 error、寫入在此狀態被擋不送後端；重開面板重讀恢復", async () => {
          backend.layersFail = true;
          await render({ events: events() });
          await act(async () => controller!.open());
          await settle();
          expect(controller!.mvu!.layers.chat).toMatchObject({ rev: null, error: expect.stringContaining("load-failed") });
          expect(controller!.mvu!.layers["script:"].error).toBeDefined();
          expect(controller!.mvu!.layers["extension:"].error).toBeDefined();
          let frame: { postMessage: ReturnType<typeof vi.fn> } | null = null;
          await act(async () => {
            frame = fromCard({ kind: "mvu-write", requestId: "l1", target: "chat", base: null, payload: "{}" });
            await new Promise((resolve) => setTimeout(resolve, 0));
          });
          await settle();
          expect(backend.layerWrites).toEqual([]);
          expect(frame!.postMessage).toHaveBeenCalledWith(
            expect.objectContaining({ results: [{ requestId: "l1", ok: false, error: "layer-unavailable" }] }),
            { targetOrigin: "*" },
          );
          // 重開面板重讀：後端好了就恢復
          backend.layersFail = false;
          backend.layers = [{ key: "chat", rev: "c1", vars: '{"a":1}' }];
          await act(async () => controller!.close());
          await act(async () => controller!.open());
          await settle();
          expect(controller!.mvu!.layers).toEqual({ chat: { rev: "c1", vars: { a: 1 } } });
        });

        it("連續寫入被擋：第一筆被拒後 error 仍在，第二筆照樣擋、都不送後端", async () => {
          backend.layersFail = true;
          await render({ events: events() });
          await act(async () => controller!.open());
          await settle();
          for (const requestId of ["l1", "l2"]) {
            await act(async () => {
              fromCard({ kind: "mvu-write", requestId, target: "chat", base: null, payload: "{}" });
              await new Promise((resolve) => setTimeout(resolve, 0));
            });
            await settle();
            expect(controller!.mvu!.layers.chat.error).toBeDefined();
          }
          expect(backend.layerWrites).toEqual([]);
        });

        it("換桌：舊桌讀回的各層失去就緒資格，新桌的讀回到了才出殼文件；讀回前 mvu 不帶舊桌的層", async () => {
          backend.layers = [{ key: "chat", rev: "a1", vars: '{"from":"A桌"}' }];
          await render({ events: events() });
          await act(async () => controller!.open());
          await settle();
          expect(controller!.shellDoc).not.toBeNull();
          let release: () => void = () => {};
          backend.layersHold = new Promise<void>((resolve) => {
            release = resolve;
          });
          backend.layers = [{ key: "chat", rev: "b1", vars: '{"from":"B桌"}' }];
          await render({ worldId: "w2" });
          // 換桌時殼先沒了、面板跟著收起；新桌的卡讀回後重開，讀各層卡在延遲的回應上
          await act(async () => controller!.open());
          expect(controller!.shellDoc).toBeNull();
          expect(controller!.mvu?.layers ?? {}).toEqual({});
          await act(async () => release());
          await settle();
          expect(controller!.shellDoc).not.toBeNull();
          expect(controller!.mvu!.layers.chat).toEqual({ rev: "b1", vars: { from: "B桌" } });
        });

        it("換角色（同桌同世代）：殼所屬身分由 A 換成 B，B 的各層讀回前殼文件為 null、mvu 不帶 A 的層；讀回後掛載 B", async () => {
          const shellCard = (id: string, tag: string): CardInterface => ({
            ...mvuCard,
            character_id: id,
            scripts: [
              {
                ...mvuCard.scripts[0],
                find_regex: `<${tag}/>`,
                replace_string: `\`\`\`html\n<!DOCTYPE html><head></head><body>${id}殼</body>\n\`\`\``,
              },
            ],
          });
          backend.cards = [shellCard("cA", "TagA"), shellCard("cB", "TagB")];
          const turn = (raw: string, gold: string): TranscriptEvent => ({ ...gmTurn("正文足夠長", gold), raw, id: raw, vars_rev: "r", message_vars: { stat_data: { 玩家: { 金幣: 1 } } } });
          backend.layers = [{ key: "chat", rev: "a1", vars: '{"from":"A"}' }];
          await render({ events: [turn("開場 <TagA/>", "1")] });
          await act(async () => controller!.open());
          await settle();
          expect(controller!.mvu!.characterId).toBe("cA");
          expect(controller!.shellDoc).not.toBeNull();
          let release: () => void = () => {};
          backend.layersHold = new Promise<void>((resolve) => {
            release = resolve;
          });
          backend.layers = [{ key: "chat", rev: "b1", vars: '{"from":"B"}' }];
          // 最新樓改由 B 卡的腳本產生殼：同桌、同世代，只有殼所屬身分變了
          await render({ events: [turn("開場 <TagA/>", "1"), turn("下一回合 <TagB/>", "2")] });
          expect(controller!.mvu!.characterId).toBe("cB");
          expect(controller!.shellDoc).toBeNull();
          expect(controller!.mvu!.layers).toEqual({});
          await act(async () => release());
          await settle();
          expect(backend.layerReads[backend.layerReads.length - 1]).toEqual({ worldId: "w1", characterId: "cB" });
          expect(controller!.shellDoc).not.toBeNull();
          expect(controller!.mvu!.layers.chat).toEqual({ rev: "b1", vars: { from: "B" } });
        });

        it("桌世代變了（整桌還原）：舊世代讀回的各層不再算就緒，新世代讀回前殼文件先不出", async () => {
          backend.layers = [{ key: "chat", rev: "a1", vars: '{"from":"舊"}' }];
          await render({ events: events() });
          await act(async () => controller!.open());
          await settle();
          expect(controller!.shellDoc).not.toBeNull();
          let release: () => void = () => {};
          backend.layersHold = new Promise<void>((resolve) => {
            release = resolve;
          });
          backend.layers = [{ key: "chat", rev: "a2", vars: '{"from":"新"}' }];
          backend.generation = 4;
          await render({ events: [...events(), player("再一句")] });
          expect(controller!.shellDoc).toBeNull();
          expect(controller!.mvu?.layers ?? {}).toEqual({});
          await act(async () => release());
          await settle();
          expect(controller!.mvu!.layers.chat).toEqual({ rev: "a2", vars: { from: "新" } });
          expect(controller!.shellDoc).not.toBeNull();
        });

        it("後端回報某層壞掉（error）：該層標錯、其他層照常", async () => {
          backend.layers = [
            { key: "chat", rev: null, vars: "{}", error: "corrupt-file: x" } as never,
            { key: "global", rev: "g1", vars: '{"n":1}' },
          ];
          await render({ events: events() });
          await act(async () => controller!.open());
          await settle();
          expect(controller!.mvu!.layers.chat.error).toBe("corrupt-file: x");
          expect(controller!.mvu!.layers.global).toEqual({ rev: "g1", vars: { n: 1 } });
        });

        it("character 層只認目前殼所屬的卡：身分不符不送後端", async () => {
          await render({ events: events() });
          await act(async () => controller!.open());
          await settle();
          let frame: { postMessage: ReturnType<typeof vi.fn> } | null = null;
          await act(async () => {
            frame = fromCard({ kind: "mvu-write", requestId: "l1", target: "character:別人", base: null, payload: "{}" });
            await new Promise((resolve) => setTimeout(resolve, 0));
          });
          await settle();
          expect(backend.layerWrites).toEqual([]);
          expect(frame!.postMessage).toHaveBeenCalledWith(
            expect.objectContaining({ results: [{ requestId: "l1", ok: false, error: "bad-target" }] }),
            { targetOrigin: "*" },
          );
        });
      });

      it("token 不對的寫入不理；關面板時未結算的寫入一律 closed", async () => {
        const events = [tabled("e0", "r0", 1, "開場白"), tabled("e1", "r1", 5)];
        await render({ events });
        await act(async () => controller!.open());
        backend.write = () => new Promise(() => {});
        await act(async () => {
          fromCard({ kind: "mvu-write", token: "別的殼", requestId: "x", target: "e1", base: "r1", payload: "{}" });
        });
        expect(backend.writes).toHaveLength(0);
        let frame: { postMessage: ReturnType<typeof vi.fn> } | null = null;
        await act(async () => {
          frame = fromCard({ kind: "mvu-write", requestId: "q1", target: "e1", base: "r1", payload: "{}" });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
        expect(backend.writes).toHaveLength(1);
        await act(async () => controller!.close());
        expect(frame!.postMessage).toHaveBeenCalledWith(
          expect.objectContaining({ kind: "mvu-settle", results: [{ requestId: "q1", ok: false, error: "closed" }] }),
          { targetOrigin: "*" },
        );
      });
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
