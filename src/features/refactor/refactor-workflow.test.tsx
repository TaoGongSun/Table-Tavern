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
  // 開重構卡走後端同一入口：這裡模擬 JSON 原樣回傳（PNG 的測試自己覆寫）
  backend.handlers.refactor_card_open = (bytes) => ({
    card: JSON.parse(new TextDecoder().decode(bytes as unknown as Uint8Array)),
    assets: [],
    token: null,
  });
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

describe("重構卡 PNG：角色圖暫存的 token 生命週期", () => {
  const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0]);
  const CARD = {
    format: "table-tavern-refactor-card",
    version: 1,
    outcome: { characters: [{ name: "阿福", source_uids: ["1"] }] },
    applied: { characters: [{ outcome_index: 0, character_id: "c" }], player_index: null },
  };
  const opened = (token: string | null) => ({
    card: CARD,
    assets: token ? [{ outcome_index: 0, kind: "avatar" }] : [],
    token,
  });
  const releases = () =>
    backend.calls.filter((call) => call.command === "refactor_card_release").map((call) => call.args);

  async function openPng(token: string | null = "T1") {
    backend.handlers.refactor_card_open = () => opened(token);
    await act(async () => controller!.pickRefactorOutcome(new File([PNG], "card.png")));
  }

  it("JSON 與 PNG 都以原始 bytes 走後端入口", async () => {
    await mount();
    await act(async () =>
      controller!.pickRefactorOutcome(
        new File([JSON.stringify({ characters: [{ name: "阿福", source_uids: ["1"] }] })], "c.json"),
      ),
    );
    const open = backend.calls.find((call) => call.command === "refactor_card_open")!;
    expect(open.args).toBeInstanceOf(Uint8Array);
    expect(controller!.outcome?.characters[0].name).toBe("阿福");
  });

  it("PNG 結果卡帶角色圖張數；關卡才釋放、帶同一桌與 token", async () => {
    await mount();
    await openPng();
    expect(controller!.assetCount).toBe(1);
    expect(releases()).toHaveLength(0);
    await act(async () => controller!.closeRefactor());
    expect(releases()).toEqual([{ worldId: "W", token: "T1" }]);
    expect(controller!.assetCount).toBe(0);
  });

  it("先開 A 再開 B、B 先回 A 後回：留 B，A 的回覆作廢並只釋放 A", async () => {
    await mount();
    const finishers: ((value: unknown) => void)[] = [];
    backend.handlers.refactor_card_open = () => new Promise((done) => finishers.push(done));
    let a!: Promise<void>;
    let b!: Promise<void>;
    await act(async () => {
      a = controller!.pickRefactorOutcome(new File([PNG], "a.png"));
    });
    await act(async () => {
      b = controller!.pickRefactorOutcome(new File([PNG], "b.png"));
    });
    await vi.waitFor(() => expect(finishers).toHaveLength(2));
    await act(async () => {
      finishers[1](opened("TB"));
      await b;
    });
    await act(async () => {
      finishers[0](opened("TA"));
      await a;
    });
    expect(releases()).toEqual([{ worldId: "W", token: "TA" }]);
    backend.handlers.refactor_apply = () => ({ new_characters: 1 });
    await act(async () => controller!.applyRefactor(controller!.selection!));
    const apply = backend.calls.find((call) => call.command === "refactor_apply")!;
    expect(apply.args.assetToken).toBe("TB");
  });

  it("開檔途中關掉結果卡：舊回覆不重開結果卡、釋放它的 token", async () => {
    await mount();
    let finish!: (value: unknown) => void;
    backend.handlers.refactor_card_open = () => new Promise((done) => (finish = done));
    let picking!: Promise<void>;
    await act(async () => {
      picking = controller!.pickRefactorOutcome(new File([PNG], "card.png"));
    });
    await vi.waitFor(() => expect(finish).toBeTypeOf("function"));
    await act(async () => controller!.closeRefactor());
    await act(async () => {
      finish(opened("T7"));
      await picking;
    });
    expect(controller!.outcome).toBeNull();
    expect(releases()).toEqual([{ worldId: "W", token: "T7" }]);
  });

  it("開完回來已換桌：釋放剛開的那份、不顯示結果卡", async () => {
    await mount();
    let finish!: (value: unknown) => void;
    backend.handlers.refactor_card_open = () => new Promise((done) => (finish = done));
    let picking!: Promise<void>;
    await act(async () => {
      picking = controller!.pickRefactorOutcome(new File([PNG], "card.png"));
    });
    await vi.waitFor(() => expect(finish).toBeTypeOf("function"));
    await act(async () => root?.render(<Harness world="B" />));
    await act(async () => {
      finish(opened("T9"));
      await picking;
    });
    expect(controller!.outcome).toBeNull();
    expect(releases()).toEqual([{ worldId: "W", token: "T9" }]);
  });

  it("套用帶 token；成功後不再送釋放（後端已取走）", async () => {
    await mount();
    await openPng();
    backend.handlers.refactor_apply = () => ({ new_characters: 1, images_applied: 1, images_failed: [] });
    await act(async () => controller!.applyRefactor(controller!.selection!));
    const apply = backend.calls.find((call) => call.command === "refactor_apply")!;
    expect(apply.args.assetToken).toBe("T1");
    expect(releases()).toHaveLength(0);
    expect(controller!.outcome).toBeNull();
  });

  it("等回合期間卸載、套用沒送出：釋放 token", async () => {
    await mount();
    await openPng();
    turn.running = true;
    let pending!: Promise<void>;
    await act(async () => {
      pending = controller!.applyRefactor(controller!.selection!);
    });
    act(() => root?.unmount());
    root = null;
    turn.running = false;
    await pending;
    expect(commands()).not.toContain("refactor_apply");
    expect(releases()).toEqual([{ worldId: "W", token: "T1" }]);
  });

  it("已送後端、回來前卸載且被拒套（素材放回槽）：釋放 token", async () => {
    await mount();
    await openPng();
    let reject!: (reason: unknown) => void;
    backend.handlers.refactor_apply = () => new Promise((_, fail) => (reject = fail));
    let pending!: Promise<void>;
    await act(async () => {
      pending = controller!.applyRefactor(controller!.selection!);
    });
    await vi.waitFor(() => expect(commands()).toContain("refactor_apply"));
    act(() => root?.unmount());
    root = null;
    expect(releases()).toHaveLength(0);
    reject(new Error('TTMSG:{"code":"player_card_exists"}'));
    await pending;
    expect(releases()).toEqual([{ worldId: "W", token: "T1" }]);
  });

  it("後端回角色圖已不在：清掉結果卡、不送釋放", async () => {
    await mount();
    await openPng();
    backend.handlers.refactor_apply = () => {
      throw new Error('TTMSG:{"code":"refactor_assets_gone"}');
    };
    await act(async () => controller!.applyRefactor(controller!.selection!));
    expect(controller!.outcome).toBeNull();
    expect(releases()).toHaveLength(0);
  });

  it("其他拒套（玩家卡已存在）：結果卡留著、token 留著，關卡時才釋放", async () => {
    await mount();
    await openPng();
    backend.handlers.refactor_apply = () => {
      throw new Error('TTMSG:{"code":"player_card_exists"}');
    };
    await act(async () => controller!.applyRefactor(controller!.selection!));
    expect(controller!.outcome).not.toBeNull();
    await act(async () => controller!.closeRefactor());
    expect(releases()).toHaveLength(1);
  });
});
