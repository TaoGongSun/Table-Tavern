// @vitest-environment happy-dom
// 卡片介面接到一桌：按鈕送出的句子照一般玩家句送（不碰輸入框）、卡片存的設定進存檔、接著玩時帶回來。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { playCardFromValue } from "../cards/play-card";
import { FailoverRuntime } from "../openrouter/failover";
import type { CallPlan } from "../openrouter/smart-call";
import { streamChat, type ChatMessage } from "../openrouter/stream-chat";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { restoreWebSave } from "../saves/web-save-codec";
import { useChat, type ChatController, type GameSetup } from "./useChat";

vi.mock("../openrouter/openrouter-api", () => ({ fetchFreeDaily: async () => null }));
vi.mock("../openrouter/stream-chat", () => ({ streamChat: vi.fn() }));

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  vi.restoreAllMocks();
});

const CARD = playCardFromValue("json", { spec: "chara_card_v2", spec_version: "2.0", data: { name: "瑟拉", first_mes: "爐火很旺。" } });

const session = {
  apiKey: "sk-or-v1-test",
  quota: { kind: "unknown" },
  deps: { fetch, apiBase: "http://fake/api/v1" },
  runtime: new FailoverRuntime(),
  pool: {
    refresh: async () => {},
    plan: (holds: (model: string) => boolean): CallPlan => ({ account: "acct", lineup: ["A"], others: [], fits: holds, names: new Map() }),
    contextLength: () => 65_536,
    tokenizer: () => null,
  },
  quotaEvent: () => {},
  refreshQuota: async () => {},
} as unknown as OpenRouterSession;

let chat!: ChatController;
const sent: ChatMessage[][] = [];

async function mount(game: GameSetup) {
  vi.mocked(streamChat).mockReset();
  vi.mocked(streamChat).mockImplementation(async ({ messages }) => {
    sent.push(structuredClone(messages));
    return { kind: "ok", text: "回覆", model: null, truncated: null };
  });
  function Probe() {
    chat = useChat(game, session, null);
    return null;
  }
  const root = createRoot(document.createElement("div"));
  await act(async () => root.render(<Probe />));
  return () => act(async () => root.unmount());
}

describe("card frontends on a table", () => {
  it("a card button sends its line like a player turn and leaves the input box alone", async () => {
    const unmount = await mount({ card: CARD, userName: "旅人", openingIndex: 0 });
    await act(async () => chat.setInput("還沒寫完的草稿"));
    await act(async () => chat.sendText("  推開門  "));
    expect(chat.entries.map((entry) => [entry.role, entry.text])).toEqual([
      ["char", "爐火很旺。"],
      ["user", "推開門"],
      ["char", "回覆"],
    ]);
    expect(sent[sent.length - 1].slice(-1)[0]).toMatchObject({ role: "user", content: "推開門" });
    expect(chat.input).toBe("還沒寫完的草稿");
    // 空白一句不送
    await act(async () => chat.sendText("   "));
    expect(chat.entries).toHaveLength(3);
    await unmount();
  });

  it("what the card stored goes into the save and comes back when the table is resumed", async () => {
    const unmount = await mount({ card: CARD, userName: "旅人", openingIndex: 0 });
    await act(async () => chat.setCardStorage({ theme: "dark" }));
    const save = chat.exportSave();
    expect(save.card_storage).toEqual({ theme: "dark" });
    await unmount();
    const restored = restoreWebSave(save);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    const { game } = restored;
    const unmountResumed = await mount({ ...game, resume: { entries: game.entries, local: game.local, carry: game.carry } });
    expect(chat.cardStorage).toEqual({ theme: "dark" });
    expect(chat.exportSave().card_storage).toEqual({ theme: "dark" });
    await unmountResumed();
  });
});
