// @vitest-environment happy-dom

// 選卡開房的時點：全部匯完（身分框、路由判定、寫檔、開場旁白都結束）才綁房間；中途離開那張桌就不開房
import { act, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const calls: string[] = [];
const payloads: unknown[] = [];
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, payload?: unknown) => {
    calls.push(command);
    payloads.push(payload);
    if (command === "create_world") return Promise.resolve("w1");
    if (command === "room_enter") return Promise.resolve({ roomCode: "ABCD2345" });
    if (command === "room_tables") return Promise.resolve([]);
    if (command === "room_status") return Promise.resolve({ active: false, worldId: null, server: "http://s" });
    return Promise.resolve();
  },
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: () => Promise.resolve(true) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: () => Promise.resolve() }));

const { useRoomFlows } = await import("./useRoomFlows");
const { OpenRoomDialog } = await import("./OpenRoomDialog");
const { GenerateTableDialog } = await import("../lobby/GenerateTableDialog");
const { t } = await import("../../i18n");
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
let created: (worldId: string) => Promise<void> = async () => {};

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
  created = (worldId) => flows.openGenerated(worldId) ?? Promise.resolve();
  return (
    <>
      {flows.dialog && (
        <OpenRoomDialog
          server={flows.dialog.server}
          existingTable={flows.dialog.world ? { name: flows.dialog.name ?? "" } : undefined}
          onCancel={flows.closeDialog}
          onOpen={(files, server) => void flows.open(files, server)}
          onGenerate={(server) => flows.beginGenerate(server)}
        />
      )}
      <GenerateTableDialog
        open={flows.genesisServer !== null}
        room={flows.genesisServer !== null}
        onClose={flows.cancelGenerate}
        onCreated={created}
        transport="api"
      />
    </>
  );
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
  payloads.length = 0;
  importFile.mockClear();
  knobs = { table: "", importPending: false, openingPending: false };
  enterTable = (id) => render({ table: id });
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  await render({});
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

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

const buttonNamed = (text: string) => [...document.querySelectorAll("button")].find((button) => button.textContent === text);

describe("一句話生成世界開房", () => {
  async function openIdea() {
    await act(async () => {
      await flows.start();
    });
    await act(async () => {
      buttonNamed(t("roomFromIdea"))!.click();
    });
  }

  it("點一句話生成：開房窗關掉，一句話對話窗出現", async () => {
    await openIdea();
    expect(document.body.textContent).not.toContain(t("roomPickCards"));
    expect(document.querySelector(".gen-table-dialog")?.textContent).toContain(t("genRoomTitle"));
  });

  it("沒建就關：不建桌、不開房", async () => {
    await openIdea();
    await act(async () => {
      document.querySelector<HTMLButtonElement>(".gen-table-dialog .dialog-close")!.click();
    });
    expect(document.querySelector(".gen-table-dialog")).toBeNull();
    expect(calls).not.toContain("create_world");
    expect(calls).not.toContain("room_enter");
  });

  it("onCreated 用生成的桌開房，不改桌名", async () => {
    await openIdea();
    await act(async () => {
      await created("w1");
    });
    const enter = payloads[calls.lastIndexOf("room_enter")] as { worldId: string };
    expect(enter.worldId).toBe("w1");
    expect(calls).not.toContain("create_world");
    expect(calls).not.toContain("rename_world");
  });
});
