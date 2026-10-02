// @vitest-environment happy-dom
// App 層的大廳／牌桌導航：用假後端（invoke 依指令回資料）把整個 App 掛起來走真流程。
// 驗的是導航語意——誰能進、誰被擋、鎖有沒有放——不驗畫面細節。

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
  h.card_interfaces = () => [];
  h.keepalive_lanes = () => [];
  h.branch_bindings = () => [];
  h.mechanism_ledger = () => [];
}

function character(id: string, name: string) {
  return {
    id,
    name,
    color: "#e07a5f",
    avatar: "🎭",
    tier: "balanced",
    show_image: false,
    archived: false,
    auto_hidden: false,
  };
}

describe("lobby and table navigation", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  beforeEach(() => {
    backend.handlers = {};
    backend.calls = [];
    dialogs.confirm.mockReset();
    dialogs.confirm.mockResolvedValue(true);
    // Node 新版的全域 localStorage 在沒給 --localstorage-file 時是 undefined，蓋掉 happy-dom 的
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

  async function boot() {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => {
      root?.render(<App />);
    });
    await settle();
    await settle();
  }

  const byLabel = (label: string) =>
    document.querySelector<HTMLElement>(`[aria-label="${label}"]`);
  const cover = (name: string) => byLabel(t("lobbyEnterTable", { name }))! as HTMLButtonElement;
  const homeButton = () => byLabel(t("backToLobby")) as HTMLButtonElement | null;
  const inLobby = () => document.querySelector(".lobby") !== null;
  const calls = (command: string) => backend.calls.filter((call) => call.command === command);

  async function click(element: Element) {
    await act(async () => {
      (element as HTMLElement).click();
    });
    await settle();
  }

  async function typeInto(element: HTMLTextAreaElement | HTMLInputElement, value: string) {
    const proto = Object.getPrototypeOf(element);
    const setter = Object.getOwnPropertyDescriptor(proto, "value")!.set!;
    await act(async () => {
      setter.call(element, value);
      element.dispatchEvent(new Event("input", { bubbles: true }));
    });
  }

  it("boots into the lobby and does not enter the first table", async () => {
    install({ worlds: [world("w1", "Alpha")], open: {}, characters: [] });
    await boot();
    expect(inLobby()).toBe(true);
    expect(calls("open_world")).toHaveLength(0);
  });

  it("boots an empty install into the lobby with a freshly made sample table", async () => {
    const fixture: Fixture = { worlds: [], open: {}, characters: [] };
    install(fixture);
    backend.handlers.create_sample_world = () => {
      fixture.worlds = [world("sample", "Sample")];
      return "sample";
    };
    await boot();
    expect(calls("create_sample_world")).toHaveLength(1);
    expect(inLobby()).toBe(true);
    expect(calls("open_world")).toHaveLength(0);
    expect(cover("Sample")).not.toBeNull();
  });

  it("goes back to the lobby even when reclaiming the empty table fails", async () => {
    const auto = t("newTableName");
    install({ worlds: [world("w1", auto)], open: {}, characters: [] });
    backend.handlers.reclaim_world_if_empty = () => {
      throw new Error("reclaim failed");
    };
    await boot();
    await click(cover(auto));
    expect(inLobby()).toBe(false);
    await click(homeButton()!);
    expect(calls("reclaim_world_if_empty")).toHaveLength(1);
    expect(inLobby()).toBe(true);
    expect(document.querySelector(".lobby-notices")?.textContent).toContain("reclaim failed");
  });

  it("enters a table that needs repair from the lobby and shows the repair page", async () => {
    install({
      worlds: [world("w2", "Broken", { needs_repair: true })],
      open: { w2: { status: "needs_repair", reason: "io", error: "disk is sad", directory: "/tmp/w2" } },
      characters: [],
    });
    await boot();
    await click(cover("Broken"));
    expect(inLobby()).toBe(false);
    expect(document.body.textContent).toContain("disk is sad");
    // 修復桌不回收，回大廳照樣放行
    await click(homeButton()!);
    expect(calls("reclaim_world_if_empty")).toHaveLength(0);
    expect(inLobby()).toBe(true);
  });

  it("a second click while the first entry is still running does not enter twice", async () => {
    install({ worlds: [world("w1", "Alpha")], open: {}, characters: [] });
    let release: (value: unknown) => void = () => {};
    backend.handlers.open_world = () => new Promise((resolve) => (release = resolve));
    await boot();
    await act(async () => {
      cover("Alpha").click();
      cover("Alpha").click();
    });
    expect(calls("open_world")).toHaveLength(1);
    expect(cover("Alpha").disabled).toBe(true);
    await act(async () => {
      release({ status: "ready" });
    });
    await settle();
    expect(inLobby()).toBe(false);
    expect(calls("open_world")).toHaveLength(1);
  });

  it("releases the lock after the leave guard is cancelled", async () => {
    install({ worlds: [world("w1", "Alpha")], open: {}, characters: [] });
    await boot();
    await click(cover("Alpha"));
    // 開世界設定、改一個字：未儲存守門會跳確認框
    await click(byLabel(t("worldSummary"))!);
    // 世界設定的文字框改由可見 label（worldSummary）命名
    const box = [...document.querySelectorAll("label")]
      .find((label) => label.textContent?.startsWith(t("worldSummary")))!
      .querySelector("textarea")!;
    await typeInto(box, "edited");
    dialogs.confirm.mockResolvedValueOnce(false);
    await click(homeButton()!);
    expect(dialogs.confirm).toHaveBeenCalled();
    expect(inLobby()).toBe(false);
    expect(homeButton()!.disabled).toBe(false);
    // 鎖已放開：再按一次、這次放行，就真的回大廳
    dialogs.confirm.mockResolvedValueOnce(true);
    await click(homeButton()!);
    expect(inLobby()).toBe(true);
  });

  it("disables going back to the lobby while a current-table import is still running", async () => {
    install({ worlds: [world("w1", "Alpha")], open: {}, characters: [] });
    let release: (value: unknown) => void = () => {};
    backend.handlers.probe_import = () => {
      throw new Error("probe unavailable");
    };
    backend.handlers.import_character = () => new Promise((resolve) => (release = resolve));
    await boot();
    await click(cover("Alpha"));
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;
    const file = new File([new Uint8Array([1, 2, 3])], "card.json");
    Object.defineProperty(input, "files", { value: [file], configurable: true });
    await act(async () => {
      input.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await settle();
    expect(calls("import_character")).toHaveLength(1);
    expect(homeButton()!.disabled).toBe(true);
    await click(homeButton()!);
    expect(inLobby()).toBe(false);
    await act(async () => {
      release({ meta: character("c1", "Imported"), book: { imported: 0, skipped: 0 } });
    });
    await settle();
    expect(homeButton()!.disabled).toBe(false);
  });

  it("uses the narrow rail for 0–1 character cards and the wide rail from 2", async () => {
    const one = { worlds: [world("w1", "Solo")], open: {}, characters: [character("c1", "Ann")] };
    install(one);
    await boot();
    await click(cover("Solo"));
    expect(document.querySelector(".sidebar-narrow")).not.toBeNull();
    expect(document.querySelector(".sidebar-resizer")).toBeNull();
    await click(homeButton()!);

    one.characters = [character("c1", "Ann"), character("c2", "Bob")];
    await click(cover("Solo"));
    expect(document.querySelector(".sidebar-narrow")).toBeNull();
    expect(document.querySelector(".sidebar-resizer")).not.toBeNull();
  });

  describe("import answers stay bound to the table they were asked in", () => {
    const parsedProbe = {
      parsed: true,
      name: "Card",
      lorebook_heavy: false,
      book_entries: 0,
      book_shaped: false,
      alternate_greetings: 0,
    };

    async function pickFile() {
      const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;
      const file = new File([new Uint8Array([1, 2, 3])], "card.json");
      Object.defineProperty(input, "files", { value: [file], configurable: true });
      await act(async () => {
        input.dispatchEvent(new Event("change", { bubbles: true }));
      });
      await settle();
    }
    const dialogOpen = () => document.querySelector("dialog") !== null;

    it("drops a probe that finishes after leaving for another table", async () => {
      install({
        worlds: [world("w1", "Alpha"), world("w2", "Beta")],
        open: {},
        characters: [],
      });
      let release: (value: unknown) => void = () => {};
      backend.handlers.probe_import = () => new Promise((resolve) => (release = resolve));
      await boot();
      await click(cover("Alpha"));
      await pickFile();
      await click(homeButton()!);
      await click(cover("Beta"));
      await act(async () => {
        release(parsedProbe);
      });
      await settle();
      expect(dialogOpen()).toBe(false);
      expect(calls("import_character")).toHaveLength(0);
    });

    it("clears a pending identity question when leaving the table", async () => {
      install({ worlds: [world("w1", "Alpha")], open: {}, characters: [] });
      backend.handlers.probe_import = () => parsedProbe;
      await boot();
      await click(cover("Alpha"));
      await pickFile();
      expect(dialogOpen()).toBe(true);
      await click(homeButton()!);
      await click(cover("Alpha"));
      expect(dialogOpen()).toBe(false);
      expect(calls("import_character")).toHaveLength(0);
    });
  });

  it("drops the opening-line panel of the old table when the player changes table", async () => {
    install({ worlds: [world("w1", "Alpha")], open: {}, characters: [] });
    backend.handlers.probe_import = () => {
      throw new Error("probe unavailable");
    };
    backend.handlers.import_character = () => ({
      meta: character("c1", "Imported"),
      book: { imported: 0, skipped: 0 },
    });
    backend.handlers.card_openings = () => ["Once upon a time"];
    backend.handlers.translate_tier_models = () => [];
    await boot();
    await click(cover("Alpha"));
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;
    Object.defineProperty(input, "files", {
      value: [new File([new Uint8Array([1])], "c.json")],
      configurable: true,
    });
    await act(async () => {
      input.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await settle();
    expect(document.body.textContent).toContain("Once upon a time");
    await click(homeButton()!);
    await click(cover("Alpha"));
    expect(document.body.textContent).not.toContain("Once upon a time");
  });

  it("a translation that returns late for table A never reaches table B's panel", async () => {
    install({
      worlds: [world("w1", "Alpha"), world("w2", "Beta")],
      open: {},
      characters: [],
    });
    backend.handlers.probe_import = () => {
      throw new Error("probe unavailable");
    };
    backend.handlers.import_character = () => ({
      meta: character("c1", "Imported"),
      book: { imported: 0, skipped: 0 },
    });
    let openingOf = "A opening";
    backend.handlers.card_openings = () => [openingOf];
    backend.handlers.translate_tier_models = () => [];
    let release: (value: unknown) => void = () => {};
    backend.handlers.translate_opening = () => new Promise((resolve) => (release = resolve));
    await boot();

    async function importCard() {
      const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;
      Object.defineProperty(input, "files", {
        value: [new File([new Uint8Array([1])], "c.json")],
        configurable: true,
      });
      await act(async () => {
        input.dispatchEvent(new Event("change", { bubbles: true }));
      });
      await settle();
    }

    await click(cover("Alpha"));
    await importCard();
    await click(document.querySelector(".opening-translate-all")!);
    await click(document.querySelector(".dialog-close")!);
    await click(homeButton()!);
    await click(cover("Beta"));
    openingOf = "B opening";
    await importCard();
    expect(document.body.textContent).toContain("B opening");
    await act(async () => {
      release("A-TRANSLATION");
    });
    await settle();
    expect(document.body.textContent).not.toContain("A-TRANSLATION");
    expect(document.body.textContent).toContain("B opening");
    // 也不能因為 A 的迴圈收尾而讓 B 的「全部翻譯」鈕卡在忙碌
    expect(
      document.querySelector<HTMLButtonElement>(".opening-translate-all")!.disabled,
    ).toBe(false);
  });

  it("disables the composer actions while a table-level operation holds the lock", async () => {
    install({ worlds: [world("w1", "Alpha")], open: {}, characters: [character("c1", "Ann")] });
    backend.handlers.probe_import = () => {
      throw new Error("probe unavailable");
    };
    backend.handlers.import_character = () => new Promise(() => {});
    await boot();
    await click(cover("Alpha"));
    const writebox = () => document.querySelector<HTMLInputElement>(".writebox")!;
    expect(writebox().disabled).toBe(false);
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;
    Object.defineProperty(input, "files", {
      value: [new File([new Uint8Array([1])], "c.json")],
      configurable: true,
    });
    await act(async () => {
      input.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await settle();
    expect(writebox().disabled).toBe(true);
    expect(document.querySelector<HTMLButtonElement>(".composer-primary")!.disabled).toBe(true);
    for (const button of document.querySelectorAll<HTMLButtonElement>(".btn-seg button")) {
      expect(button.disabled).toBe(true);
    }
  });

  describe("toolbar rename settles once", () => {
    async function startRename() {
      install({ worlds: [world("w1", "Alpha")], open: {}, characters: [] });
      await boot();
      await click(cover("Alpha"));
      await click(document.querySelector(".table-title")!);
      const field = document.querySelector<HTMLInputElement>(".table-title-input")!;
      await typeInto(field, "Beta");
      return field;
    }

    it("Enter submits once even though the input then blurs", async () => {
      const field = await startRename();
      await act(async () => {
        field.form!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
        field.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
      });
      await settle();
      expect(calls("rename_world")).toHaveLength(1);
    });

    it("Escape cancels and the following blur does not save", async () => {
      const field = await startRename();
      await act(async () => {
        field.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
        field.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
      });
      await settle();
      expect(calls("rename_world")).toHaveLength(0);
    });
  });

  it("converting the current speaker into a worldbook entry clears it everywhere", async () => {
    const fixture: Fixture = {
      worlds: [world("w1", "Alpha")],
      open: {},
      characters: [character("c1", "Alice"), character("c2", "Bob"), character("c3", "Cora")],
    };
    install(fixture);
    backend.handlers.read_character = ({ characterId }) => ({
      ...fixture.characters.find((c) => c.id === characterId)!,
      public_md: "",
      private_md: "",
      gen_prompt: "",
    });
    backend.handlers.character_to_worldbook_entry = ({ characterId }) => {
      fixture.characters = fixture.characters.filter((c) => c.id !== characterId);
    };
    await boot();
    await click(cover("Alpha"));
    const rail = () => document.querySelector<HTMLElement>(".sidebar")!;
    const cardMain = (name: string) =>
      [...rail().querySelectorAll<HTMLButtonElement>(".tcard-main")].find((button) =>
        button.textContent?.includes(name),
      );
    if (cardMain("Alice")!.getAttribute("aria-pressed") !== "true") await click(cardMain("Alice")!);
    expect(cardMain("Alice")!.getAttribute("aria-pressed")).toBe("true");

    await click(byLabel(t("editCardSummary", { name: "Alice" }))!);
    expect(document.querySelector(".edit-page")).not.toBeNull();
    await click(document.querySelector(`.edit-page [aria-label="${t("moreActions")}"]`)!);
    const convert = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      (item) => item.textContent === t("convertCardToEntry"),
    )!;
    await click(convert);

    expect(calls("character_to_worldbook_entry")).toHaveLength(1);
    expect(rail().textContent).not.toContain("Alice");
    expect(document.querySelector("[data-archive-row]")).toBeNull();
    expect(cardMain("Bob")!.getAttribute("aria-pressed")).toBe("true");
    expect(document.querySelector(".edit-page")).toBeNull();
  });

  describe("focus after an archived row disappears", () => {
    const archivedCard = (id: string, name: string) => ({ ...character(id, name), archived: true });
    const rowOf = (name: string) =>
      [...document.querySelectorAll<HTMLElement>("[data-archive-row]")].find((row) =>
        row.textContent?.includes(name),
      )!;
    const restoreButton = (name: string) =>
      [...rowOf(name).querySelectorAll<HTMLButtonElement>("button")].find(
        (button) => button.textContent === t("restoreCharacter"),
      )!;

    function setup(active: number) {
      const fixture: Fixture = {
        worlds: [world("w1", "Alpha")],
        open: {},
        characters: [
          ...Array.from({ length: active }, (_, i) => character(`a${i}`, `Active${i}`)),
          archivedCard("x1", "Xena"),
          archivedCard("x2", "Yuri"),
        ],
      };
      install(fixture);
      backend.handlers.set_character_archived = ({ characterId }) => {
        const found = fixture.characters.find((c) => c.id === characterId)!;
        found.archived = false;
      };
      return fixture;
    }

    async function restore(name: string) {
      await act(async () => {
        restoreButton(name).focus();
      });
      await click(restoreButton(name));
    }

    it("wide rail: moves to the next row, then to the add button once none are left", async () => {
      setup(2);
      await boot();
      await click(cover("Alpha"));
      document.querySelector<HTMLDetailsElement>("details.archive-section")!.open = true;
      await restore("Xena");
      expect(rowOf("Yuri").contains(document.activeElement)).toBe(true);
      await restore("Yuri");
      expect(document.activeElement).toBe(document.querySelector(".rail-add"));
    });

    it("narrow rail panel: next row first, then the add button when the panel closes", async () => {
      setup(0);
      await boot();
      await click(cover("Alpha"));
      await click(document.querySelector(".rail-archive-btn")!);
      await restore("Xena");
      expect(rowOf("Yuri").contains(document.activeElement)).toBe(true);
      await restore("Yuri");
      expect(document.querySelector(".archive-popover")).toBeNull();
      expect(document.activeElement).toBe(document.querySelector(".rail-add"));
    });

    const panel = () => document.querySelector(".archive-popover");
    const trigger = () => document.querySelector<HTMLElement>(".rail-archive-btn")!;
    const flush = () => act(async () => void (await new Promise((r) => setTimeout(r, 5))));
    const pointerDownOutside = () =>
      act(async () => {
        document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      });

    it("outside click closes the panel and returns focus only if it was inside", async () => {
      setup(0);
      await boot();
      await click(cover("Alpha"));
      await click(trigger());
      await act(async () => restoreButton("Xena").focus());
      await pointerDownOutside();
      await flush();
      expect(panel()).toBeNull();
      expect(document.activeElement).toBe(trigger());

      // 焦點已在別的控制項上：不搶
      await click(trigger());
      const add = document.querySelector<HTMLElement>(".rail-add")!;
      await act(async () => add.focus());
      await pointerDownOutside();
      await flush();
      expect(panel()).toBeNull();
      expect(document.activeElement).toBe(add);
    });

    it("clicking the trigger while focus is inside the panel closes it and does not reopen", async () => {
      setup(0);
      await boot();
      await click(cover("Alpha"));
      await click(trigger());
      const inside = restoreButton("Xena");
      await act(async () => inside.focus());
      await act(async () => {
        inside.dispatchEvent(
          new FocusEvent("focusout", { bubbles: true, relatedTarget: trigger() }),
        );
        trigger().click();
      });
      expect(panel()).toBeNull();
    });

    it("tabbing out of the panel closes it", async () => {
      setup(0);
      await boot();
      await click(cover("Alpha"));
      await click(trigger());
      const inside = restoreButton("Xena");
      await act(async () => inside.focus());
      await act(async () => {
        inside.dispatchEvent(
          new FocusEvent("focusout", { bubbles: true, relatedTarget: document.body }),
        );
      });
      // relatedTarget 為 body 代表走去面板外的別處（沒有目標才是整個視窗失焦，不關）
      expect(panel()).toBeNull();
    });
  });

  describe("picking a character card", () => {
    const cast = [character("c1", "Ann"), character("c2", "Bob")];
    // 寬欄的選取鈕名字來自內容（名字 wedge），窄欄才有 aria-label
    const cardButton = (name: string) =>
      [...document.querySelectorAll<HTMLElement>("button[aria-pressed]")].find(
        (button) => button.textContent?.includes(name),
      )!;
    const pressed = (name: string) => cardButton(name)?.getAttribute("aria-pressed");
    const nameField = () =>
      document.querySelector<HTMLInputElement>(`input[placeholder="${t("newCharacterPlaceholder")}"]`);

    async function openDirtyNewCard() {
      await click(cover("Alpha"));
      expect(document.querySelector(".sidebar-narrow")).toBeNull();
      await click(document.querySelector(".rail-add")!);
      await click(document.querySelector<HTMLElement>('[role="menuitem"]')!);
      const field = nameField();
      expect(field).not.toBeNull();
      await typeInto(field!, "Draft");
    }

    it("asks the leave guard first, then closes the editor and selects the speaker", async () => {
      install({ worlds: [world("w1", "Alpha")], open: {}, characters: cast });
      await boot();
      await openDirtyNewCard();
      dialogs.confirm.mockResolvedValueOnce(false);
      await click(cardButton("Bob"));
      expect(dialogs.confirm).toHaveBeenCalledTimes(1);
      // 守門取消：編輯頁還在，對象沒變
      expect(nameField()).not.toBeNull();
      expect(pressed("Bob")).not.toBe("true");

      dialogs.confirm.mockResolvedValueOnce(true);
      await click(cardButton("Bob"));
      expect(nameField()).toBeNull();
      expect(pressed("Bob")).toBe("true");
      // 不是取消：再點才是取消
      await click(cardButton("Bob"));
      expect(pressed("Bob")).toBe("false");
    });
  });
});
