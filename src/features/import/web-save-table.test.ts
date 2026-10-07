import { describe, expect, it } from "vitest";
import type { WebSaveImport } from "./useImportController";
import { openWebSaveTable, type WebSaveTableIo } from "./web-save-table";

const imported: WebSaveImport = {
  world_id: "NEW",
  character_id: "C1",
  card_storage: { theme: "dark" },
  worldbook_entries: 0,
  shared_kept: 0,
};

const CLEANUP_INCOMPLETE =
  'TTMSG:{"code":"web_save_cleanup_incomplete","error":"STORAGE_REASON","leftovers":"extension:ext.example:theme"}';

function io(writes: boolean[], answers: boolean[], discardError?: string) {
  const calls: string[] = [];
  const value: WebSaveTableIo = {
    importSave: async () => {
      calls.push("import");
      return imported;
    },
    writeStorage: (worldId) => {
      calls.push(`write:${worldId}`);
      return writes.shift() ?? true;
    },
    askRetry: async () => {
      calls.push("ask");
      return answers.shift() ?? false;
    },
    discardImport: async (worldId, reason) => {
      calls.push(`discard:${worldId}:${reason}`);
      if (discardError) throw discardError;
    },
    confirmImport: async (worldId) => void calls.push(`confirm:${worldId}`),
    refreshWorlds: async () => void calls.push("refresh"),
    enterTable: async (id) => {
      calls.push(`enter:${id}`);
      return { entered: true, writable: true };
    },
    storageFailed: "STORAGE_FAILED",
    storageReason: "STORAGE_REASON",
  };
  return { value, calls };
}

describe("網頁存檔進桌前的卡片 storage", () => {
  it("寫成了先確認匯入（清掉未確認記錄）才進桌", async () => {
    const { value, calls } = io([true], []);
    await expect(openWebSaveTable(value, [1])).resolves.toEqual(imported);
    expect(calls).toEqual(["import", "write:NEW", "confirm:NEW", "refresh", "enter:NEW"]);
  });

  it("寫不進去：問玩家，重試寫成了才進桌", async () => {
    const { value, calls } = io([false, true], [true]);
    await expect(openWebSaveTable(value, [1])).resolves.toEqual(imported);
    expect(calls).toEqual([
      "import",
      "write:NEW",
      "ask",
      "write:NEW",
      "confirm:NEW",
      "refresh",
      "enter:NEW",
    ]);
  });

  it("寫不進去而玩家放棄：走專用撤回（跨桌層＋新桌），不確認、不進桌、匯入算失敗", async () => {
    const { value, calls } = io([false], [false]);
    await expect(openWebSaveTable(value, [1])).rejects.toBe("STORAGE_FAILED");
    expect(calls).toEqual(["import", "write:NEW", "ask", "discard:NEW:STORAGE_REASON"]);
  });

  it("放棄後清理沒做完：丟後端含殘留位置的回報", async () => {
    const { value, calls } = io([false], [false], CLEANUP_INCOMPLETE);
    const error = await openWebSaveTable(value, [1]).catch((caught: unknown) => caught);
    expect(error).toBe(CLEANUP_INCOMPLETE);
    expect(String(error)).toContain("extension:ext.example:theme");
    expect(calls).toEqual(["import", "write:NEW", "ask", "discard:NEW:STORAGE_REASON"]);
  });
});
