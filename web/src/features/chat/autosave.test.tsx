// @vitest-environment happy-dom
// 自動存檔：新開的桌等玩家第一次開口才佔存檔、回合結束後存、從存檔接著玩的桌沿用同一格；
// 存下來的就是契約 v1 的網頁存檔（玩家句裡的 setvar 落在 MVU 的 chat 層）。
import { IDBFactory } from "fake-indexeddb";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { parseWebSave } from "@desktop/shared/contracts/web-save/web-save";
import { SAMPLE_PLAY_CARD } from "../cards/sample-card";
import { failureNetwork } from "../openrouter/api-failure";
import { FailoverRuntime } from "../openrouter/failover";
import type { CallPlan } from "../openrouter/smart-call";
import { streamChat } from "../openrouter/stream-chat";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { MemoryStorage } from "../openrouter/memory-storage";
import { openSaveStore, restoreGlobals, type SaveStore } from "../saves/save-store";
import { EMPTY_CARRY, restoreWebSave } from "../saves/web-save-codec";
import { GLOBAL_VARIABLES, useChat, type ChatController, type GameSetup } from "./useChat";

vi.mock("../openrouter/openrouter-api", () => ({ fetchFreeDaily: async () => null }));
vi.mock("../openrouter/stream-chat", () => ({ streamChat: vi.fn() }));

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

const plan: CallPlan = { account: "acct", lineup: ["A"], others: [], fits: (model) => model === "A", names: new Map() };
const session = {
  apiKey: "sk-or-v1-test",
  quota: { kind: "unknown" },
  deps: { fetch, apiBase: "http://fake/api/v1" },
  runtime: new FailoverRuntime(),
  pool: { refresh: async () => {}, plan: () => plan, contextLength: () => 65536, tokenizer: () => null },
  quotaEvent: () => {},
  refreshQuota: async () => {},
} as unknown as OpenRouterSession;

let root: Root | null = null;
let chat!: ChatController;

async function mount(game: GameSetup, saves: SaveStore) {
  function Probe() {
    chat = useChat(game, session, saves);
    return null;
  }
  root = createRoot(document.createElement("div"));
  await act(async () => root!.render(<Probe />));
}

const settle = () => act(async () => new Promise((resolve) => setTimeout(resolve, 20)));

afterEach(async () => {
  await act(async () => root?.unmount());
  root = null;
});

describe("自動存檔", () => {
  it("新開的桌：只有開場白不佔存檔；玩家開口、回合結束後存成契約 v1", async () => {
    vi.mocked(streamChat).mockResolvedValue({ kind: "ok", text: "雪還在下。", model: null, truncated: null });
    const saves = openSaveStore(new IDBFactory())!;
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, saves);
    await settle();
    expect(await saves.list()).toEqual([]);

    await act(async () => chat.setInput("我付了錢{{setvar::錢包::15}}"));
    await act(async () => chat.send());
    await settle();
    const [meta] = await saves.list();
    expect(meta).toMatchObject({ title: "瑟拉", messageCount: 3 });
    const save = await saves.get(meta.id);
    expect(parseWebSave(JSON.stringify(save)).ok).toBe(true);
    expect(save?.messages.map((message) => [message.role, message.text])).toEqual([
      ["char", SAMPLE_PLAY_CARD.openings[0]],
      ["user", "我付了錢"],
      ["char", "雪還在下。"],
    ]);
    expect(save?.messages[0].opening).toBe(true);
    expect(save?.mvu?.layers.chat).toEqual({ 錢包: "15" });

    // 再改一次（刪掉最後一則）：同一格覆寫
    await act(async () => chat.deleteLast());
    await settle();
    const list = await saves.list();
    expect(list).toHaveLength(1);
    expect((await saves.get(meta.id))?.messages).toHaveLength(2);
  });

  it("從存檔接著玩：沿用同一格，逐字稿與變數接得上", async () => {
    vi.mocked(streamChat).mockResolvedValue({ kind: "ok", text: "第二句回覆", model: null, truncated: null });
    const saves = openSaveStore(new IDBFactory())!;
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, saves);
    await act(async () => chat.setInput("你好{{setvar::錢包::15}}"));
    await act(async () => chat.send());
    await settle();
    const [meta] = await saves.list();
    await act(async () => root?.unmount());

    const restored = restoreWebSave((await saves.get(meta.id))!);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    const { game } = restored;
    await mount(
      { card: game.card, userName: game.userName, openingIndex: game.openingIndex, saveId: meta.id, resume: { entries: game.entries, local: game.local, carry: game.carry } },
      saves,
    );
    expect(chat.entries.map((entry) => entry.text)).toEqual(game.entries.map((entry) => entry.text));
    expect(chat.setup.variables.local.get("錢包")).toBe(15);
    await act(async () => chat.setInput("再一句"));
    await act(async () => chat.send());
    await settle();
    const list = await saves.list();
    expect(list.map((item) => item.id)).toEqual([meta.id]);
    expect((await saves.get(meta.id))?.messages.map((message) => message.text).slice(-2)).toEqual(["再一句", "第二句回覆"]);
  });

  it("同一格的寫入照順序：前一筆卡住時，晚到的舊回合不會蓋掉新回合", async () => {
    vi.mocked(streamChat).mockResolvedValue({ kind: "ok", text: "回覆", model: null, truncated: null });
    const real = openSaveStore(new IDBFactory())!;
    let release = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    let calls = 0;
    const slow: SaveStore = {
      ...real,
      // 第一筆是開口前的首格；第二筆（較舊的回合）卡到放行為止才真的寫
      put: async (meta, save) => {
        calls += 1;
        if (calls === 2) await gate;
        return real.put(meta, save);
      },
    };
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, slow);
    await act(async () => chat.setInput("你好"));
    await act(async () => chat.send());
    await settle();
    await act(async () => chat.deleteLast());
    await settle();
    release();
    await settle();
    const [meta] = await real.list();
    expect((await real.get(meta.id))?.messages).toHaveLength(2);
  });

  it("組出的存檔不合契約（例如壞掉的時間）：不寫進存檔庫、標自動存檔失敗", async () => {
    const saves = openSaveStore(new IDBFactory())!;
    const entries = [{ id: "m0", role: "user" as const, text: "你好", ts: "2026-99-99T99:99:99Z" }];
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: null, resume: { entries, local: {}, carry: EMPTY_CARRY } }, saves);
    await settle();
    expect(chat.saveFailed).toBe(true);
    expect(await saves.list()).toEqual([]);
  });

  it("組存檔時丟錯（JSON 帶不走的值）：落到自動存檔失敗，不讓例外跑出去", async () => {
    const saves = openSaveStore(new IDBFactory())!;
    const entries = [{ id: "m0", role: "user" as const, text: "你好" }];
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: null, resume: { entries, local: { big: 1n }, carry: EMPTY_CARRY } }, saves);
    await settle();
    expect(chat.saveFailed).toBe(true);
    expect(await saves.list()).toEqual([]);
  });

  it("first slot must land before the first dispatch", async () => {
    vi.mocked(streamChat).mockReset();
    vi.mocked(streamChat).mockResolvedValue({ kind: "ok", text: "回覆", model: null, truncated: null });
    const real = openSaveStore(new IDBFactory())!;
    let release = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    let calls = 0;
    const slow: SaveStore = {
      ...real,
      put: async (meta, save) => {
        calls += 1;
        if (calls === 1) await gate;
        return real.put(meta, save);
      },
    };
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, slow);
    await act(async () => chat.setInput("第一句"));
    await act(async () => void chat.send());
    await settle();
    // 首格還沒寫成：模型一發都沒送
    expect(vi.mocked(streamChat)).not.toHaveBeenCalled();
    expect(await real.list()).toEqual([]);
    release();
    await settle();
    expect(vi.mocked(streamChat)).toHaveBeenCalledTimes(1);
    const [meta] = await real.list();
    expect(meta).toBeDefined();
  });

  it("首格存不進去：不送模、說明原因、原句留在輸入框，下次開口再試", async () => {
    vi.mocked(streamChat).mockReset();
    vi.mocked(streamChat).mockResolvedValue({ kind: "ok", text: "回覆", model: null, truncated: null });
    const real = openSaveStore(new IDBFactory())!;
    let fail = true;
    const flaky: SaveStore = {
      ...real,
      put: async (meta, save) => {
        if (fail) throw new Error("QuotaExceededError");
        return real.put(meta, save);
      },
    };
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, flaky);
    await act(async () => chat.setInput("第一句"));
    await act(async () => chat.send());
    await settle();
    expect(vi.mocked(streamChat)).not.toHaveBeenCalled();
    expect(chat.error).toMatch(/存不進瀏覽器/);
    expect(chat.input).toBe("第一句");
    expect(chat.entries.map((entry) => entry.role)).toEqual(["char"]);
    fail = false;
    await act(async () => chat.send());
    await settle();
    expect(vi.mocked(streamChat)).toHaveBeenCalledTimes(1);
  });

  it("a late older save success never clears a newer failure", async () => {
    vi.mocked(streamChat).mockResolvedValue({ kind: "ok", text: "回覆", model: null, truncated: null });
    const real = openSaveStore(new IDBFactory())!;
    let release = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    let calls = 0;
    const slow: SaveStore = {
      ...real,
      put: async (meta, save) => {
        calls += 1;
        if (calls === 2) await gate;
        return real.put(meta, save);
      },
    };
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, slow);
    await act(async () => chat.setInput("你好"));
    await act(async () => chat.send());
    await settle();
    // 最新一次：改成超出存檔上限的訊息 → 存不了
    await act(async () => chat.editLast("a".repeat(2 * 1024 * 1024 + 1)));
    await settle();
    expect(chat.saveFailed).toBe(true);
    // 卡住的舊筆後來寫成功：提示不能因此消失（最新內容其實沒存到）
    release();
    await settle();
    expect(chat.saveFailed).toBe(true);
  });

  it("reload mid-stream restores the last complete turn and the pending input", async () => {
    vi.stubGlobal("sessionStorage", new MemoryStorage());
    const saves = openSaveStore(new IDBFactory())!;
    // 第一回合完整結束，第二回合串流到一半就重新整理
    vi.mocked(streamChat)
      .mockResolvedValueOnce({ kind: "ok", text: "第一回合的回覆", model: null, truncated: null })
      .mockImplementationOnce(() => new Promise(() => {}));
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, saves);
    await act(async () => chat.setInput("第一句"));
    await act(async () => chat.send());
    await settle();
    await act(async () => chat.setInput("我想借宿一晚{{setvar::住宿::1}}"));
    await act(async () => void chat.send());
    await settle();
    expect(chat.busy).toBe(true);
    await act(async () => root?.unmount());
    root = null;

    const [meta] = await saves.list();
    const save = (await saves.get(meta.id))!;
    expect(save.messages.map((message) => message.text)).toEqual([SAMPLE_PLAY_CARD.openings[0], "第一句", "第一回合的回覆"]);
    const restored = restoreWebSave(save);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    const { game } = restored;
    await mount(
      { card: game.card, userName: game.userName, openingIndex: game.openingIndex, saveId: meta.id, resume: { entries: game.entries, local: game.local, carry: game.carry } },
      saves,
    );
    expect(chat.input).toBe("我想借宿一晚{{setvar::住宿::1}}");
    expect(chat.entries.map((entry) => entry.text)).toEqual(save.messages.map((message) => message.text));
    expect(sessionStorage.getItem(`tt-web:pending-input:${meta.id}`)).toBeNull();
    vi.unstubAllGlobals();
  });

  it.each(["beforeunload", "pagehide"])("a reload cuts the request after %s: that failure is not the turn's end, the draft stays and nothing half-done is saved", async (leaving) => {
    vi.stubGlobal("sessionStorage", new MemoryStorage());
    const saves = openSaveStore(new IDBFactory())!;
    let fail!: () => void;
    vi.mocked(streamChat).mockImplementationOnce(
      () => new Promise((resolve) => (fail = () => resolve({ kind: "failed", failure: failureNetwork("Load failed", false) }))),
    );
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, saves);
    await act(async () => chat.setInput("說到一半就重新整理"));
    await act(async () => void chat.send());
    await settle();
    const [meta] = await saves.list();
    // 瀏覽器先發離開事件，請求隨即被中斷、腳本還跑得到失敗結果
    await act(async () => {
      window.dispatchEvent(leaving === "pagehide" ? new PageTransitionEvent("pagehide", { persisted: false }) : new Event("beforeunload"));
      fail();
    });
    await settle();
    expect(sessionStorage.getItem(`tt-web:pending-input:${meta.id}`)).toBe("說到一半就重新整理");
    expect(chat.input).toBe("");
    // 存檔停在上一個完整回合（新桌：開口前只有開場白），半截回合的玩家句不存進去
    const save = (await saves.get(meta.id))!;
    expect(save.messages.map((message) => message.role)).toEqual(["char"]);
    vi.unstubAllGlobals();
  });

  it("the draft is cleared only once the save holding that turn is written; leaving before that keeps it", async () => {
    vi.stubGlobal("sessionStorage", new MemoryStorage());
    vi.mocked(streamChat).mockResolvedValueOnce({ kind: "ok", text: "回覆落地了", model: null, truncated: null });
    const real = openSaveStore(new IDBFactory())!;
    let release = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    let calls = 0;
    // 第一筆是開口前的首格；第二筆（含這一回合）卡到放行為止
    const slow: SaveStore = {
      ...real,
      put: async (meta, save) => {
        calls += 1;
        if (calls === 2) await gate;
        return real.put(meta, save);
      },
    };
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, slow);
    await act(async () => chat.setInput("落地前離開"));
    await act(async () => chat.send());
    await settle();
    expect(chat.entries[chat.entries.length - 1]?.text).toBe("回覆落地了");
    const [meta] = await real.list();
    const key = `tt-web:pending-input:${meta.id}`;
    // 回合已落地、存檔還沒寫成就離開：存檔仍是上一回合，草稿要留著
    await act(async () => {
      window.dispatchEvent(new Event("beforeunload"));
    });
    expect(sessionStorage.getItem(key)).toBe("落地前離開");
    expect((await real.get(meta.id))?.messages.map((message) => message.role)).toEqual(["char"]);
    // 那一筆寫成了（存檔含這一回合），草稿才清
    release();
    await settle();
    expect((await real.get(meta.id))?.messages.map((message) => message.text).slice(-1)).toEqual(["回覆落地了"]);
    expect(sessionStorage.getItem(key)).toBeNull();
    vi.unstubAllGlobals();
  });

  it("an older turn's save finishing late does not clear the next turn's draft", async () => {
    vi.stubGlobal("sessionStorage", new MemoryStorage());
    vi.mocked(streamChat)
      .mockResolvedValueOnce({ kind: "ok", text: "第一回合的回覆", model: null, truncated: null })
      .mockImplementationOnce(() => new Promise(() => {}));
    const real = openSaveStore(new IDBFactory())!;
    let release = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    let calls = 0;
    const slow: SaveStore = {
      ...real,
      put: async (meta, save) => {
        calls += 1;
        if (calls === 2) await gate;
        return real.put(meta, save);
      },
    };
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, slow);
    await act(async () => chat.setInput("第一句"));
    await act(async () => chat.send());
    await settle();
    // 第一回合的存檔還卡著，玩家已經送出第二句（串流中）
    await act(async () => chat.setInput("第二句"));
    await act(async () => void chat.send());
    await settle();
    const [meta] = await real.list();
    const key = `tt-web:pending-input:${meta.id}`;
    expect(sessionStorage.getItem(key)).toBe("第二句");
    release();
    await settle();
    expect(sessionStorage.getItem(key)).toBe("第二句");
    vi.unstubAllGlobals();
  });

  it("新開的桌第一回合就重新整理：開口前的樣子已佔一格，那一句放回輸入框", async () => {
    vi.stubGlobal("sessionStorage", new MemoryStorage());
    const saves = openSaveStore(new IDBFactory())!;
    vi.mocked(streamChat).mockImplementationOnce(() => new Promise(() => {}));
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, saves);
    await act(async () => chat.setInput("第一句"));
    await act(async () => void chat.send());
    await settle();
    await act(async () => root?.unmount());
    root = null;
    const [meta] = await saves.list();
    const save = (await saves.get(meta.id))!;
    expect(save.messages.map((message) => message.role)).toEqual(["char"]);
    const restored = restoreWebSave(save);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    await mount(
      { ...restored.game, saveId: meta.id, resume: { entries: restored.game.entries, local: restored.game.local, carry: restored.game.carry } },
      saves,
    );
    expect(chat.input).toBe("第一句");
    vi.unstubAllGlobals();
  });

  it("跨對話 global 存進瀏覽器：重新整理後還在，另一張存檔讀得到同一把", async () => {
    vi.mocked(streamChat).mockResolvedValue({ kind: "ok", text: "回覆", model: null, truncated: null });
    const factory = new IDBFactory();
    const saves = openSaveStore(factory)!;
    // 開站讀回 global（讀成功之後才准寫回）
    await restoreGlobals(saves, GLOBAL_VARIABLES);
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, saves);
    await act(async () => chat.setInput("你好{{setglobalvar::名聲::5}}"));
    await act(async () => chat.send());
    await settle();
    await act(async () => root?.unmount());
    root = null;

    // 重新整理：分頁的 global 清空，開站時從存檔庫讀回
    for (const key of Object.keys(GLOBAL_VARIABLES)) delete GLOBAL_VARIABLES[key];
    await restoreGlobals(openSaveStore(factory), GLOBAL_VARIABLES);
    expect(GLOBAL_VARIABLES.名聲).toBe("5");

    // 另開一桌（另一格存檔）讀得到同一把；這桌沒寫過，匯出不帶它
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, saves);
    expect(chat.setup.variables.global.get("名聲")).toBe(5);
    expect(chat.exportSave().mvu).toBeNull();
  });

  it("a global snapshot taken before a successful load is discarded even after a retry", async () => {
    vi.mocked(streamChat).mockResolvedValue({ kind: "ok", text: "回覆", model: null, truncated: null });
    const factory = new IDBFactory();
    const seeded = openSaveStore(factory)!;
    await seeded.loadGlobal({});
    await seeded.reserveGlobalWrite()({ kept: "old" });
    for (const key of Object.keys(GLOBAL_VARIABLES)) delete GLOBAL_VARIABLES[key];

    // 開站讀 global 失敗；第一筆是開口前的首格，第二筆（回合結束、取樣時還沒載入）卡到放行為止
    const real = openSaveStore(factory)!;
    let loadFails = true;
    let release = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    let calls = 0;
    const store: SaveStore = {
      ...real,
      loadGlobal: async (target) => {
        if (loadFails) throw new Error("transient");
        return real.loadGlobal(target);
      },
      put: async (meta, save) => {
        calls += 1;
        if (calls === 2) await gate;
        return real.put(meta, save);
      },
    };
    expect(await restoreGlobals(store, GLOBAL_VARIABLES)).toBe(false);
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, store);
    await act(async () => chat.setInput("你好{{setglobalvar::名聲::5}}"));
    await act(async () => chat.send());
    await settle();
    expect(calls).toBe(2);

    // 重試讀回成功（分頁補上 kept），才放行重試前取樣的那一筆：它少了 kept，不准寫
    loadFails = false;
    expect(await restoreGlobals(store, GLOBAL_VARIABLES)).toBe(true);
    expect(GLOBAL_VARIABLES).toEqual({ kept: "old", 名聲: "5" });
    release();
    await settle();
    expect(await openSaveStore(factory)!.getGlobal()).toEqual({ kept: "old" });

    // 載入之後取樣的照常寫回，庫裡的鍵都在
    await act(async () => chat.setInput("再一句"));
    await act(async () => chat.send());
    await settle();
    expect(await openSaveStore(factory)!.getGlobal()).toEqual({ kept: "old", 名聲: "5" });
    for (const key of Object.keys(GLOBAL_VARIABLES)) delete GLOBAL_VARIABLES[key];
  });

  it("瀏覽器不讓存：標出自動存檔失敗", async () => {
    vi.mocked(streamChat).mockResolvedValue({ kind: "ok", text: "回覆", model: null, truncated: null });
    const failing: SaveStore = {
      list: async () => [],
      get: async () => null,
      put: async () => {
        throw new Error("QuotaExceededError");
      },
      remove: async () => {},
      getGlobal: async () => ({}),
      loadGlobal: async () => {},
      reserveGlobalWrite: () => async () => {},
    };
    await mount({ card: SAMPLE_PLAY_CARD, userName: "旅人", openingIndex: 0 }, failing);
    await act(async () => chat.setInput("你好"));
    await act(async () => chat.send());
    await settle();
    expect(chat.saveFailed).toBe(true);
  });
});
