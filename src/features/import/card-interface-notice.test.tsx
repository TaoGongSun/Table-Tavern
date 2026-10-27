// @vitest-environment happy-dom
// 匯入完成後的「自帶介面」提示：只有替換範本真的帶整頁 HTML 的卡才跳；加密卡與雲端載入器卡的說明照舊。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { CardInterface } from "../card-interface/interface-card";

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
let ifaces: CardInterface[] = [];
const noop = async () => {};
const errors: string[] = [];

function Harness() {
  imports = useImportController({
    worldId: "W",
    lang: "zh-TW",
    castSize: 0,
    refreshCharacters: async () => [],
    reloadGmImage: noop,
    refreshInterfaces: async () => ifaces,
    openIfDrawable: () => {},
    adoptTableName: noop,
    focusSpeaker: () => {},
    openTableForImport: async () => null,
    openWebSaveTable: async () => null,
    runTableOp: async (fn) => fn(),
    resetChatted: () => {},
    refreshState: noop,
    isTurnRunning: () => false,
    onError: (message) => message && errors.push(message),
    onWorldbookChanged: () => {},
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

function card(overrides: Partial<CardInterface>): CardInterface {
  return { character_id: "c1", character_name: "莉亞", scripts: [], unsupported: null, opening: null, mvu: false, ...overrides };
}

const script = (replace_string: string) => ({
  name: "s",
  find_regex: "/x/",
  replace_string,
  trim_strings: [],
  min_depth: null,
  max_depth: null,
});

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
  ifaces = [];
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

describe("匯入後的卡片介面提示", () => {
  beforeEach(() => {
    backend.handlers.import_character = () => ({ meta, book: { imported: 0, skipped: 0 }, source: null, image_dropped: false });
  });

  it("只有美化 regex 的卡：不跳", async () => {
    ifaces = [card({ scripts: [script("<span>x</span>")] })];
    await importFile("莉亞.png");
    expect(shown()).toEqual([]);
  });

  it("替換範本帶整頁 HTML 的卡：跳", async () => {
    ifaces = [card({ scripts: [script("<html><body>x</body></html>")] })];
    await importFile("莉亞.png");
    expect(shown()).toEqual([t("importCardInterface")]);
  });

  it("加密卡與雲端載入器卡：各自的說明保留", async () => {
    ifaces = [card({ unsupported: "scrypt" })];
    await importFile("莉亞.png");
    expect(shown()).toEqual([t("importCardScrypt")]);

    vi.mocked(showMessage).mockClear();
    ifaces = [card({ unsupported: "remote_loader" })];
    await importFile("莉亞.png");
    expect(shown()).toEqual([t("importCardRemoteLoader")]);
  });
});
