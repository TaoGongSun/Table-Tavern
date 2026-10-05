// @vitest-environment happy-dom
// 匯入回傳的原檔識別要原樣帶到 post_opening.import_source（照 App 的接線：chat.postOpening(…, imports.openingsSource)），
// 後端才把開場白序號掛到這一筆匯入；沒留收據（source 為 null）時送 null。
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
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { useImportController, type ImportController } from "./useImportController";
import { useChatController, type ChatController } from "../play/useChatController";
import { useOpeningPost } from "./useOpeningPost";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const turn = { running: false };
let root: Root | null = null;
let host: HTMLElement | null = null;
let imports: ImportController | null = null;
let chat: ChatController | null = null;
let openingPost: ReturnType<typeof useOpeningPost> | null = null;
const errors: string[] = [];
const noop = async () => {};

// 照 App 的接線：同一個 worldId 給 imports、chat 與貼出開場白
function Harness({ world }: { world: string }) {
  imports = useImportController({
    worldId: world,
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
    isTurnRunning: () => turn.running,
    onError: (message) => message && errors.push(message),
    onRefactorCard: () => {},
  });
  chat = useChatController({
    worldId: world,
    scene: 0,
    config: null,
    speaker: "gm",
    gmTargeted: true,
    metaOf: () => undefined,
    playerName: undefined,
    castCount: 0,
    onArrived: () => {},
    refreshState: noop,
    refreshWorlds: noop,
    noteChatStarted: () => {},
    markCliConnected: noop,
    onError: (message) => message && errors.push(message),
    onTurnFailed: ({ raw }) => errors.push(raw),
  });
  openingPost = useOpeningPost({ worldId: world, chat, imports });
  return null;
}

async function mount(world = "W") {
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  await act(async () => {
    root?.render(<Harness world={world} />);
  });
}

async function switchTo(world: string) {
  await act(async () => {
    root?.render(<Harness world={world} />);
  });
}

beforeEach(() => {
  errors.length = 0;
  backend.calls = [];
  backend.handlers = {
    probe_import: () => ({
      lorebook_heavy: true,
      parsed: true,
      book_entries: 1,
      book_shaped: true,
      alternate_greetings: 1,
    }),
    read_worldbook: () => [],
    list_import_receipts: () => [],
    card_openings: () => ["主開場", "備用開場"],
    translate_tier_models: () => [],
    post_opening: (args) => ({
      ts: args.ts,
      speaker_id: "",
      speaker_name: "GM",
      kind: "narration",
      text: args.text,
    }),
  };
});

afterEach(() => {
  turn.running = false;
  act(() => root?.unmount());
  host?.remove();
  root = null;
  imports = null;
  chat = null;
});

async function importThenPost(source: string | null) {
  backend.handlers.import_worldbook = () => ({ imported: 1, skipped: 0, source });
  await act(async () => imports!.importFile(new File([new Uint8Array([1, 2, 3])], "乙.json")));
  expect(imports!.openings).toEqual(["主開場", "備用開場"]);
  await act(async () => {
    await openingPost!.post("備用開場", 1);
  });
  const posted = backend.calls.filter((call) => call.command === "post_opening");
  return posted[posted.length - 1]!.args;
}

describe("開場白的匯入識別", () => {
  it("匯入回傳的 source 原樣送到 post_opening.importSource，序號一起帶", async () => {
    await mount();
    const args = await importThenPost("import-source-B.json");
    expect(args).toMatchObject({ importSource: "import-source-B.json", openingIndex: 1 });
  });

  it("沒留收據的匯入（source 為 null）送 null，不沿用上一次匯入的識別", async () => {
    await mount();
    await importThenPost("import-source-A.json");
    const args = await importThenPost(null);
    expect(args).toMatchObject({ importSource: null, openingIndex: 1 });
  });
});

describe("匯入排在進行中的回合後面", () => {
  it("回合進行中匯入：回合結束前不送後端，等待提示亮到後端回來", async () => {
    await mount();
    let resolve!: (value: unknown) => void;
    backend.handlers.import_worldbook = () => new Promise((done) => (resolve = done));
    turn.running = true;
    let pending!: Promise<void>;
    await act(async () => {
      pending = imports!.importFile(new File([new Uint8Array([1])], "乙.json"));
    });
    expect(imports!.waitingForTurn).toBe(true);
    await new Promise((done) => setTimeout(done, 120));
    expect(backend.calls.some((call) => call.command === "import_worldbook")).toBe(false);
    turn.running = false;
    await vi.waitFor(() =>
      expect(backend.calls.some((call) => call.command === "import_worldbook")).toBe(true),
    );
    await act(async () => {
      resolve({ imported: 1, skipped: 0, source: null });
      await pending;
    });
    expect(imports!.waitingForTurn).toBe(false);
  });
});

describe("貼出開場白等整個回合落檔才送", () => {
  function gate<T>() {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>((done) => (resolve = done));
    return { promise, resolve };
  }

  it("AI 串流結束（後端已放開）時還不送；GM 旁白落檔、刷新跑完 finally 後才送 post_opening", async () => {
    await mount();
    backend.handlers.import_worldbook = () => ({ imported: 1, skipped: 0, source: null });
    await act(async () => imports!.importFile(new File([new Uint8Array([1])], "甲.json")));
    const stream = gate<unknown>();
    const append = gate<void>();
    backend.handlers.gm_narrate = () => stream.promise;
    backend.handlers.append_transcript = async (args) => {
      await append.promise;
      return args.event;
    };
    let turnDone!: Promise<void>;
    await act(async () => {
      turnDone = chat!.gmNarrate();
    });
    expect(chat!.isBusy()).toBe(true);
    let posting!: Promise<void>;
    await act(async () => {
      posting = openingPost!.post("主開場", 0);
    });
    expect(openingPost!.waiting).toBe(true);
    // 串流結束：後端此刻已放開許可，但前端的 GM 旁白還沒落檔
    await act(async () => stream.resolve({ text: "GM 旁白", raw: null, next: null }));
    await vi.waitFor(() =>
      expect(backend.calls.some((call) => call.command === "append_transcript")).toBe(true),
    );
    await new Promise((done) => setTimeout(done, 120));
    expect(backend.calls.some((call) => call.command === "post_opening")).toBe(false);
    await act(async () => {
      append.resolve();
      await turnDone;
    });
    expect(chat!.isBusy()).toBe(false);
    await act(async () => {
      await posting;
    });
    const order = backend.calls.map((call) => call.command);
    expect(order.indexOf("post_opening")).toBeGreaterThan(order.lastIndexOf("append_transcript"));
    expect(openingPost!.waiting).toBe(false);
  });

  it("等待回合中換桌：開場白不送出，原面板留在原桌", async () => {
    await mount("A");
    backend.handlers.import_worldbook = () => ({ imported: 1, skipped: 0, source: null });
    await act(async () => imports!.importFile(new File([new Uint8Array([1])], "甲.json")));
    const stream = gate<unknown>();
    backend.handlers.gm_narrate = () => stream.promise;
    let turnDone!: Promise<void>;
    await act(async () => {
      turnDone = chat!.gmNarrate();
    });
    let posting!: Promise<void>;
    await act(async () => {
      posting = openingPost!.post("主開場", 0);
    });
    await switchTo("B");
    await act(async () => {
      stream.resolve({ text: "GM 旁白", raw: null, next: null });
      await turnDone;
      await posting;
    });
    await new Promise((done) => setTimeout(done, 120));
    expect(backend.calls.some((call) => call.command === "post_opening")).toBe(false);
    expect(errors).toEqual([]);
  });
});

describe("貼出開場白排隊期間換桌或卸載", () => {
  async function importAndQueuePost() {
    backend.handlers.import_worldbook = () => ({ imported: 1, skipped: 0, source: "import-source-A.json" });
    await act(async () => imports!.importFile(new File([new Uint8Array([1])], "甲.json")));
    let resolve!: (value: unknown) => void;
    backend.handlers.post_opening = (args) =>
      new Promise((done) => (resolve = () => done({ ts: args.ts, speaker_id: "", speaker_name: "GM", kind: "narration", text: args.text })));
    let pending!: Promise<void>;
    await act(async () => {
      pending = openingPost!.post("主開場", 0);
    });
    return { pending, resolve: () => resolve(null) };
  }

  it("A 桌貼出排隊中換到 B 桌並開了 B 的面板：A 回來後不加進 B 的畫面、不關 B 的面板", async () => {
    await mount("A");
    const queued = await importAndQueuePost();
    expect(openingPost!.busy).toBe(true);
    await switchTo("B");
    expect(openingPost!.busy).toBe(false);
    backend.handlers.import_worldbook = () => ({ imported: 1, skipped: 0, source: "import-source-B.json" });
    await act(async () => imports!.importFile(new File([new Uint8Array([2])], "乙.json")));
    expect(imports!.openingsWorldId).toBe("B");
    const eventsBefore = chat!.events.length;
    await act(async () => {
      queued.resolve();
      await queued.pending;
    });
    expect(chat!.events.length).toBe(eventsBefore);
    expect(imports!.openings).toEqual(["主開場", "備用開場"]);
    expect(imports!.openingsWorldId).toBe("B");
    const posted = backend.calls.filter((call) => call.command === "post_opening");
    expect(posted).toHaveLength(1);
    expect(posted[0]!.args.worldId).toBe("A");
    expect(errors).toEqual([]);
  });

  it("貼出排隊中元件卸載：回來時不報錯、不回寫", async () => {
    await mount("A");
    const queued = await importAndQueuePost();
    act(() => root?.unmount());
    root = null;
    queued.resolve();
    await expect(queued.pending).resolves.toBeUndefined();
    expect(errors).toEqual([]);
  });
});
