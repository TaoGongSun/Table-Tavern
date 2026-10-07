// @vitest-environment happy-dom
// 網頁存檔併進既有匯入（D13）：依內容認出存檔就開新桌整桌匯入，不問身分、不走角色卡探測；
// 匯完講世界書規則差異，卡片介面照角色找。
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";

const backend = vi.hoisted(() => ({
  calls: [] as string[],
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    backend.calls.push(command);
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
import { useImportController, type ImportController, type WebSaveImport } from "./useImportController";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// happy-dom 底下 import.meta.url 不是檔案網址：照 repo 根目錄（vitest 的工作目錄）找契約 fixture
const fixture = (name: string) => readFileSync(resolve(process.cwd(), "src/shared/contracts/web-save", name));

let root: Root | null = null;
let imports: ImportController | null = null;
const noop = async () => {};
const opened: number[][] = [];
const focused: (string | null)[] = [];
const interfaceChecks: string[] = [];
let result: WebSaveImport | null = null;

function Harness() {
  imports = useImportController({
    worldId: "W",
    lang: "zh-TW",
    castSize: 0,
    refreshCharacters: async () => [],
    reloadGmImage: noop,
    refreshInterfaces: async (worldId) => {
      interfaceChecks.push(worldId);
      return [];
    },
    openIfDrawable: () => {},
    adoptTableName: noop,
    focusSpeaker: (id) => focused.push(id),
    openTableForImport: async () => null,
    openWebSaveTable: async (data) => {
      opened.push(data);
      return result;
    },
    runTableOp: async (fn) => fn(),
    resetChatted: () => {},
    refreshState: noop,
    isTurnRunning: () => false,
    onError: () => {},
    onRefactorCard: () => {},
  });
  return null;
}

async function importBytes(bytes: Uint8Array, name: string) {
  await act(async () => imports!.importFile(new File([bytes], name)));
}

beforeEach(async () => {
  vi.mocked(showMessage).mockClear();
  backend.calls.length = 0;
  backend.handlers = { list_import_receipts: () => [] };
  opened.length = 0;
  focused.length = 0;
  interfaceChecks.length = 0;
  result = {
    world_id: "NEW",
    character_id: "C1",
    card_storage: {},
    worldbook_entries: 5,
    shared_kept: 1,
  };
  root = createRoot(document.createElement("div"));
  await act(async () => root?.render(<Harness />));
});

afterEach(() => {
  act(() => root?.unmount());
  root = null;
  imports = null;
});

describe("網頁存檔匯入分流", () => {
  it("認出存檔就整桌匯入：不探測身分、不走角色卡匯入，匯完講規則差異並看新桌的卡片介面", async () => {
    const bytes = new Uint8Array(fixture("short.json"));
    await importBytes(bytes, "存檔.json");
    expect(opened).toEqual([Array.from(bytes)]);
    expect(backend.calls).not.toContain("probe_import");
    expect(backend.calls).not.toContain("import_character");
    expect(focused).toEqual(["C1"]);
    const shown = vi.mocked(showMessage).mock.calls.map((call) => String(call[0]));
    expect(shown).toHaveLength(1);
    expect(shown[0]).toContain(t("importWebSaveDone"));
    expect(shown[0]).toContain(t("importWebSaveWorldbookRules"));
    expect(shown[0]).toContain(t("importWebSaveSharedKept", { n: 1 }));
    expect(interfaceChecks).toEqual(["NEW"]);
  });

  it("版號不認得的存檔也交給存檔匯入（由後端說明），不當成角色卡", async () => {
    result = null;
    await importBytes(new Uint8Array(fixture("future-version.json")), "未來.json");
    expect(opened).toHaveLength(1);
    expect(backend.calls).not.toContain("probe_import");
    expect(vi.mocked(showMessage)).not.toHaveBeenCalled();
  });

  it("一般角色卡 JSON 照舊探測身分", async () => {
    backend.handlers.probe_import = () => ({
      lorebook_heavy: false,
      parsed: true,
      name: "莉亞",
      book_entries: 0,
      book_shaped: false,
      alternate_greetings: 0,
    });
    await importBytes(new TextEncoder().encode('{"spec":"chara_card_v2","data":{"name":"莉亞"}}'), "莉亞.json");
    expect(opened).toEqual([]);
    expect(backend.calls).toContain("probe_import");
  });
});
