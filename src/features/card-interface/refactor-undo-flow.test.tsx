// @vitest-environment happy-dom
// 重構沒產殼的 interface 桌照原卡畫面（空桌是開場白那一樓）；「復原上次匯入」把殼與狀態樹退回後，
// 介面鈕還在、狀態樹重讀；退回的是有骨架的接管桌，頂部狀態欄不顯示（refactor-noshell-panel、
// card-chat-messages-shim、refactor-statusbar-skeleton）。

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

  it("keeps the card interface button and restores the tree after undo", async () => {
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

    // 沒產殼：照原卡畫面，空桌退回卡片開場白
    expect(panelButton()).not.toBeNull();
    expect(document.body.textContent).toContain("320");

    await click(document.querySelector(`[aria-label="${t("castAdd")}"]`)!);
    const undo = [...document.querySelectorAll('[role="menuitem"]')].find((item) =>
      item.textContent?.includes(t("undoLastImport")),
    );
    const readsBefore = backend.calls.filter((call) => call.command === "read_state").length;
    await click(undo!);
    await settle();

    expect(panelButton()).not.toBeNull();
    expect(backend.calls.filter((call) => call.command === "read_state").length).toBeGreaterThan(readsBefore);
    // 退回有骨架的桌＝介面由 App 接管：狀態在卡片畫面裡，頂部狀態欄不顯示
    expect(document.body.textContent).not.toContain("320");
    expect(document.body.textContent).not.toContain("清晨");
  });

  // 撤銷匯入與套用重構完成後走實際的 App 刷新鏈；在 characters.refresh／refreshInterfaces／refreshShell
  // 任一處懸著時換到另一桌，舊桌的陣容、發言對象、介面與骨架都不能寫進新桌
  describe.each([
    ["撤銷匯入", "undo"],
    ["套用重構", "apply"],
  ] as const)("%s後刷新到一半換桌", (_label, flow) => {
    it.each([
      ["characters.refresh", "list_characters"],
      ["refreshInterfaces", "card_interfaces"],
      ["refreshShell", "refactor_interface_shell"],
    ])("懸在 %s：新桌的發言對象、介面、骨架不被舊桌覆寫", async (_stage, command) => {
      const actor = (id: string, name: string) => ({
        id,
        name,
        color: "#888",
        archived: false,
        auto_hidden: false,
      });
      install({ worlds: [world("w1", "酒館"), world("w2", "驛站")], open: {}, characters: [] });
      const h = backend.handlers;
      let acted = false;
      let seen = 0;
      // 套用後 WorldEditor 自己先重讀一次陣容（refreshCast），App 的 characters.refresh 是第二次
      const skip = flow === "apply" && command === "list_characters" ? 1 : 0;
      let release!: () => void;
      const held = new Promise<void>((done) => (release = done));
      const w1Data: Record<string, () => unknown> = {
        list_characters: () => (acted ? [] : [actor("c1", "甲角")]),
        card_interfaces: () => [card],
        refactor_interface_shell: () => "<UI>{{世界.時間}}</UI>",
        refactor_table_mode: () => "interface",
      };
      const w2Data: Record<string, () => unknown> = {
        list_characters: () => [actor("c2", "乙角")],
        card_interfaces: () => [],
        refactor_interface_shell: () => null,
        refactor_table_mode: () => null,
      };
      for (const name of Object.keys(w1Data)) {
        h[name] = ({ worldId }) => {
          if (worldId !== "w1") return w2Data[name]!();
          if (acted && name === command && seen++ === skip) return held.then(w1Data[name]!);
          return w1Data[name]!();
        };
      }
      // 新桌有頂部狀態欄：舊桌的骨架一旦寫進來就成了接管桌，狀態欄會被收掉
      h.world_has_state_bar = ({ worldId }) => worldId === "w2";
      h.read_state = ({ worldId }) => ({
        state: { table: {}, tree: worldId === "w2" ? { 驛站: { 糧草: "999石" } } : {}, jumps: {} },
        current_scene: 0,
        scene_titles: {},
        scene_labels: {},
        player_card_id: null,
      });
      h.list_import_receipts = ({ worldId }) =>
        worldId === "w1" && !acted
          ? [{ kind: "character", label: "甲角", timestamp: "t", character_id: "c1" }]
          : [];
      h.undo_last_import = () => {
        acted = true;
        return {
          removed_character: "c1",
          removed_characters: ["甲角"],
          removed_entries: 0,
          kept_entries: 0,
          renamed_back: false,
          removed_opening: false,
        };
      };
      h.refactor_apply = () => {
        acted = true;
        return {
          new_characters: 1,
          player_assigned: false,
          new_entries: 0,
          deleted_entries: 0,
          interface_applied: true,
          mechanisms_applied: 0,
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

      if (flow === "undo") {
        await click(document.querySelector(`[aria-label="${t("castAdd")}"]`)!);
        const undo = [...document.querySelectorAll('[role="menuitem"]')].find((item) =>
          item.textContent?.includes(t("undoLastImport")),
        );
        await click(undo!);
      } else {
        await click(document.querySelector(`[aria-label="${t("worldSummary")}"]`)!);
        const input = document.querySelector('input[type="file"][accept^=".json"]') as HTMLInputElement;
        const outcome = JSON.stringify({ characters: [{ name: "阿福", source_uids: ["1"] }] });
        Object.defineProperty(input, "files", {
          configurable: true,
          value: [new File([outcome], "card.json")],
        });
        await act(async () => {
          input.dispatchEvent(new Event("change", { bubbles: true }));
        });
        await settle();
        const applyAll = [...document.querySelectorAll("button")].find(
          (button) => button.textContent?.includes(t("refactorApplyAll")),
        );
        await click(applyAll!);
      }
      await settle();
      await vi.waitFor(() => expect(seen).toBeGreaterThan(skip));

      await click(document.querySelector(`[aria-label="${t("backToLobby")}"]`)!);
      await settle();
      await click(document.querySelector(`[aria-label="${t("lobbyEnterTable", { name: "驛站" })}"]`)!);
      await settle();
      const pressed = () =>
        [...document.querySelectorAll('button[aria-pressed="true"]')].map((b) => b.textContent ?? "");
      expect(pressed().some((text) => text.includes("乙角"))).toBe(true);
      expect(panelButton()).toBeNull();
      expect(document.body.textContent).toContain("999石");

      const mark = backend.calls.length;
      await act(async () => {
        release();
      });
      await settle();
      await settle();

      expect(pressed().some((text) => text.includes("乙角"))).toBe(true);
      expect(document.body.textContent).toContain("乙角");
      expect(panelButton()).toBeNull();
      expect(document.body.textContent).toContain("999石");
      const touchedOld = backend.calls
        .slice(mark)
        .filter((call) => call.args.worldId === "w1")
        .map((call) => call.command);
      expect(touchedOld).toEqual([]);
    });
  });
});
