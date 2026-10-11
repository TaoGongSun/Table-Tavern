// @vitest-environment happy-dom
// 匯入寫完世界書要通知世界設定（import-refresh-world-editor）：後端回來後第一步通知、帶傳入的桌 id 與
// 擴大了可見度的條目；後端丟錯不通知；後續重讀失敗也已通知。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    const handler = backend.handlers[command];
    return handler ? handler(args) : null;
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
  save: vi.fn(async () => null),
}));

import { useImportController, type ImportController } from "./useImportController";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root | null = null;
let host: HTMLElement | null = null;
let imports: ImportController | null = null;
const errors: string[] = [];
// 依序記下：後端匯入、通知、名單重讀、GM 圖重讀
let log: string[] = [];
let notices: [string, number[]][] = [];
let failCharacters = false;
let failGmImage = false;

function Harness() {
  imports = useImportController({
    worldId: "W",
    lang: "zh-TW",
    castSize: 0,
    refreshCharacters: async () => {
      log.push("refreshCharacters");
      if (failCharacters) throw new Error("cast down");
      return [];
    },
    reloadGmImage: async () => {
      log.push("reloadGmImage");
      if (failGmImage) throw new Error("image down");
    },
    refreshInterfaces: async () => [],
    openIfDrawable: () => {},
    adoptTableName: async () => {},
    focusSpeaker: () => {},
    openTableForImport: async () => "W2",
    openWebSaveTable: async () => null,
    runTableOp: async (fn) => fn(),
    resetChatted: () => {},
    refreshState: async () => {},
    isTurnRunning: () => false,
    onError: (message) => message && errors.push(message),
    onWorldbookChanged: (worldId, widened) => {
      log.push("notify");
      notices.push([worldId, widened]);
    },
    onRefactorCard: () => {},
  });
  return null;
}

const meta = {
  id: "c1",
  name: "莉亞",
  color: "#e07a5f",
  avatar: "🎭",
  tier: "balanced",
  show_image: true,
  archived: false,
  auto_hidden: false,
};

const probe = (book: boolean, name: string | null) => ({
  lorebook_heavy: book,
  parsed: true,
  name,
  book_entries: 1,
  book_shaped: book,
  alternate_greetings: 0,
});

async function importFile(name: string) {
  await act(async () => imports!.importFile(new File([new Uint8Array([1, 2, 3])], name)));
}

beforeEach(async () => {
  log = [];
  notices = [];
  failCharacters = false;
  failGmImage = false;
  errors.length = 0;
  backend.handlers = {
    list_import_receipts: () => [],
    card_openings: () => [],
    translate_tier_models: () => [],
    read_worldbook: () => [],
    probe_import: () => probe(false, "莉亞"),
    import_character: () => {
      log.push("import_character");
      return { meta, book: { imported: 0, skipped: 1 }, source: null, image_dropped: false, book_failed: false, widened_entries: [4] };
    },
    import_worldbook: () => {
      log.push("import_worldbook");
      return { imported: 1, skipped: 0, source: null, image_dropped: false, widened_entries: [2, 3] };
    },
  };
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  await act(async () => root?.render(<Harness />));
});

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  imports = null;
});

describe("匯入通知世界設定", () => {
  it("角色卡路：後端回來後第一步通知，帶這桌 id 與擴大的條目", async () => {
    await importFile("莉亞.json");
    await act(async () => imports!.answerChoice("character"));
    expect(errors).toEqual([]);
    expect(log.slice(0, 3)).toEqual(["import_character", "notify", "refreshCharacters"]);
    expect(notices).toEqual([["W", [4]]]);
  });

  it("世界書路：後端回來後第一步通知，帶這桌 id 與擴大的條目", async () => {
    backend.handlers.probe_import = () => probe(true, "書");
    await importFile("書.json");
    expect(errors).toEqual([]);
    expect(log.slice(0, 3)).toEqual(["import_worldbook", "notify", "reloadGmImage"]);
    expect(notices).toEqual([["W", [2, 3]]]);
  });

  it("舊後端沒有 widened_entries：照樣通知、帶空清單", async () => {
    backend.handlers.import_character = () => ({ meta, book: { imported: 1, skipped: 0 }, source: null, image_dropped: false, book_failed: false });
    await importFile("莉亞.json");
    await act(async () => imports!.answerChoice("character"));
    expect(notices).toEqual([["W", []]]);
  });

  it("後端丟錯：不通知", async () => {
    backend.handlers.import_character = () => {
      throw new Error("bad card");
    };
    backend.handlers.import_worldbook = () => {
      throw new Error("bad book");
    };
    await importFile("莉亞.json");
    await act(async () => imports!.answerChoice("character"));
    backend.handlers.probe_import = () => probe(true, "書");
    await importFile("書.json");
    expect(errors).toHaveLength(2);
    expect(notices).toEqual([]);
  });

  it("後續名單／GM 圖重讀失敗：通知已送出", async () => {
    failCharacters = true;
    await importFile("莉亞.json");
    await act(async () => imports!.answerChoice("character"));
    failGmImage = true;
    backend.handlers.probe_import = () => probe(true, "書");
    await importFile("書.json");
    expect(errors).toEqual(["Error: cast down", "Error: image down"]);
    expect(notices).toEqual([
      ["W", [4]],
      ["W", [2, 3]],
    ]);
  });

  it("開新桌並匯入：通知帶新桌 id", async () => {
    await act(async () =>
      imports!.hydrate([{ kind: "character", label: "甲", timestamp: "t", character_id: "c0" }]),
    );
    await importFile("莉亞.json");
    await act(async () => imports!.answerChoice("character"));
    expect(imports!.route).not.toBeNull();
    await act(async () => imports!.answerRoute("new_table"));
    expect(notices).toEqual([["W2", [4]]]);
  });
});
