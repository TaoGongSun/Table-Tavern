// @vitest-environment happy-dom
// 逐字稿重讀與本地改動的競態：手改狀態欄後的重讀在途時，卡片寫入確認（換表）或追加事件先換進畫面，
// 重讀晚回不得把畫面蓋回舊版；在途期間再要求重讀只補讀一次。

import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, TranscriptEvent } from "../../shared/contracts/backend-contracts";

type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const invokeMock = vi.hoisted(() => vi.fn<Invoke>());
vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  Channel: class {
    onmessage: ((value: unknown) => void) | null = null;
  },
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import { useChatController, type ChatController } from "./useChatController";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => (resolve = done));
  return { promise, resolve };
}

const event = (vars: Record<string, unknown>, rev: string): TranscriptEvent => ({
  id: "e1",
  ts: "t1",
  speaker_id: "",
  speaker_name: "",
  kind: "narration",
  text: "開場",
  message_vars: vars,
  vars_rev: rev,
});

describe("chat reload 競態", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    invokeMock.mockReset();
  });

  let setWorld: (worldId: string) => void = () => {};

  function mount() {
    const config = { preferences: { max_round_speakers: 1 } } as unknown as AppConfig;
    let chat!: ChatController;
    function Harness() {
      const [worldId, setWorldId] = useState("w1");
      setWorld = setWorldId;
      chat = useChatController({
        worldId,
        scene: 0,
        config,
        speaker: "gm",
        gmTargeted: true,
        metaOf: () => undefined,
        playerName: undefined,
        castCount: 1,
        onArrived: () => {},
        refreshState: async () => {},
        refreshWorlds: async () => {},
        noteChatStarted: () => {},
        markCliConnected: async () => {},
        onError: () => {},
        onTurnFailed: () => {},
      });
      return null;
    }
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root!.render(<Harness />));
    return () => chat;
  }

  it("重讀讀到 v1 後卡片寫入換成 v2，晚回的 v1 不套用、重讀一次拿 v2", async () => {
    const v1 = event({ hp: 1 }, "r1");
    const v2 = event({ hp: 2 }, "r2");
    const reads: ReturnType<typeof deferred<TranscriptEvent[]>>[] = [];
    invokeMock.mockImplementation(async (command) => {
      if (command === "read_transcript") {
        const read = deferred<TranscriptEvent[]>();
        reads.push(read);
        return read.promise;
      }
      return null;
    });
    const chat = mount();
    act(() => chat().hydrate([v1]));

    let reloaded = false;
    act(() => void chat().reload().then(() => (reloaded = true)));
    expect(reads).toHaveLength(1);
    // 卡片寫入確認：換進 v2
    act(() => chat().replaceEvent(v1, v2));
    expect(chat().events[0].vars_rev).toBe("r2");
    // 在途的重讀回來時還是 v1（讀在寫入之前）
    await act(async () => reads[0].resolve([v1]));
    expect(chat().events[0].vars_rev).toBe("r2");
    expect(reads).toHaveLength(2);
    expect(reloaded).toBe(false);
    await act(async () => reads[1].resolve([v2]));
    expect(reloaded).toBe(true);
    expect(chat().events[0].vars_rev).toBe("r2");
    expect(invokeMock.mock.calls.filter(([command]) => command === "read_transcript")).toHaveLength(2);
  });

  it("重讀在途時追加的事件不被晚回的舊逐字稿吃掉；追加落檔中途的重讀等它落定才讀", async () => {
    const v1 = event({ hp: 1 }, "r1");
    const opening: TranscriptEvent = { ts: "t2", speaker_id: "", speaker_name: "", kind: "narration", text: "開場白" };
    const reads: ReturnType<typeof deferred<TranscriptEvent[]>>[] = [];
    const posted = deferred<TranscriptEvent>();
    invokeMock.mockImplementation(async (command) => {
      if (command === "read_transcript") {
        const read = deferred<TranscriptEvent[]>();
        reads.push(read);
        return read.promise;
      }
      if (command === "post_opening") return posted.promise;
      return null;
    });
    const chat = mount();
    act(() => chat().hydrate([v1]));

    act(() => void chat().reload());
    let opened: Promise<boolean> | null = null;
    act(() => void (opened = chat().postOpening("開場白")));
    // 讀在追加落檔之前回來：追加還在途，這份不套用
    await act(async () => reads[0].resolve([v1]));
    expect(chat().events).toEqual([v1]);
    expect(reads).toHaveLength(1);
    // 追加換進畫面後才補讀
    await act(async () => {
      posted.resolve(opening);
      await opened;
    });
    expect(chat().events).toEqual([v1, opening]);
    expect(reads).toHaveLength(2);
    await act(async () => reads[1].resolve([v1, opening]));
    expect(chat().events).toEqual([v1, opening]);
  });

  it("在途期間連續要求重讀合併成一趟補讀，呼叫端都等到最後一趟", async () => {
    const v1 = event({ hp: 1 }, "r1");
    const v3 = event({ hp: 3 }, "r3");
    const reads: ReturnType<typeof deferred<TranscriptEvent[]>>[] = [];
    invokeMock.mockImplementation(async (command) => {
      if (command === "read_transcript") {
        const read = deferred<TranscriptEvent[]>();
        reads.push(read);
        return read.promise;
      }
      return null;
    });
    const chat = mount();
    const done: number[] = [];
    act(() => {
      void chat().reload().then(() => done.push(1));
      void chat().reload().then(() => done.push(2));
      void chat().reload().then(() => done.push(3));
    });
    expect(reads).toHaveLength(1);
    await act(async () => reads[0].resolve([v1]));
    expect(reads).toHaveLength(2);
    expect(done).toEqual([]);
    await act(async () => reads[1].resolve([v3]));
    expect(done).toEqual([1, 2, 3]);
    expect(chat().events).toEqual([v3]);
  });

  it("沒 id 的舊事件第一次被寫入：只換同一物件那則；畫面重讀過認不出就重讀後端，不批次替換同時同文的則", async () => {
    const legacy = (): TranscriptEvent => ({ ts: "t1", speaker_id: "", speaker_name: "", kind: "narration", text: "同一句" });
    const a = legacy();
    const b = legacy();
    const written: TranscriptEvent = { ...legacy(), id: "new", message_vars: { hp: 2 }, vars_rev: "r2" };
    const reads: ReturnType<typeof deferred<TranscriptEvent[]>>[] = [];
    invokeMock.mockImplementation(async (command) => {
      if (command === "read_transcript") {
        const read = deferred<TranscriptEvent[]>();
        reads.push(read);
        return read.promise;
      }
      return null;
    });
    const chat = mount();
    act(() => chat().hydrate([a, b]));
    act(() => chat().replaceEvent(b, written));
    expect(chat().events[0]).toBe(a);
    expect(chat().events[1]).toBe(written);
    await act(async () => reads[0].resolve([a, written]));

    // 畫面重讀過（新物件），卡片手上的還是舊物件：不猜，兩則都不動，改拿後端結果
    const a2 = legacy();
    const b2 = legacy();
    act(() => chat().hydrate([a2, b2]));
    act(() => chat().replaceEvent(b, written));
    expect(chat().events[0]).toBe(a2);
    expect(chat().events[1]).toBe(b2);
    expect(reads).toHaveLength(2);
    await act(async () => reads[1].resolve([a2, written]));
    expect(chat().events.map((event) => event.id)).toEqual([undefined, "new"]);
  });

  it("舊桌落檔永不回應時換桌：新桌重讀照常發出，舊桌晚到的收尾不讓新桌重讀作廢", async () => {
    const reads: { worldId: unknown; read: ReturnType<typeof deferred<TranscriptEvent[]>> }[] = [];
    const posted = deferred<TranscriptEvent>();
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "read_transcript") {
        const read = deferred<TranscriptEvent[]>();
        reads.push({ worldId: args!.worldId, read });
        return read.promise;
      }
      if (command === "post_opening") return posted.promise;
      return null;
    });
    const chat = mount();
    act(() => void chat().postOpening("開場白", undefined, null, () => false));
    // A 桌在途時重讀：等落檔
    let staleDone = false;
    act(() => void chat().reload().then(() => (staleDone = true)));
    expect(reads).toHaveLength(0);
    act(() => setWorld("w2"));
    await act(async () => {});
    // 舊世代的等待被叫醒、直接結束
    expect(staleDone).toBe(true);
    act(() => void chat().reload());
    expect(reads.map((entry) => entry.worldId)).toEqual(["w2"]);
    const fresh = event({ hp: 9 }, "r9");
    // A 桌的開場白這時才回來：收尾只算舊世代，新桌這趟照套
    await act(async () => posted.resolve({ ts: "t9", speaker_id: "", speaker_name: "", kind: "narration", text: "開場白" }));
    await act(async () => reads[0].read.resolve([fresh]));
    expect(reads).toHaveLength(1);
    expect(chat().events).toEqual([fresh]);
  });
});
