// @vitest-environment happy-dom
// 舞台卡（stage-card-in-speaker-list 包 3）：重構把原卡標成舞台卡並套用後，發言對象撥回 GM
// （App 的 refreshAfterRefactorApplied 既有規則），不會停在「選中的卡變成舞台」的沒有對象狀態。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { WorldMeta } from "../../shared/contracts/backend-contracts";

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
  h.gm_table_mode = () => ({ gm_only: false, nominates: true });
  h.card_interfaces = () => [];
  h.branch_bindings = () => [];
  h.mechanism_ledger = () => [];
}

describe("refactor apply that stages the original card", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  beforeEach(() => {
    backend.handlers = {
      refactor_card_open: (bytes) => ({
        card: JSON.parse(new TextDecoder().decode(bytes as unknown as Uint8Array)),
        assets: [],
        token: null,
      }),
    };
    backend.calls = [];
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

  it("points the speaker back at GM", async () => {
    const actor = (id: string, name: string, stage = false) => ({
      id,
      name,
      color: "#888",
      archived: false,
      auto_hidden: false,
      stage,
    });
    let applied = false;
    install({ worlds: [world("w1", "酒館")], open: {}, characters: [] });
    const h = backend.handlers;
    h.list_characters = () =>
      applied ? [actor("c1", "灰岩要塞", true), actor("c2", "托伦")] : [actor("c1", "灰岩要塞")];
    // 這一桌有可發言的同名卡：開卡時的舞台名單過濾照收
    h.refactor_stage_cards = ({ names }) => names;
    h.refactor_apply = ({ selection }) => {
      expect((selection as { stage_indices: number[] }).stage_indices).toEqual([0]);
      applied = true;
      return {
        new_characters: 1,
        player_assigned: false,
        stage_cards: 1,
        new_entries: 0,
        deleted_entries: 0,
        interface_applied: false,
        mechanisms_applied: 0,
        entry_changes: { changed: [], deleted: [] },
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
    const pressed = () =>
      [...document.querySelectorAll('button[aria-pressed="true"]')].map((b) => b.textContent ?? "");
    // 套用前預設發言對象是原卡
    expect(pressed().some((text) => text.includes("灰岩要塞"))).toBe(true);

    await click(document.querySelector(`[aria-label="${t("worldSummary")}"]`)!);
    const input = document.querySelector('input[type="file"][accept^=".json"]') as HTMLInputElement;
    const outcome = JSON.stringify({
      mode: "characters",
      characters: [{ name: "托伦", source_uids: ["1"] }],
      stage_cards: ["灰岩要塞"],
    });
    Object.defineProperty(input, "files", {
      configurable: true,
      value: [new File([outcome], "card.json")],
    });
    await act(async () => {
      input.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await settle();
    const applyAll = [...document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes(t("refactorApplyAll")),
    );
    await click(applyAll!);
    await settle();
    await settle();

    expect(applied).toBe(true);
    const gm = document.querySelector(".tcard-gm .tcard-main");
    expect(gm?.getAttribute("aria-pressed")).toBe("true");
    expect(pressed().some((text) => text.includes("灰岩要塞") || text.includes("托伦"))).toBe(false);
  });
});
