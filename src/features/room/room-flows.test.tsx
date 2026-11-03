// @vitest-environment happy-dom

// 選卡開房的時點：全部匯完（身分框、路由判定、寫檔、開場旁白都結束）才綁房間；中途離開那張桌就不開房
import { act, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const calls: string[] = [];
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string) => {
    calls.push(command);
    if (command === "create_world") return Promise.resolve("w1");
    if (command === "room_enter") return Promise.resolve({ roomCode: "ABCD2345" });
    if (command === "room_tables") return Promise.resolve([]);
    if (command === "room_status") return Promise.resolve({ active: false, worldId: null, server: "" });
    return Promise.resolve();
  },
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: () => Promise.resolve(true) }));

const { useRoomFlows } = await import("./useRoomFlows");
type Flows = ReturnType<typeof useRoomFlows>;

interface Knobs {
  table: string;
  importPending: boolean;
  openingPending: boolean;
}

let root: Root;
let host: HTMLDivElement;
let flows: Flows;
let knobs: Knobs;
let enterTable: (id: string) => Promise<void>;

function Probe({ k, importFile }: { k: Knobs; importFile: (file: File) => Promise<void> }): ReactNode {
  flows = useRoomFlows({
    table: k.table,
    busy: false,
    runTableOp: (op) => op(),
    enterTable: (id) => enterTable(id),
    refreshWorlds: () => Promise.resolve(),
    setError: () => {},
    importFile,
    importPending: k.importPending,
    openingPending: k.openingPending,
    aiReady: true,
    openAiSettings: () => {},
  });
  return null;
}

// 匯入函式：測試先把身分框打開（importPending）再讓它回來，跟真的 importFile 一樣
let finishImport: () => void = () => {};
const importFile = vi.fn(() => new Promise<void>((resolve) => (finishImport = resolve)));
async function importReturnsWithDialogOpen() {
  await render({ importPending: true });
  await act(async () => finishImport());
}
async function render(next: Partial<Knobs>) {
  knobs = { ...knobs, ...next };
  await act(async () => root.render(<Probe k={knobs} importFile={importFile} />));
}
const bound = () => calls.includes("room_enter");

beforeEach(async () => {
  calls.length = 0;
  importFile.mockClear();
  knobs = { table: "", importPending: false, openingPending: false };
  enterTable = (id) => render({ table: id });
  host = document.createElement("div");
  root = createRoot(host);
  await render({});
});
afterEach(() => act(() => root.unmount()));

describe("選卡開房", () => {
  it("身分框答完到路由判定、寫檔結束前都不綁房；結束才綁", async () => {
    await act(async () => void (await flows.open([new File(["x"], "a.json")], "http://s")));
    // 匯入函式回來時身分框開著
    await importReturnsWithDialogOpen();
    expect(importFile).toHaveBeenCalledTimes(1);
    expect(bound()).toBe(false);
    // 選了「匯入成世界書」：框收了，但路由判定與寫檔還在跑（answering 併在 importPending 裡）
    await render({ importPending: true });
    expect(bound()).toBe(false);
    // 接著跳出開場旁白
    await render({ importPending: false, openingPending: true });
    expect(bound()).toBe(false);
    await render({ openingPending: false });
    expect(bound()).toBe(true);
  });

  it("匯入途中離開那張桌：不開房，之後回來也不補開", async () => {
    await act(async () => void (await flows.open([new File(["x"], "a.json")], "http://s")));
    await importReturnsWithDialogOpen();
    await render({ table: "", importPending: false });
    await render({ table: "w1" });
    expect(bound()).toBe(false);
  });
});
