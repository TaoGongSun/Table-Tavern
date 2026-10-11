// @vitest-environment happy-dom
// 陣容欄撤銷上次匯入時主欄的編輯畫面（undo-import-editor-guard）：世界設定不重新掛載、未存的字留著；
// 被撤銷刪掉的卡正在編輯就關、不另問；別張卡不關；撤銷退回玩家卡指定時側欄跟著重讀。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));
const dialogs = vi.hoisted(() => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    backend.calls.push({ command, args });
    const handler = backend.handlers[command];
    return handler ? handler(args) : null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "0.2.0") }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: dialogs.confirm,
  message: dialogs.message,
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import App from "../../App";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const actor = (id: string, name: string) => ({
  id,
  name,
  color: "#888",
  avatar: "🎭",
  tier: "balanced",
  show_image: true,
  archived: false,
  auto_hidden: false,
});

describe("undo last import with an editor open", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let undone = false;
  let playerCardId: string | null = null;

  beforeEach(() => {
    undone = false;
    playerCardId = null;
    const h: Record<string, (args: Record<string, unknown>) => unknown> = {};
    h.read_config = () => ({
      api_keys: { openrouter: "key" },
      tier_models: {},
      preferences: { language: "zh-TW", transport: "api" },
    });
    h.list_worlds = () => [{ id: "w1", name: "酒館", read_only: false, needs_repair: false }];
    h.open_world = () => ({ status: "ready" });
    h.read_state = () => ({
      state: {},
      current_scene: 0,
      scene_titles: {},
      scene_labels: {},
      player_card_id: playerCardId,
    });
    h.read_transcript = () => [];
    h.list_characters = () =>
      undone ? [actor("c2", "乙角")] : [actor("c1", "甲角"), actor("c2", "乙角")];
    h.read_character = ({ characterId }) => ({
      ...actor(characterId as string, characterId === "c1" ? "甲角" : characterId === "p1" ? "我" : "乙角"),
      public_md: "",
      private_md: "",
      gen_prompt: "",
    });
    h.scene_appearances = () => ({ character_ids: [], person_titles: [] });
    h.read_world_md = () => "世界原文";
    h.read_worldbook = () => [];
    h.list_import_receipts = () =>
      undone ? [] : [{ kind: "character", label: "甲角", timestamp: "t", character_id: "c1" }];
    h.world_has_state_bar = () => false;
    h.reclaim_world_if_empty = () => false;
    h.new_id = () => "new-card-id";
    h.smart_free_new_models = () => [];
    h.card_interfaces = () => [];
    h.branch_bindings = () => [];
    h.mechanism_ledger = () => ({ entries: [], rejected: 0, clamped: 0, errors: 0, jumps: 0 });
    h.undo_last_import = () => {
      undone = true;
      playerCardId = null;
      return {
        removed_character: "甲角",
        removed_characters: [],
        removed_entries: 0,
        kept_entries: 0,
        renamed_back: false,
        removed_opening: false,
        removed_character_ids: ["c1"],
        entry_changes: { changed: [], deleted: [] },
      };
    };
    backend.handlers = h;
    backend.calls = [];
    dialogs.confirm.mockReset();
    dialogs.confirm.mockResolvedValue(true);
    dialogs.message.mockClear();
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
    document.body.innerHTML = "";
    vi.unstubAllGlobals();
  });

  async function settle() {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }

  async function click(element: Element) {
    await act(async () => (element as HTMLElement).click());
    await settle();
  }

  async function typeInto(element: HTMLTextAreaElement | HTMLInputElement, value: string) {
    const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), "value")!.set!;
    await act(async () => {
      setter.call(element, value);
      element.dispatchEvent(new Event("input", { bubbles: true }));
    });
  }

  async function start() {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root!.render(<App />));
    await settle();
    await settle();
    await click(document.querySelector(`[aria-label="${t("lobbyEnterTable", { name: "酒館" })}"]`)!);
    await settle();
  }

  async function undo() {
    await click(document.querySelector(".rail-add")!);
    const item = [...document.querySelectorAll('[role="menuitem"]')].find((candidate) =>
      candidate.textContent?.includes(t("undoLastImport")),
    )!;
    await click(item);
    await settle();
  }

  const labelled = (label: string) =>
    [...document.querySelectorAll("[aria-label]")].find(
      (element) => element.getAttribute("aria-label") === label,
    )!;

  const nameInput = (value: string) =>
    [...document.querySelectorAll<HTMLInputElement>("input")].find((input) => input.value === value);

  it("keeps the world editor mounted with unsaved world.md", async () => {
    await start();
    await click(labelled(t("worldSummary")));
    const text = document.querySelector<HTMLTextAreaElement>(".settings-form textarea")!;
    await typeInto(text, "沒存的世界");
    const reads = backend.calls.filter((call) => call.command === "read_world_md").length;
    await undo();
    expect(document.querySelector<HTMLTextAreaElement>(".settings-form textarea")!.value).toBe("沒存的世界");
    expect(backend.calls.filter((call) => call.command === "read_world_md")).toHaveLength(reads);
    // 只有撤銷確認那一次
    expect(dialogs.confirm).toHaveBeenCalledTimes(1);
  });

  it("closes the undone card's editor without asking, and keeps another card's", async () => {
    await start();
    await click(labelled(t("editCardSummary", { name: "甲角" })));
    await typeInto(nameInput("甲角")!, "改名沒存");
    await undo();
    expect(nameInput("改名沒存")).toBeUndefined();
    expect(dialogs.confirm).toHaveBeenCalledTimes(1);

    undone = false;
    await act(async () => {
      root!.unmount();
    });
    document.body.innerHTML = "";
    dialogs.confirm.mockClear();
    await start();
    await click(labelled(t("editCardSummary", { name: "乙角" })));
    await typeInto(nameInput("乙角")!, "乙改名沒存");
    await undo();
    expect(nameInput("乙改名沒存")).toBeDefined();
    expect(dialogs.confirm).toHaveBeenCalledTimes(1);
  });

  it("rereads the player card after the undo, and a failed reread does not block the done message", async () => {
    playerCardId = "p1";
    await start();
    const before = backend.calls.length;
    await undo();
    const after = backend.calls.slice(before).map((call) => call.command);
    expect(after.lastIndexOf("read_state")).toBeGreaterThan(after.indexOf("undo_last_import"));
    expect(dialogs.message).toHaveBeenCalled();

    undone = false;
    await act(async () => {
      root!.unmount();
    });
    document.body.innerHTML = "";
    dialogs.message.mockClear();
    await start();
    // 只讓收據重讀之後那次（玩家卡重讀）失敗
    const okState = backend.handlers.read_state;
    const okReceipts = backend.handlers.list_import_receipts;
    let receiptsReread = false;
    backend.handlers.list_import_receipts = (args) => {
      if (undone) receiptsReread = true;
      return okReceipts(args);
    };
    backend.handlers.read_state = (args) => {
      if (receiptsReread) throw "讀不到狀態";
      return okState(args);
    };
    await undo();
    expect(dialogs.message).toHaveBeenCalled();
    expect(document.body.textContent).toContain("讀不到狀態");
  });
});
