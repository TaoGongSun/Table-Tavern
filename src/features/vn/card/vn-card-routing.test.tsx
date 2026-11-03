// @vitest-environment happy-dom
// 初始 VN 卡匯入分流（plans/vn-mode-pkg9.md §2.2）：認得的版本跳分級框、開新 VN 桌；不認得就提示後照普通卡走。
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../../i18n";
import type { Bible, VnCardImport, VnCardProbe } from "../../../shared/contracts/backend-contracts";
import { looksLikeWebSave } from "../../../shared/contracts/web-save/web-save";

const backend = vi.hoisted(() => ({ calls: [] as string[], probe: null as unknown }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    backend.calls.push(command);
    if (command === "probe_import") return backend.probe;
    if (command === "list_import_receipts") return [];
    return null;
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
  save: vi.fn(async () => null),
  open: vi.fn(async () => null),
}));

import { message as showMessage } from "@tauri-apps/plugin-dialog";
import { useImportController, type ImportController } from "../../import/useImportController";
import { restoresCardBible, vnCardConfirmText } from "./vn-card";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const card: VnCardProbe = {
  supported: true,
  title: "港口的燈",
  rating: { level: "teen", warnings: ["grief"] },
  cast: ["凱", "米拉"],
};
let root: Root | null = null;
let imports: ImportController | null = null;
const opened: [number[], boolean][] = [];
const entered: [string, boolean | undefined][] = [];
const noop = async () => {};

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
    onVnTableImported: (worldId, fromCard) => entered.push([worldId, fromCard]),
    openVnCardTable: async (data, spoilerLock): Promise<VnCardImport> => {
      opened.push([data, spoilerLock]);
      return { world_id: "NEW", notices: [{ kind: "clipped", target: "world" }] };
    },
    openWebSaveTable: async () => null,
    runTableOp: async (fn) => fn(),
    resetChatted: () => {},
    refreshState: noop,
    isTurnRunning: () => false,
    onError: () => {},
    onWorldbookChanged: () => {},
    onRefactorCard: () => {},
  });
  return null;
}

beforeEach(async () => {
  vi.mocked(showMessage).mockClear();
  backend.calls.length = 0;
  opened.length = 0;
  entered.length = 0;
  root = createRoot(document.createElement("div"));
  await act(async () => root?.render(<Harness />));
});

afterEach(() => {
  act(() => root?.unmount());
  root = null;
});

const probeOf = (vn: VnCardProbe) => ({ parsed: true, name: "港口的燈", lorebook_heavy: false, book_entries: 2, book_shaped: false, alternate_greetings: 0, vn_card: vn });

describe("VN 卡匯入分流", () => {
  it("認得的版本：分級框確認後開新 VN 桌、顯示夾斷提示、精靈從 ③ 起，不走角色卡匯入", async () => {
    backend.probe = probeOf(card);
    await act(async () => imports!.importFile(new File(["{}"], "card.json")));
    expect(opened).toEqual([[Array.from(new TextEncoder().encode("{}")), true]]);
    expect(entered).toEqual([["NEW", true]]);
    expect(backend.calls).not.toContain("import_character");
    expect(String(vi.mocked(showMessage).mock.calls[0][0])).toBe(t("vnCardNotice_clipped", { target: "world" }));
  });

  it("精靈 ① 選的劇透鎖帶進建桌", async () => {
    backend.probe = probeOf(card);
    await act(async () => imports!.importFileToVnTable(new File(["{}"], "card.json"), false));
    expect(opened[0][1]).toBe(false);
  });

  it("版本不認得：提示後改當普通卡（問身分），不建 VN 桌", async () => {
    backend.probe = probeOf({ ...card, supported: false });
    await act(async () => imports!.importFile(new File(["{}"], "card.json")));
    expect(String(vi.mocked(showMessage).mock.calls[0][0])).toBe(t("vnCardUnsupported"));
    expect(opened).toEqual([]);
    expect(imports!.choice?.name).toBe("港口的燈");
  });

  it("分級框列出作品名、分級與警示；VN 卡 JSON 不會被當成網頁存檔", () => {
    const text = vnCardConfirmText(card);
    expect(text).toContain("港口的燈");
    expect(text).toContain(t("vnCardRating_teen"));
    expect(text).toContain("grief");
    const fixture = readFileSync(resolve(process.cwd(), "src-tauri/src/import/vn_card/fixtures/harbor.json"));
    expect(looksLikeWebSave(new Uint8Array(fixture))).toBe(false);
  });
});

describe("restoresCardBible", () => {
  it("restores only when the card stored an original bible (same condition as the backend)", () => {
    const original = {} as Bible;
    expect(restoresCardBible({ card: { protagonist_fixed: false, original_bible: original } })).toBe(true);
    // bible: null 的卡匯入後在分頁生成了聖經：卡上沒有原版，按鈕照常是重骰
    expect(restoresCardBible({ card: { protagonist_fixed: false, original_bible: null } })).toBe(false);
    expect(restoresCardBible({ card: null })).toBe(false);
    expect(restoresCardBible(null)).toBe(false);
  });
});
