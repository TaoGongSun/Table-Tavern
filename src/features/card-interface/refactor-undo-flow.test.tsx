// @vitest-environment happy-dom
// 重構沒產殼的 interface 桌：沒有卡片介面鈕；「復原上次匯入」把殼與狀態樹退回後，
// 介面鈕與頂部狀態欄都跟著回來（refactor-noshell-panel）。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { WorldMeta } from "../../shared/contracts/backend-contracts";
import type { CardInterface } from "./interface-card";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));
const dialogs = vi.hoisted(() => ({ confirm: vi.fn(async () => true) }));

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
  message: vi.fn(async () => {}),
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import App from "../../App";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const world = (id: string, name: string, extra: Partial<WorldMeta> = {}): WorldMeta => ({
  id,
  name,
  read_only: false,
  needs_repair: false,
  ...extra,
});

interface Fixture {
  worlds: WorldMeta[];
  open: Record<string, unknown>;
  characters: Record<string, unknown>[];
}

function install(fixture: Fixture) {
  const h = backend.handlers;
  h.read_config = () => ({
    api_keys: { openrouter: "key" },
    tier_models: {},
    preferences: { language: "zh-TW", transport: "api" },
  });
  h.list_worlds = () => fixture.worlds;
  h.open_world = ({ worldId }) => fixture.open[worldId as string] ?? { status: "ready" };
  h.read_state = () => ({
    state: {},
    current_scene: 0,
    scene_titles: {},
    scene_labels: {},
    player_card_id: null,
  });
  h.read_transcript = () => [];
  // 回新陣列：同一個參照 React 會當沒變
  h.list_characters = () => fixture.characters.map((c) => ({ ...c }));
  h.scene_appearances = () => ({ character_ids: [], person_titles: [] });
  h.read_world_md = () => "";
  h.read_worldbook = () => [];
  h.list_import_receipts = () => [];
  h.world_has_state_bar = () => false;
  h.reclaim_world_if_empty = () => false;
  h.new_id = () => "new-card-id";
  h.smart_free_new_models = () => [];
  h.card_interfaces = () => [];
  h.keepalive_lanes = () => [];
  h.branch_bindings = () => [];
  h.mechanism_ledger = () => [];
}

const card: CardInterface = {
  character_id: "c1",
  character_name: "卡",
  scripts: [
    {
      name: "殼",
      find_regex: "/<UI>([\\s\\S]*?)<\\/UI>/s",
      replace_string: "```text\n<!DOCTYPE html><body>$1</body>\n```",
      trim_strings: [],
      min_depth: null,
      max_depth: null,
    },
  ],
  unsupported: null,
  opening: "<UI>開場白</UI>",
};

describe("refactor undo restores panel and state bar", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  beforeEach(() => {
    backend.handlers = {};
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
    act(() => {
      root?.unmount();
    });
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
    await act(async () => {
      (element as HTMLElement).click();
    });
    await settle();
  }

  const panelButton = () => document.querySelector(`[title="${t("cardInterfaceOpen")}"]`);

  it("brings back the card interface button and the restored tree after undo", async () => {
    install({ worlds: [world("w1", "酒館")], open: {}, characters: [] });
    const table = {
      shell: null as string | null,
      tree: { 驛站: { 糧草: "320" } } as Record<string, unknown>,
      receipts: [{ kind: "refactor", label: "重構", timestamp: "t", character_id: null }],
    };
    const h = backend.handlers;
    // 角色清單回同一個參照：不讓「角色清單換新」順帶觸發狀態樹重讀，驗的是復原本身有重讀
    const cast: unknown[] = [];
    h.list_characters = () => cast;
    h.card_interfaces = () => [card];
    h.refactor_table_mode = () => "interface";
    h.refactor_interface_shell = () => table.shell;
    h.read_state = () => ({
      state: { table: {}, tree: table.tree, jumps: {} },
      current_scene: 0,
      scene_titles: {},
      scene_labels: {},
      player_card_id: null,
    });
    h.list_import_receipts = () => table.receipts;
    h.undo_last_import = () => {
      table.shell = "<UI>{{世界.時間}}</UI>";
      table.tree = { 世界: { 時間: "清晨" } };
      table.receipts = [];
      return {
        removed_character: null,
        removed_characters: [],
        removed_entries: 0,
        kept_entries: 0,
        renamed_back: false,
        removed_opening: false,
      };
    };

    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => {
      root?.render(<App />);
    });
    await settle();
    await settle();
    await click(document.querySelector(`[aria-label="${t("lobbyEnterTable", { name: "酒館" })}"]`)!);
    await settle();

    // 沒產殼：空桌也不退回卡片開場白，沒有介面鈕
    expect(panelButton()).toBeNull();
    expect(document.body.textContent).toContain("320");

    await click(document.querySelector(`[aria-label="${t("castAdd")}"]`)!);
    const undo = [...document.querySelectorAll('[role="menuitem"]')].find((item) =>
      item.textContent?.includes(t("undoLastImport")),
    );
    await click(undo!);
    await settle();

    expect(panelButton()).not.toBeNull();
    expect(document.body.textContent).toContain("清晨");
    expect(document.body.textContent).not.toContain("320");
  });
});
