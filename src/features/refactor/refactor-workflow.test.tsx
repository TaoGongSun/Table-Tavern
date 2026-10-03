// @vitest-environment happy-dom
// 重新重構的入口判定與外框兩階段派發（refactor-statusbar-skeleton 待問 1、3）。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));
const dialogs = vi.hoisted(() => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
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
  confirm: dialogs.confirm,
  message: dialogs.message,
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { useRefactorWorkflow, type RefactorWorkflowController } from "./useRefactorWorkflow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const turn = { running: false };
let root: Root | null = null;
let host: HTMLElement | null = null;
let controller: RefactorWorkflowController | null = null;
const refreshAfterApply = vi.fn(async () => {});

function Harness({ world = "W" }: { world?: string }) {
  controller = useRefactorWorkflow({
    world,
    worldName: "驛站",
    setStatusMessage: () => {},
    refreshAfterApply,
    isTurnRunning: () => turn.running,
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

const commands = () => backend.calls.map((call) => call.command);

beforeEach(() => {
  backend.handlers = {};
  backend.calls = [];
  dialogs.confirm.mockReset().mockResolvedValue(true);
  dialogs.message.mockReset().mockResolvedValue(undefined);
  refreshAfterApply.mockReset().mockResolvedValue(undefined);
  backend.handlers.card_interfaces = () => [];
  backend.handlers.read_worldbook = () => [
    { uid: 21, title: "美化状态栏", content: "" },
    { uid: 22, title: "格式增强Plus", content: "" },
  ];
});

afterEach(() => {
  turn.running = false;
  act(() => root?.unmount());
  host?.remove();
  root = null;
  controller = null;
});

describe("runAiRefactor 重新重構判定", () => {
  it("已遊玩、沒有匯入原檔：提示後擋下，不清桌也不重構", async () => {
    await mount();
    for (const [status, key] of [
      ["played", "refactorRerunPlayed"],
      ["no_source", "refactorRerunNoSource"],
    ] as const) {
      backend.calls = [];
      backend.handlers.refactor_rerun_status = () => status;
      await act(async () => controller!.runAiRefactor());
      expect(dialogs.message).toHaveBeenLastCalledWith(t(key), expect.anything());
      expect(commands()).toEqual(["refactor_rerun_status"]);
    }
    expect(dialogs.confirm).not.toHaveBeenCalled();
  });

  it("未遊玩的重構桌：確認後清回原卡、刷新畫面，再照一般流程；取消就不動", async () => {
    await mount();
    backend.handlers.refactor_rerun_status = () => "ready";
    dialogs.confirm.mockResolvedValueOnce(false);
    await act(async () => controller!.runAiRefactor());
    expect(commands()).toEqual(["refactor_rerun_status"]);

    backend.calls = [];
    await act(async () => controller!.runAiRefactor());
    expect(dialogs.confirm).toHaveBeenLastCalledWith(t("refactorRerunWarnBody"), expect.anything());
    expect(commands().slice(0, 3)).toEqual([
      "refactor_rerun_status",
      "refactor_reset_to_source",
      "card_interfaces",
    ]);
    expect(refreshAfterApply).toHaveBeenCalledTimes(1);
  });

  it("清回已提交但臨時根沒清掉（committed_cleanup_pending）：照成功刷新、接著重構", async () => {
    await mount();
    backend.handlers.refactor_rerun_status = () => "ready";
    backend.handlers.refactor_reset_to_source = () => "committed_cleanup_pending";
    await act(async () => controller!.runAiRefactor());
    expect(commands().slice(0, 3)).toEqual([
      "refactor_rerun_status",
      "refactor_reset_to_source",
      "card_interfaces",
    ]);
    expect(refreshAfterApply).toHaveBeenCalledTimes(1);
  });

  it("沒重構過的桌照現狀：不確認、不清桌", async () => {
    await mount();
    backend.handlers.refactor_rerun_status = () => "fresh";
    await act(async () => controller!.runAiRefactor());
    expect(dialogs.confirm).not.toHaveBeenCalled();
    expect(commands()).not.toContain("refactor_reset_to_source");
    expect(commands()).toContain("card_interfaces");
  });
});

const STATUS = '<Status_block>\n地点: "{{地點}}"\n</Status_block>';

function installRun(definingShell: string) {
  backend.handlers.refactor_survey = () => ({
    persons: [],
    interface_uids: ["21", "22"],
    playable_interface_uids: [],
    frame_candidates: [{ uid: "22", tags: ["maintext", "Status_block"] }],
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
      raw: `STATE ${entryUid}`,
      shell: entryUid === "21" ? definingShell : STATUS,
      rules: { 地點: { kind: "text", update: "replace", inject: "turn" } },
      guide: "地點每回合必報",
    },
    raw: "",
  });
}

const expandedUids = () =>
  backend.calls.filter((call) => call.command === "refactor_expand").map((call) => call.args.entryUid);

describe("外框兩階段派發", () => {
  it("定義骨架含相同容器：外框不展開，只貢獻容器排法並隨介面消耗", async () => {
    await mount();
    installRun(STATUS);
    await act(async () => controller!.startRefactorRun("interface"));
    expect(expandedUids()).toEqual(["21"]);
    expect(controller!.outcome?.interface?.source_uids).toEqual(["21", "22"]);
    expect(controller!.outcome?.interface?.shell).toBe(`<maintext>\n{{本回合.正文}}\n</maintext>\n${STATUS}`);
    expect(controller!.outcome?.interface?.guide).toBe("地點每回合必報");
    expect(controller!.outcome?.preserve_source_uids).toBeUndefined();
  });

  it("沒有定義骨架含相同容器：候選照一般條目展開", async () => {
    await mount();
    installRun('<Other_block>\n地点: "{{地點}}"\n</Other_block>');
    await act(async () => controller!.startRefactorRun("interface"));
    expect(expandedUids()).toEqual(["21", "22"]);
  });
});

describe("套用重構排在進行中的回合後面", () => {
  const OUTCOME = JSON.stringify({ characters: [{ name: "阿福", source_uids: ["1"] }] });

  it("回合進行中按套用：回合結束前不送後端，等待提示亮到後端回來，期間再按不會送第二次", async () => {
    await mount();
    await act(async () => controller!.pickRefactorOutcome(new File([OUTCOME], "card.json")));
    let resolve!: (value: unknown) => void;
    backend.handlers.refactor_apply = () => new Promise((done) => (resolve = done));
    turn.running = true;
    let first!: Promise<void>;
    await act(async () => {
      first = controller!.applyRefactor(controller!.selection!);
    });
    expect(controller!.waitingForTurn).toBe(true);
    await act(async () => controller!.applyRefactor(controller!.selection!));
    await new Promise((done) => setTimeout(done, 120));
    expect(commands()).not.toContain("refactor_apply");
    turn.running = false;
    await vi.waitFor(() => expect(commands()).toContain("refactor_apply"));
    expect(controller!.waitingForTurn).toBe(true);
    await act(async () => {
      resolve({
        new_characters: 1,
        player_assigned: false,
        new_entries: 0,
        deleted_entries: 0,
        interface_applied: false,
        mechanisms_applied: 0,
      });
      await first;
    });
    expect(commands().filter((command) => command === "refactor_apply")).toHaveLength(1);
    expect(controller!.waitingForTurn).toBe(false);
  });

  it("等待回合中換桌：套用直接終止、不送後端，按鈕鎖放開", async () => {
    await mount();
    await act(async () => controller!.pickRefactorOutcome(new File([OUTCOME], "card.json")));
    turn.running = true;
    let first!: Promise<void>;
    await act(async () => {
      first = controller!.applyRefactor(controller!.selection!);
    });
    await act(async () => root?.render(<Harness world="B" />));
    turn.running = false;
    await act(async () => {
      await first;
    });
    await new Promise((done) => setTimeout(done, 120));
    expect(commands()).not.toContain("refactor_apply");
    expect(refreshAfterApply).not.toHaveBeenCalled();
    expect(dialogs.message).not.toHaveBeenCalled();
    expect(controller!.busy).toBe(false);
  });

  it("沒有回合在跑：不亮等待提示", async () => {
    await mount();
    await act(async () => controller!.pickRefactorOutcome(new File([OUTCOME], "card.json")));
    let resolve!: (value: unknown) => void;
    backend.handlers.refactor_apply = () => new Promise((done) => (resolve = done));
    let first!: Promise<void>;
    await act(async () => {
      first = controller!.applyRefactor(controller!.selection!);
    });
    expect(controller!.waitingForTurn).toBe(false);
    await act(async () => {
      resolve({ new_characters: 1 });
      await first;
    });
  });
});

describe("套用重構排隊期間換桌或卸載", () => {
  const OUTCOME = JSON.stringify({ characters: [{ name: "阿福", source_uids: ["1"] }] });

  async function queueApply() {
    await mount();
    await act(async () => controller!.pickRefactorOutcome(new File([OUTCOME], "card.json")));
    let resolve!: (value: unknown) => void;
    backend.handlers.refactor_apply = () => new Promise((done) => (resolve = done));
    let pending!: Promise<void>;
    await act(async () => {
      pending = controller!.applyRefactor(controller!.selection!);
    });
    return { pending, resolve: () => resolve({ new_characters: 1 }) };
  }

  it("換到別桌後舊的套用才回來：不關結果卡、不刷新、不跳訊息，按鈕鎖放開", async () => {
    const queued = await queueApply();
    await act(async () => root?.render(<Harness world="B" />));
    await act(async () => {
      queued.resolve();
      await queued.pending;
    });
    expect(refreshAfterApply).not.toHaveBeenCalled();
    expect(dialogs.message).not.toHaveBeenCalled();
    expect(controller!.outcome).not.toBeNull();
    expect(controller!.busy).toBe(false);
  });

  it("卸載後舊的套用才回來：不報錯、不刷新、不跳訊息", async () => {
    const queued = await queueApply();
    act(() => root?.unmount());
    root = null;
    queued.resolve();
    await expect(queued.pending).resolves.toBeUndefined();
    expect(refreshAfterApply).not.toHaveBeenCalled();
    expect(dialogs.message).not.toHaveBeenCalled();
  });

  it("套用回來、刷新到一半換桌：不跳完成訊息", async () => {
    const queued = await queueApply();
    let releaseRefresh!: () => void;
    refreshAfterApply.mockImplementation(() => new Promise<void>((done) => (releaseRefresh = done)));
    await act(async () => {
      queued.resolve();
    });
    await vi.waitFor(() => expect(refreshAfterApply).toHaveBeenCalledTimes(1));
    await act(async () => root?.render(<Harness world="B" />));
    await act(async () => {
      releaseRefresh();
      await queued.pending;
    });
    expect(dialogs.message).not.toHaveBeenCalled();
  });
});
