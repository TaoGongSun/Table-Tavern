// @vitest-environment happy-dom
// 撤銷上次匯入與匯入共用換桌鎖（undo-import-table-lock）：一方在途（含排在回合後面）時另一方在函式層被擋、
// 選單項停用；做完或失敗都放鎖。後端收據已換過時回 import_receipt_changed，前端報錯並重讀收據。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { backendText } from "../../shared/ui/backend-text";

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
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "0.2.0") }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: dialogs.confirm,
  message: dialogs.message,
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));
// 保留真的陣容欄，只記下最近一次 render 收到的 props：拿 onUndoImport 繞過停用的選單項，直接驗 App 那層的鎖
const rail = vi.hoisted(() => ({ props: null as Record<string, unknown> | null }));
vi.mock("./CastRail", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./CastRail")>();
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((ok, fail) => {
    resolve = ok;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const receipt = (label: string, id: string) => ({
  kind: "character",
  label,
  timestamp: "2026-10-11 10:00",
  character_id: id,
});

describe("undo last import shares the table lock with imports", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let receipts: ReturnType<typeof receipt>[] = [];

  beforeEach(() => {
    receipts = [receipt("甲角", "c1")];
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
      player_card_id: null,
    });
    h.read_transcript = () => [
      { ts: "t", speaker_id: "", speaker_name: "GM", kind: "narration", text: "開場" },
    ];
    h.list_characters = () => [actor("c1", "甲角")];
    h.read_character = ({ characterId }) => ({
      ...actor(characterId as string, "甲角"),
      public_md: "",
      private_md: "",
      gen_prompt: "",
    });
    h.scene_appearances = () => ({ character_ids: [], person_titles: [] });
    h.read_world_md = () => "";
    h.read_worldbook = () => [];
    h.list_import_receipts = () => receipts;
    h.world_has_state_bar = () => false;
    h.reclaim_world_if_empty = () => false;
    h.smart_free_new_models = () => [];
    h.card_interfaces = () => [];
    h.branch_bindings = () => [];
    h.mechanism_ledger = () => ({ entries: [], rejected: 0, clamped: 0, errors: 0, jumps: 0 });
    h.probe_import = () => {
      throw new Error("probe unavailable");
    };
    h.import_character = () => {
      receipts = [...receipts, receipt("丙角", "c3")];
      return importResult();
    };
    h.undo_last_import = () => {
      receipts = receipts.slice(0, -1);
      return undoResult();
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

  const importResult = () => ({
    meta: actor("c3", "丙角"),
    book: { imported: 0, skipped: 0 },
    source: null,
    image_dropped: false,
    book_failed: false,
    widened_entries: [],
  });
  const undoResult = () => ({
    removed_character: null,
    removed_characters: [],
    removed_entries: 0,
    kept_entries: 0,
    renamed_back: false,
    removed_opening: false,
    removed_character_ids: [],
    entry_changes: { changed: [], deleted: [] },
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
  }

  const called = (command: string) => backend.calls.filter((call) => call.command === command);

  /** 直接派送檔案選擇（不經選單），驗到函式層的鎖 */
  async function pickFile() {
    const input = document.querySelector<HTMLInputElement>('input[type="file"][accept^=".png"]')!;
    Object.defineProperty(input, "files", {
      value: [new File([new Uint8Array([1, 2, 3])], "card.json")],
      configurable: true,
    });
    await act(async () => {
      input.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await settle();
  }

  /** 呼叫陣容欄最近一次 render 收到的撤銷 handler（繞過停用的選單項） */
  async function triggerUndoHandler() {
    const handler = rail.props!.onUndoImport as () => void;
    await act(async () => handler());
    await settle();
  }

  async function menuItem(label: string) {
    if (!document.querySelector('[role="menuitem"]')) {
      await act(async () => (document.querySelector(".rail-add") as HTMLElement).click());
      await settle(1);
    }
    const item = [...document.querySelectorAll('[role="menuitem"]')].find((candidate) =>
      candidate.textContent?.includes(label),
    ) as HTMLElement | undefined;
    return item;
  }

  async function closeMenu() {
    await act(async () => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
    await settle(1);
  }

  async function undoFromMenu() {
    const item = (await menuItem(t("undoLastImport")))!;
    await act(async () => item.click());
    await settle();
  }

  async function retractLine() {
    const button = [...document.querySelectorAll("button")].find(
      (candidate) => candidate.textContent === t("undoLast"),
    )!;
    await act(async () => button.click());
    await settle();
  }

  // 回歸：舊碼撤銷項不看鎖、撤銷不取鎖
  it("blocks undo while an import is in flight, then confirms the new receipt", async () => {
    const importing = deferred<unknown>();
    backend.handlers.import_character = () => importing.promise;
    await start();
    await pickFile();
    expect(called("import_character")).toHaveLength(1);

    const item = await menuItem(t("undoLastImport"));
    expect(item?.getAttribute("aria-disabled")).toBe("true");
    expect((await menuItem(t("importCard")))?.getAttribute("aria-disabled")).toBe("true");
    await closeMenu();
    await triggerUndoHandler();
    expect(dialogs.confirm).not.toHaveBeenCalled();
    expect(called("undo_last_import")).toHaveLength(0);

    receipts = [...receipts, receipt("丙角", "c3")];
    await act(async () => importing.resolve(importResult()));
    await settle();
    await undoFromMenu();
    expect(dialogs.confirm.mock.calls[0][0]).toContain("丙角");
    expect(called("undo_last_import")[0].args.expected).toMatchObject({ label: "丙角", count: 2 });
  });

  // 回歸：舊碼撤銷確認框期間匯入照走
  it("blocks imports while the undo confirm is open, and releases the lock on cancel", async () => {
    const answer = deferred<boolean>();
    dialogs.confirm.mockImplementationOnce(() => answer.promise);
    await start();
    await undoFromMenu();
    expect(dialogs.confirm).toHaveBeenCalledTimes(1);
    await pickFile();
    expect(called("import_character")).toHaveLength(0);
    expect((await menuItem(t("importCard")))?.getAttribute("aria-disabled")).toBe("true");
    await closeMenu();

    await act(async () => answer.resolve(false));
    await settle();
    // 防漏放：取消後鎖已放
    await pickFile();
    expect(called("import_character")).toHaveLength(1);
  });

  // 回歸：匯入排在回合（此處用收回上一句的非 AI 忙碌）後面時撤銷照走
  it("blocks undo while an import waits for the turn, and both proceed after it", async () => {
    const popping = deferred<unknown[]>();
    backend.handlers.pop_transcript_floor = () => popping.promise;
    await start();
    await retractLine();
    await pickFile();
    expect(called("import_character")).toHaveLength(0);
    await triggerUndoHandler();
    expect(dialogs.confirm).not.toHaveBeenCalled();

    await act(async () => popping.resolve([]));
    await vi.waitFor(() => expect(called("import_character")).toHaveLength(1));
    await vi.waitFor(() => expect(rail.props!.undoImportLocked).toBe(false));
    // 防漏放：回合結束、匯入完成後可撤銷
    await undoFromMenu();
    expect(called("undo_last_import")).toHaveLength(1);
  });

  // 回歸：撤銷確認後排在回合後面時匯入照走
  it("blocks imports while a confirmed undo waits for the turn, and both proceed after it", async () => {
    const popping = deferred<unknown[]>();
    backend.handlers.pop_transcript_floor = () => popping.promise;
    await start();
    await retractLine();
    await undoFromMenu();
    expect(dialogs.confirm).toHaveBeenCalledTimes(1);
    expect(called("undo_last_import")).toHaveLength(0);
    await pickFile();
    expect(called("import_character")).toHaveLength(0);

    await act(async () => popping.resolve([]));
    await vi.waitFor(() => expect(called("undo_last_import")).toHaveLength(1));
    await vi.waitFor(() => expect(rail.props!.importLocked).toBe(false));
    // 防漏放
    await pickFile();
    expect(called("import_character")).toHaveLength(1);
  });

  // 貼開場白在途時撤銷項停用、handler 放棄；貼完恢復
  it("blocks undo while an opening line is being posted", async () => {
    const posting = deferred<unknown>();
    backend.handlers.card_openings = () => ["主開場"];
    backend.handlers.translate_tier_models = () => [];
    backend.handlers.post_opening = () => posting.promise;
    await start();
    await pickFile();
    await settle();
    const head = document.querySelector(".opening-choice-head") as HTMLElement;
    await act(async () => head.click());
    await settle(1);
    const post = [...document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes(t("openingLineOk")),
    )!;
    await act(async () => post.click());
    await settle();
    expect(called("post_opening")).toHaveLength(1);

    expect((await menuItem(t("undoLastImport")))?.getAttribute("aria-disabled")).toBe("true");
    await closeMenu();
    await triggerUndoHandler();
    expect(dialogs.confirm).not.toHaveBeenCalled();

    await act(async () =>
      posting.resolve({ ts: "t", speaker_id: "", speaker_name: "GM", kind: "narration", text: "主開場" }),
    );
    await settle();
    expect((await menuItem(t("undoLastImport")))?.getAttribute("aria-disabled")).toBeNull();
    await closeMenu();
    await triggerUndoHandler();
    expect(dialogs.confirm).toHaveBeenCalledTimes(1);
  });

  // 防漏放：後端回錯時顯示錯誤、鎖已放
  it("shows a failed undo and still allows importing", async () => {
    backend.handlers.undo_last_import = () => {
      throw "撤銷失敗了";
    };
    await start();
    await undoFromMenu();
    expect(document.body.textContent).toContain("撤銷失敗了");
    await pickFile();
    expect(called("import_character")).toHaveLength(1);
  });

  // 新行為：後端說收據已換過 → 報錯並重讀收據，下次確認框是新的最後一筆
  it("rereads receipts when the backend says the last receipt changed", async () => {
    backend.handlers.undo_last_import = () => {
      receipts = [...receipts, receipt("重構卡", "")];
      throw `TTMSG:${JSON.stringify({ code: "import_receipt_changed" })}`;
    };
    await start();
    const reads = called("list_import_receipts").length;
    await undoFromMenu();
    expect(called("list_import_receipts").length).toBeGreaterThan(reads);
    expect(document.body.textContent).toContain(backendText(`TTMSG:${JSON.stringify({ code: "import_receipt_changed" })}`));

    dialogs.confirm.mockClear();
    await undoFromMenu();
    expect(dialogs.confirm.mock.calls[0][0]).toContain("重構卡");
  });
});
