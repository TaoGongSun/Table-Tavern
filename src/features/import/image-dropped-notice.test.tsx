// @vitest-environment happy-dom
// 卡圖救不回（後端回 image_dropped）時匯入完成要講：角色卡沒有隨身世界書也要講，世界書接在完成訊息後。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";

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

import { message as showMessage } from "@tauri-apps/plugin-dialog";
import { useImportController, type ImportController } from "./useImportController";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root | null = null;
let host: HTMLElement | null = null;
let imports: ImportController | null = null;
const noop = async () => {};
const errors: string[] = [];

function Harness() {
  imports = useImportController({
    worldId: "W",
    lang: "zh-TW",
    castSize: 0,
    refreshCharacters: async () => [],
    reloadGmImage: noop,
    refreshInterfaces: async () => [],
    openIfDrawable: () => {},
    adoptTableName: noop,
    focusSpeaker: () => {},
    openTableForImport: async () => null,
    runTableOp: async (fn) => fn(),
    resetChatted: () => {},
    refreshState: noop,
    isTurnRunning: () => false,
    onError: (message) => message && errors.push(message),
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

function shown() {
  return vi.mocked(showMessage).mock.calls.map((call) => String(call[0]));
}

async function importFile(name: string) {
  await act(async () => imports!.importFile(new File([new Uint8Array([1, 2, 3])], name)));
}

beforeEach(async () => {
  vi.mocked(showMessage).mockClear();
  backend.handlers = {
    list_import_receipts: () => [],
    card_openings: () => [],
    translate_tier_models: () => [],
    read_worldbook: () => [],
    // 解析不出來＝直接走角色路徑
    probe_import: () => ({
      lorebook_heavy: false,
      parsed: false,
      book_entries: 0,
      book_shaped: false,
      alternate_greetings: 0,
    }),
  };
  errors.length = 0;
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

describe("卡圖沒存成的提示", () => {
  it("角色卡沒有隨身世界書：圖沒存成照樣跳提示", async () => {
    backend.handlers.import_character = () => ({
      meta,
      book: { imported: 0, skipped: 0 },
      source: null,
      image_dropped: true,
    });
    await importFile("莉亞.png");
    expect(errors).toEqual([]);
    expect(shown()).toEqual([t("importCardImageNotSaved")]);
  });

  it("角色卡有隨身世界書：提示接在收編數字後", async () => {
    backend.handlers.import_character = () => ({
      meta,
      book: { imported: 2, skipped: 0 },
      source: null,
      image_dropped: true,
    });
    await importFile("莉亞.png");
    expect(shown()).toHaveLength(1);
    expect(shown()[0]).toContain(t("worldbookImportDone", { n: 2 }));
    expect(shown()[0]).toContain(t("importCardImageNotSaved"));
  });

  it("角色卡圖存成了：不多講", async () => {
    backend.handlers.import_character = () => ({
      meta,
      book: { imported: 0, skipped: 0 },
      source: null,
      image_dropped: false,
    });
    await importFile("莉亞.png");
    expect(shown()).toEqual([]);
  });

  it("世界書：GM 封面圖沒存成接在完成訊息後，存成了就不提", async () => {
    backend.handlers.probe_import = () => ({
      lorebook_heavy: true,
      parsed: true,
      book_entries: 1,
      book_shaped: true,
      alternate_greetings: 0,
    });
    backend.handlers.import_worldbook = () => ({
      imported: 1,
      skipped: 0,
      source: null,
      image_dropped: true,
    });
    await importFile("書.png");
    expect(shown()).toHaveLength(1);
    expect(shown()[0]).toContain(t("worldbookImportDone", { n: 1 }));
    expect(shown()[0]).toContain(t("worldbookCoverNotSaved"));

    vi.mocked(showMessage).mockClear();
    backend.handlers.import_worldbook = () => ({
      imported: 1,
      skipped: 0,
      source: null,
      image_dropped: false,
    });
    await importFile("書.png");
    expect(shown()).toEqual([t("worldbookImportDone", { n: 1 })]);
  });
});
