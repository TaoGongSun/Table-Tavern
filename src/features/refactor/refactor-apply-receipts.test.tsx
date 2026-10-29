// @vitest-environment happy-dom
// 套用與收據（refactor-undo-ai-apply）：AI 結果卡與重構卡各送哪種 origin、套用失敗後的補救刷新。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    backend.calls.push({ command, args });
    const handler = backend.handlers[command];
    return handler ? handler(args) : null;
  }),
  Channel: class {
    onmessage: (delta: string) => void = () => {};
  },
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { message as showMessage } from "@tauri-apps/plugin-dialog";
import { useRefactorWorkflow, type RefactorWorkflowController } from "./useRefactorWorkflow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const statuses: string[] = [];
const order: string[] = [];
let root: Root | null = null;
let host: HTMLElement | null = null;
let controller: RefactorWorkflowController | null = null;
const refreshAfterApply = vi.fn(async (_live: () => boolean, _changes: unknown) => {});
const flushDraft = vi.fn(async () => true);
const flushDraftForRecovery = vi.fn(async () => true);
const refreshReceipts = vi.fn(async () => {});

function Harness() {
  controller = useRefactorWorkflow({
    world: "W",
    worldName: "驛站",
    setStatusMessage: (message: string) => statuses.push(message),
    refreshAfterApply,
    flushDraft,
    flushDraftForRecovery,
    refreshReceipts,
    isTurnRunning: () => false,
  });
  return null;
}

async function mount() {
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  await act(async () => {
    root?.render(<Harness />);
  });
}

const OUTCOME = JSON.stringify({ characters: [{ name: "阿福", source_uids: ["1"] }] });

function applyArgs() {
  return backend.calls.filter((call) => call.command === "refactor_apply").map((call) => call.args);
}

async function pickCard() {
  await act(async () => controller!.pickRefactorOutcome(new File([OUTCOME], "card.json")));
}

/** 最小的 AI 重構流程：盤點出一條介面條目，展開成狀態欄 */
function installAiRun() {
  backend.handlers.refactor_survey = () => ({
    persons: [],
    interface_uids: ["21"],
    playable_interface_uids: [],
    frame_candidates: [],
    verdicts: [],
    splits: [],
    groups: [],
    fields: [],
    mode: "interface",
    raw: "",
  });
  backend.handlers.refactor_assemble_local = () => ({
    entries: [],
    characters: [],
    clean_person_names: [],
    dropped: [],
    unabsorbed: [],
    audit: [],
  });
  backend.handlers.refactor_expand = ({ entryUid }) => ({
    interface: {
      state_fields: { 地點: "前院" },
      source_uids: [entryUid],
      raw: "",
      shell: '<Status_block>\n地点: "{{地點}}"\n</Status_block>',
      rules: { 地點: { kind: "text", update: "replace", inject: "turn" } },
      guide: "",
    },
    raw: "",
  });
}

beforeEach(() => {
  statuses.length = 0;
  order.length = 0;
  backend.handlers = {};
  backend.calls = [];
  refreshAfterApply.mockReset().mockImplementation(async () => {
    order.push("refreshAfterApply");
  });
  flushDraft.mockReset().mockResolvedValue(true);
  flushDraftForRecovery.mockReset().mockImplementation(async () => {
    order.push("flush");
    return true;
  });
  refreshReceipts.mockReset().mockImplementation(async () => {
    order.push("receipts");
  });
  backend.handlers.card_interfaces = () => [];
  backend.handlers.refactor_rerun_status = () => "fresh";
  backend.handlers.refactor_card_open = (bytes) => ({
    card: JSON.parse(new TextDecoder().decode(bytes as unknown as Uint8Array)),
    assets: [],
    token: null,
  });
  backend.handlers.read_worldbook = () => [{ uid: 21, title: "美化状态栏", content: "" }];
});

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  controller = null;
});

describe("套用送的 origin", () => {
  it("重構卡送 card、不再帶 recordReceipt", async () => {
    await mount();
    await pickCard();
    backend.handlers.refactor_apply = () => ({ new_characters: 1, entry_changes: null });
    await act(async () => controller!.applyRefactor(controller!.selection!));
    expect(applyArgs()).toHaveLength(1);
    expect(applyArgs()[0].origin).toBe("card");
    expect(applyArgs()[0]).not.toHaveProperty("recordReceipt");
  });

  it("AI 結果卡送 ai", async () => {
    await mount();
    installAiRun();
    await act(async () => controller!.startRefactorRun("interface"));
    expect(controller!.outcome).not.toBeNull();
    backend.handlers.refactor_apply = () => ({ new_characters: 0, entry_changes: null });
    await act(async () => controller!.applyRefactor(controller!.selection!));
    expect(applyArgs()).toHaveLength(1);
    expect(applyArgs()[0].origin).toBe("ai");
  });
});

describe("套用結果框列出沒收編的規則與觸發", () => {
  it("略過的規則、觸發與矛盾欄位各一行（名稱最多列 5 個），並提示換模型重構", async () => {
    await mount();
    await pickCard();
    backend.handlers.refactor_apply = () => ({
      new_characters: 1,
      entry_changes: null,
      skipped: {
        rules: ["a", "b", "c", "d", "e", "f"],
        triggers: ["第7天斥候登場"],
        conflicts: ["天數"],
      },
    });
    await act(async () => controller!.applyRefactor(controller!.selection!));
    const text = vi.mocked(showMessage).mock.calls.slice(-1)[0][0];
    expect(text).toContain("6 條規則");
    expect(text).toContain("a、b、c、d、e…");
    expect(text).not.toContain("f");
    expect(text).toContain("第7天斥候登場");
    expect(text).toContain("天數");
    expect(text).toContain("換較強的模型");
  });

  it("沒有略過的就不多出任何一行", async () => {
    await mount();
    await pickCard();
    backend.handlers.refactor_apply = () => ({
      new_characters: 1,
      entry_changes: null,
      skipped: { rules: [], triggers: [], conflicts: [] },
    });
    await act(async () => controller!.applyRefactor(controller!.selection!));
    expect(vi.mocked(showMessage).mock.calls.slice(-1)[0][0]).not.toContain("\n");
  });
});

describe("套用失敗後的補救刷新", () => {
  const PARTIAL = 'TTMSG:{"code":"refactor_apply_partial","error":"寫卡失敗"}';
  const NO_RECEIPT = 'TTMSG:{"code":"refactor_apply_partial_no_receipt","error":"寫卡失敗"}';

  for (const error of [PARTIAL, NO_RECEIPT]) {
    it(`${error.includes("no_receipt") ? "沒留收據" : "部分套用"}：先存草稿、刷新桌面、重讀收據，狀態列是原錯誤`, async () => {
      await mount();
      await pickCard();
      backend.handlers.refactor_apply = () => Promise.reject(error);
      await act(async () => controller!.applyRefactor(controller!.selection!));
      expect(order).toEqual(["flush", "refreshAfterApply", "receipts"]);
      expect(refreshAfterApply.mock.calls[0][1]).toBeNull();
      expect(statuses[statuses.length - 1]).toBe(error);
    });
  }

  it("桌面刷新 reject：收據仍重讀，狀態列是原錯誤", async () => {
    await mount();
    await pickCard();
    refreshAfterApply.mockImplementation(async () => {
      throw new Error("read_state failed");
    });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    backend.handlers.refactor_apply = () => Promise.reject(PARTIAL);
    await act(async () => controller!.applyRefactor(controller!.selection!));
    warn.mockRestore();
    expect(refreshReceipts).toHaveBeenCalledTimes(1);
    expect(statuses[statuses.length - 1]).toBe(PARTIAL);
    expect(controller!.busy).toBe(false);
  });

  it("草稿存不起來（含條目已被刪）：不刷新桌面，收據仍重讀，狀態列是原錯誤", async () => {
    await mount();
    await pickCard();
    flushDraftForRecovery.mockResolvedValue(false);
    backend.handlers.refactor_apply = () => Promise.reject(PARTIAL);
    await act(async () => controller!.applyRefactor(controller!.selection!));
    expect(refreshAfterApply).not.toHaveBeenCalled();
    expect(refreshReceipts).toHaveBeenCalledTimes(1);
    expect(statuses[statuses.length - 1]).toBe(PARTIAL);
  });

  it("後端關卡拒套（已遊玩、空選取）：不做補救刷新", async () => {
    await mount();
    await pickCard();
    for (const code of ["refactor-played", "refactor-empty-selection"]) {
      backend.handlers.refactor_apply = () => Promise.reject(code);
      await act(async () => controller!.applyRefactor(controller!.selection!));
    }
    expect(flushDraftForRecovery).not.toHaveBeenCalled();
    expect(refreshReceipts).not.toHaveBeenCalled();
  });
});
