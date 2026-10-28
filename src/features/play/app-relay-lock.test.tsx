// @vitest-environment happy-dom
// App 層：「送出並接話」（B）與「讓 X 接話」（C）走 AppWorkspace 實際傳給 PlayView 的 handler，
// 換桌鎖持有中、桌型還沒讀回時都不進回合（零玩家落檔、零點名列、零模型呼叫），解除後照常；
// 主區名單一變就重讀「會不會點名」（隱藏卡被帶出場可能帶進卡片自帶格式）。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { TranscriptEvent } from "../../shared/contracts/backend-contracts";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));
const dialogs = vi.hoisted(() => ({
  confirm: vi.fn(async (_text?: string, _options?: unknown): Promise<boolean> => true),
  message: vi.fn(async () => {}),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    backend.calls.push({ command, args });
    const handler = backend.handlers[command];
    return handler ? handler(args) : null;
  }),
  Channel: class {
    onmessage: ((value: unknown) => void) | null = null;
  },
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "0.2.0") }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: dialogs.confirm,
  message: dialogs.message,
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));
// 保留真的 PlayView，只記下最近一次 render 收到的 props：直接呼叫 handler，繞過停用的按鈕驗 App 那層的鎖
const view = vi.hoisted(() => ({ props: null as Record<string, unknown> | null }));
vi.mock("./PlayView", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./PlayView")>();
  const { createElement } = await import("react");
  return {
    ...actual,
    PlayView: (props: Parameters<typeof actual.PlayView>[0]) => {
      view.props = props as unknown as Record<string, unknown>;
      return createElement(actual.PlayView, props);
    },
  };
});

// 換桌鎖：直接呼叫陣容欄拿到的「撤銷上次匯入」handler（已聊過時選單項不給，handler 本身仍取鎖、開確認框）
const rail = vi.hoisted(() => ({ props: null as Record<string, unknown> | null }));
vi.mock("../characters/CastRail", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../characters/CastRail")>();
  const { createElement } = await import("react");
  return {
    ...actual,
    CastRail: (props: Parameters<typeof actual.CastRail>[0]) => {
      rail.props = props as unknown as Record<string, unknown>;
      return createElement(actual.CastRail, props);
    },
  };
});

import App from "../../App";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const actor = (id: string, name: string, autoHidden = false) => ({
  id,
  name,
  color: "#888",
  avatar: "🎭",
  tier: "balanced",
  show_image: false,
  archived: false,
  auto_hidden: autoHidden,
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((ok) => {
    resolve = ok;
  });
  return { promise, resolve };
}

describe("B／C 入口在 App 層", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let mode: () => unknown;
  let arrived: string[];

  beforeEach(() => {
    let id = 0;
    mode = () => ({ gm_only: false, nominates: true });
    arrived = [];
    const appeared = new Set<string>();
    const h: Record<string, (args: Record<string, unknown>) => unknown> = {};
    h.read_config = () => ({
      api_keys: { openrouter: "key" },
      tier_models: {},
      preferences: { language: "zh-TW", transport: "api" },
    });
    h.list_worlds = () => [{ id: "w1", name: "酒館", read_only: false, needs_repair: false }];
    h.open_world = () => ({ status: "ready" });
    h.read_state = () => ({ state: {}, current_scene: 0, scene_titles: {}, scene_labels: {}, player_card_id: null });
    h.read_transcript = () => [];
    h.list_characters = () => [actor("c1", "狐狸"), actor("c2", "怪盜", true)];
    h.read_character = ({ characterId }) => ({
      ...actor(characterId as string, "狐狸"),
      public_md: "",
      private_md: "",
      gen_prompt: "",
    });
    h.scene_appearances = () => ({ character_ids: [...appeared], person_titles: [] });
    h.read_world_md = () => "";
    h.read_worldbook = () => [];
    h.list_import_receipts = () => [
      { kind: "character", label: "狐狸", timestamp: "2026-10-28 10:00", character_id: "c1" },
    ];
    h.world_has_state_bar = () => false;
    h.reclaim_world_if_empty = () => false;
    h.smart_free_new_models = () => [];
    h.card_interfaces = () => [];
    h.branch_bindings = () => [];
    h.mechanism_ledger = () => ({ entries: [], rejected: 0, clamped: 0, errors: 0, jumps: 0 });
    h.gm_table_mode = () => mode();
    h.append_player_event = (args) => ({ event: { ...(args.event as object), id: `p${++id}` }, offset: 1 });
    h.append_transcript = (args) => ({ ...(args.event as object), id: `e${++id}` });
    h.gm_narrate = () => {
      arrived.forEach((cid) => appeared.add(cid));
      return { text: "門開了。", raw: null, next: "c1", arrived_characters: arrived };
    };
    h.chat_with_character = () => ({ text: "嗨", raw: null, aborted: false });
    backend.handlers = h;
    backend.calls = [];
    dialogs.confirm.mockReset();
    dialogs.confirm.mockResolvedValue(true);
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
    view.props = null;
    document.body.innerHTML = "";
    vi.unstubAllGlobals();
  });

  async function settle(rounds = 3) {
    for (let i = 0; i < rounds; i += 1) {
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    }
  }

  async function start() {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root!.render(<App />));
    await settle();
    await act(async () =>
      (document.querySelector(`[aria-label="${t("lobbyEnterTable", { name: "酒館" })}"]`) as HTMLElement).click(),
    );
    await settle();
    // 發言對象選 GM
    await act(async () => (document.querySelector('.rail-tile-main[aria-label="GM"]') as HTMLElement).click());
    await settle(1);
  }

  const called = (command: string) => backend.calls.filter((call) => call.command === command);
  const gmCalls = () =>
    called("append_transcript").filter((call) => (call.args.event as TranscriptEvent).marker?.type === "gm_call");
  const props = () => view.props as {
    relayable: boolean;
    nominated: unknown;
    onInputChange: (value: string) => void;
    onSendRelay: () => void;
    onCallNominated: () => void;
    castIds: string[];
    onSubmit: (event: { preventDefault(): void }) => void;
  };

  // 「GM 推進」只在確定不點名時藏〔作者裁決 2026-10-28〕
  const advanceShown = () =>
    [...document.querySelectorAll("button")].some((button) => button.textContent === t("gmAdvance"));

  async function type(text: string) {
    await act(async () => props().onInputChange(text));
  }

  async function run(handler: () => void) {
    await act(async () => handler());
    await settle();
  }

  const nothingRan = () => {
    expect(called("append_player_event")).toHaveLength(0);
    expect(called("gm_narrate")).toHaveLength(0);
    expect(called("chat_with_character")).toHaveLength(0);
    expect(gmCalls()).toHaveLength(0);
  };

  it("桌型還沒讀回：B、C 都不進回合；讀回後 B 照常", async () => {
    const pending = deferred<unknown>();
    mode = () => pending.promise;
    await start();
    expect(props().relayable).toBe(false);
    expect(advanceShown()).toBe(true);
    await type("我推門");
    await run(() => props().onSendRelay());
    await run(() => props().onCallNominated());
    nothingRan();

    await act(async () => pending.resolve({ gm_only: false, nominates: true }));
    await settle();
    expect(props().relayable).toBe(true);
    expect(advanceShown()).toBe(true);
    await run(() => props().onSendRelay());
    expect(called("gm_narrate")).toHaveLength(1);
    expect(called("chat_with_character")).toHaveLength(1);
  });

  it("換桌鎖持有中（撤銷匯入的確認框開著）：B、C 都不進回合；放鎖後照常", async () => {
    await start();
    // 先有一則點名旁白，C 才有東西可接
    await type("我推門");
    await act(async () => props().onSubmit({ preventDefault() {} }));
    await settle();
    expect(props().nominated).not.toBeNull();
    backend.calls = [];

    const answer = deferred<boolean>();
    dialogs.confirm.mockImplementationOnce(() => answer.promise);
    await act(async () => (rail.props!.onUndoImport as () => void)());
    await settle();
    expect(dialogs.confirm).toHaveBeenCalledTimes(1);

    await type("再推");
    await run(() => props().onSendRelay());
    await run(() => props().onCallNominated());
    nothingRan();

    await act(async () => answer.resolve(false));
    await settle();
    await run(() => props().onCallNominated());
    expect(gmCalls()).toHaveLength(1);
    expect(called("chat_with_character")).toHaveLength(1);
    await type("再推");
    await run(() => props().onSendRelay());
    expect(called("gm_narrate")).toHaveLength(1);
    expect(called("chat_with_character")).toHaveLength(2);
  });

  it("旁白帶出隱藏卡、主區名單變了就重讀；新結果不點名時 B 與 GM 推進都藏起來", async () => {
    await start();
    expect(props().relayable).toBe(true);
    expect(advanceShown()).toBe(true);
    const before = called("gm_table_mode").length;
    arrived = ["c2"];
    mode = () => ({ gm_only: false, nominates: false });
    await type("我推門");
    await act(async () => props().onSubmit({ preventDefault() {} }));
    await settle();
    expect(called("gm_table_mode").length).toBeGreaterThan(before);
    expect(props().relayable).toBe(false);
    expect(advanceShown()).toBe(false);
  });

  it("點名後那張卡被切成舞台卡：「讓 X 接話」不出現，硬呼叫也不寫點名列、不接話", async () => {
    await start();
    await type("我推門");
    await act(async () => props().onSubmit({ preventDefault() {} }));
    await settle();
    expect(props().nominated).not.toBeNull();
    expect(document.querySelector(".next-call")).not.toBeNull();
    // 名單重讀後 c1 已是舞台卡（隱藏區拉回 c2 會觸發名單重讀）
    backend.handlers.list_characters = () => [{ ...actor("c1", "狐狸"), stage: true }, actor("c2", "怪盜", true)];
    await act(async () => (rail.props!.onRestoreAutoHidden as (id: string) => void)("c2"));
    await settle();
    expect(props().castIds).not.toContain("c1");
    expect(document.querySelector(".next-call")).toBeNull();
    backend.calls = [];
    await run(() => props().onCallNominated());
    expect(gmCalls()).toHaveLength(0);
    expect(called("chat_with_character")).toHaveLength(0);
  });

  it("重讀失敗保留這桌上次的結果", async () => {
    await start();
    expect(props().relayable).toBe(true);
    mode = () => Promise.reject("disk");
    arrived = ["c2"];
    const before = called("gm_table_mode").length;
    await type("我推門");
    await act(async () => props().onSubmit({ preventDefault() {} }));
    await settle();
    expect(called("gm_table_mode").length).toBeGreaterThan(before);
    expect(props().relayable).toBe(true);
    expect(advanceShown()).toBe(true);
  });

  it("這桌從沒讀成：當一般桌、藏 B，GM 推進照常顯示", async () => {
    mode = () => Promise.reject("disk");
    await start();
    expect(props().relayable).toBe(false);
    expect(advanceShown()).toBe(true);
    await type("我推門");
    await act(async () => props().onSubmit({ preventDefault() {} }));
    await settle();
    // 一般送出照常（判定已就緒，只是不點名）
    expect(called("gm_narrate")).toHaveLength(1);
  });
});
