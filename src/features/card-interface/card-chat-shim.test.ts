import { describe, expect, it } from "vitest";
import { buildCardChat, buildChatShimSource, scriptLiteral, type CardChat } from "./card-chat-shim";
import { buildShellDocument } from "./interface-card";

type Listener = (event: { source: unknown; data: unknown }) => void;
interface Sandbox {
  parent: object;
  listeners: Listener[];
  addEventListener: (type: string, fn: Listener) => void;
  getChatMessages: (range?: unknown, option?: unknown) => Record<string, unknown>[];
  getCurrentMessageId: () => number;
  getLastMessageId: () => number;
}

function sandbox(chat: CardChat, token = "tok"): Sandbox {
  const listeners: Listener[] = [];
  const win = {
    parent: {},
    listeners,
    addEventListener(type: string, fn: Listener) {
      if (type === "message") listeners.push(fn);
    },
  } as unknown as Sandbox;
  new Function("window", buildChatShimSource(chat, token))(win);
  return win;
}

const floors: CardChat["floors"] = [
  { name: "GM", role: "assistant", message: "開場" },
  { name: "玩家", role: "user", message: "你好" },
  { name: "系統", role: "system", message: "換幕" },
  { name: "GM", role: "assistant", message: "<Status_block>糧草: 320</Status_block>" },
  { name: "玩家", role: "user", message: "再一句" },
];
const chat: CardChat = { currentId: 3, floors };
const ids = (list: Record<string, unknown>[]) => list.map((item) => item.message_id);

describe("buildCardChat", () => {
  it("樓層照選路算好的那份原樣交出，本樓指向產生殼的那一樓", () => {
    expect(buildCardChat(floors, { id: 3, name: "GM", text: floors[3].message })).toEqual({ currentId: 3, floors });
  });

  it("空桌：開場白就是第 0 樓", () => {
    expect(buildCardChat([], { id: 0, name: "卡", text: "<UI>開場</UI>" })).toEqual({
      currentId: 0,
      floors: [{ name: "卡", role: "assistant", message: "<UI>開場</UI>" }],
    });
  });
});

describe("讀訊息墊片", () => {
  it("getCurrentMessageId 回本樓（0 也是合法樓號），getLastMessageId 回最後一樓", () => {
    const win = sandbox(chat);
    expect(win.getCurrentMessageId()).toBe(3);
    expect(win.getLastMessageId()).toBe(4);
    expect(sandbox({ currentId: 0, floors: floors.slice(0, 1) }).getCurrentMessageId()).toBe(0);
  });

  it("單一樓號、數字字串、負數深度", () => {
    const win = sandbox(chat);
    expect(win.getChatMessages(3)[0].message).toBe("<Status_block>糧草: 320</Status_block>");
    expect(ids(win.getChatMessages("3"))).toEqual([3]);
    expect(ids(win.getChatMessages(-1))).toEqual([4]);
    expect(ids(win.getChatMessages("-2"))).toEqual([3]);
  });

  it("範圍：一般、反序、負數端點、{{lastMessageId}}、超出範圍夾回現有樓層", () => {
    const win = sandbox(chat);
    expect(ids(win.getChatMessages("1-3"))).toEqual([1, 2, 3]);
    expect(ids(win.getChatMessages("3-1"))).toEqual([1, 2, 3]);
    expect(ids(win.getChatMessages("-3--1"))).toEqual([2, 3, 4]);
    expect(ids(win.getChatMessages("0-{{lastMessageId}}"))).toEqual([0, 1, 2, 3, 4]);
    expect(ids(win.getChatMessages("2-999"))).toEqual([2, 3, 4]);
    expect(ids(win.getChatMessages(999))).toEqual([4]);
    expect(ids(win.getChatMessages(-999))).toEqual([0]);
    // 巨大端點不逐號迴圈：夾進現有樓層後最多掃五樓
    expect(ids(win.getChatMessages("0-99999999999999"))).toEqual([0, 1, 2, 3, 4]);
  });

  it("null／undefined 照酒館拋 TypeError", () => {
    const win = sandbox(chat);
    expect(() => win.getChatMessages(null)).toThrow(TypeError);
    expect(() => win.getChatMessages(undefined)).toThrow(TypeError);
    expect(() => win.getChatMessages()).toThrow(TypeError);
  });

  it("符合格式的超長整數照酒館夾回：正數落最後一樓、負數落第 0 樓", () => {
    const win = sandbox(chat);
    expect(ids(win.getChatMessages("9".repeat(400)))).toEqual([4]);
    expect(ids(win.getChatMessages("-" + "9".repeat(400)))).toEqual([0]);
    expect(ids(win.getChatMessages(`0-${"9".repeat(400)}`))).toEqual([0, 1, 2, 3, 4]);
    expect(ids(win.getChatMessages(`-${"9".repeat(400)}-1`))).toEqual([0, 1]);
  });

  it.each([
    ["false", false],
    ["空字串", ""],
    ["空陣列", []],
    ["物件", {}],
    ["NaN", NaN],
    ["Infinity", Infinity],
    ["小數", 1.5],
    ["小數字串", "1.5"],
    ["文字", "abc"],
    ["帶空白", " 1 - 2 "],
  ])("非法 range（%s）回空陣列", (_label, range) => {
    expect(sandbox(chat).getChatMessages(range)).toEqual([]);
  });

  it("篩選：role、hide_state；不合法的選項回空陣列", () => {
    const win = sandbox(chat);
    expect(ids(win.getChatMessages("0-4", { role: "user" }))).toEqual([1, 4]);
    expect(ids(win.getChatMessages("0-4", { role: "system" }))).toEqual([2]);
    expect(ids(win.getChatMessages("0-4", { role: "assistant" }))).toEqual([0, 3]);
    expect(ids(win.getChatMessages("0-4", { hide_state: "unhidden" }))).toHaveLength(5);
    expect(win.getChatMessages("0-4", { hide_state: "hidden" })).toEqual([]);
    expect(win.getChatMessages("0-4", { role: "narrator" })).toEqual([]);
    expect(win.getChatMessages("0-4", { hide_state: "maybe" })).toEqual([]);
    expect(ids(win.getChatMessages("0-4", "不是物件"))).toHaveLength(5);
  });

  it("兩種回傳形狀：一般與 include_swipes", () => {
    const win = sandbox(chat);
    expect(win.getChatMessages(1)[0]).toEqual({
      message_id: 1,
      name: "玩家",
      role: "user",
      is_hidden: false,
      message: "你好",
      data: {},
      extra: {},
      swipe_id: 0,
      swipes: ["你好"],
      swipes_data: [{}],
    });
    expect(win.getChatMessages(1, { include_swipes: true })[0]).toEqual({
      message_id: 1,
      name: "玩家",
      role: "user",
      is_hidden: false,
      swipe_id: 0,
      swipes: ["你好"],
      swipes_data: [{}],
      swipes_info: [{}],
    });
  });

  it("回傳的是全新物件：卡片改了也不影響下一次", () => {
    const win = sandbox(chat);
    const first = win.getChatMessages(3);
    first[0].message = "被改掉";
    (first[0].swipes as string[]).push("x");
    expect(win.getChatMessages(3)[0].message).toBe("<Status_block>糧草: 320</Status_block>");
    expect(win.getChatMessages(3)[0].swipes).toEqual(["<Status_block>糧草: 320</Status_block>"]);
  });

  it("推送：只認真 parent、對的 token 與正確形狀", () => {
    const win = sandbox(chat, "tok");
    const parent = win.parent;
    const next: CardChat = { currentId: 0, floors: [{ name: "GM", role: "assistant", message: "新的" }] };
    const send = (source: unknown, data: unknown) => win.listeners.forEach((fn) => fn({ source, data }));

    send({}, { source: "table-tavern-host", kind: "chat", token: "tok", chat: next });
    send(parent, { source: "table-tavern-host", kind: "chat", token: "別的殼", chat: next });
    send(parent, { source: "table-tavern-host", kind: "other", token: "tok", chat: next });
    send(parent, { source: "table-tavern-host", kind: "chat", token: "tok", chat: { currentId: "0", floors: [] } });
    send(parent, {
      source: "table-tavern-host",
      kind: "chat",
      token: "tok",
      chat: { currentId: 0, floors: [{ name: "GM", role: "narrator", message: "x" }] },
    });
    expect(win.getCurrentMessageId()).toBe(3);

    send(parent, { source: "table-tavern-host", kind: "chat", token: "tok", chat: next });
    expect(win.getCurrentMessageId()).toBe(0);
    expect(win.getChatMessages(0)[0].message).toBe("新的");
    // 推來的物件之後被宿主改動也不影響沙盒裡的資料
    next.floors[0].message = "宿主改了";
    expect(win.getChatMessages(0)[0].message).toBe("新的");
  });

  it("仿讀本樓卡片的寫法：拿到本樓原文", () => {
    const win = sandbox(chat);
    const readCurrent = new Function(
      "getCurrentMessageId",
      "getChatMessages",
      `const id = getCurrentMessageId();
       if (!id && id !== 0) return null;
       const data = getChatMessages(id);
       const message = Array.isArray(data) ? data[0] : data;
       const match = message.message.match(/<Status_block>([\\s\\S]*?)<\\/Status_block>/i);
       return match ? match[1].trim() : "";`,
    );
    expect(readCurrent(win.getCurrentMessageId, win.getChatMessages)).toBe("糧草: 320");
  });
});

describe("嵌入跳脫", () => {
  const tricky = `引號"與'、反斜線\\、</ScRiPt><script>alert(1)</script>、&amp;、\u2028\u2029、emoji 🐎`;

  it("scriptLiteral 逐字 round-trip，文字裡沒有可關掉 script 的字元", () => {
    const literal = scriptLiteral({ message: tricky });
    expect(literal).not.toMatch(/<|>|&|\u2028|\u2029/);
    expect(new Function(`return ${literal};`)()).toEqual({ message: tricky });
  });

  it("buildShellDocument：讀訊息墊片排在橋接墊片前面，原文含 </ScRiPt> 也不破文件", () => {
    const doc = buildShellDocument("<html><head></head><body>殼</body></html>", {}, {
      chat: { currentId: 0, floors: [{ name: "GM", role: "assistant", message: tricky }] },
      token: "tok",
    });
    expect(doc.indexOf("getChatMessages")).toBeLessThan(doc.indexOf("__ttHost"));
    // 只有墊片自己的 script 結尾（內建庫三支＋讀訊息＋橋接），卡片文字沒有多關一支
    const closings = doc.match(/<\/script>/gi) ?? [];
    expect(closings).toHaveLength(5);
  });

  it("沒有讀訊息快照時不定義這三支", () => {
    const doc = buildShellDocument("<html><head></head><body>殼</body></html>");
    expect(doc).not.toContain("getChatMessages");
    expect(doc).not.toContain("getCurrentMessageId");
  });
});
