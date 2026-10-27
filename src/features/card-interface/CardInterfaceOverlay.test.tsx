// @vitest-environment happy-dom
// 覆蓋層：殼雙緩衝（背面載、ready 翻面、逾時強制翻、硬切、鎖定）與讀訊息推送（帶這份殼的 token，只送進它的 iframe）。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { buildOverlayComposer, CardInterfaceOverlay, type OverlayComposer } from "./CardInterfaceOverlay";
import { type CardChat } from "./card-chat-shim";
import { createShellRegistry, type ShellRegistry } from "./shell-frames/shell-frames-state";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const composer = (over: Partial<OverlayComposer> = {}): OverlayComposer => ({
  input: "",
  onInputChange: () => {},
  onSubmit: (event) => event.preventDefault(),
  placeholder: "對 GM 說",
  inputDisabled: false,
  sendDisabled: false,
  canStop: false,
  onStop: () => {},
  errorNote: null,
  notice: null,
  ...over,
});

const chatA: CardChat = { currentId: 0, floors: [{ name: "GM", role: "assistant", message: "A" }] };
const chatB: CardChat = {
  currentId: 0,
  floors: [
    { name: "GM", role: "assistant", message: "A" },
    { name: "玩家", role: "user", message: "B" },
  ],
};

describe("CardInterfaceOverlay 殼雙緩衝與讀訊息推送", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let registry: ShellRegistry = createShellRegistry();

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
    registry = createShellRegistry();
    vi.useRealTimers();
  });

  async function render(chat: CardChat | null, shellKey: string, identity = "桌A") {
    if (root === null) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () =>
      root?.render(
        <CardInterfaceOverlay
          generatingName={null}
          shellDoc={`<html>${shellKey}</html>`}
          shellKey={shellKey}
          shellIdentity={identity}
          shellRegistry={registry}
          chat={chat}
          onAdvanceScene={() => {}}
          advanceDisabled={false}
          onClose={() => {}}
          composer={composer()}
        />,
      ),
    );
  }

  const frames = () => [...host!.querySelectorAll<HTMLIFrameElement>("iframe")];
  const frameOf = (key: string) => host!.querySelector<HTMLIFrameElement>(`iframe[data-shell-key="${key}"]`)!;

  function spyFrame(key: string) {
    const frame = frameOf(key);
    const postMessage = vi.fn();
    const view = { postMessage };
    Object.defineProperty(frame, "contentWindow", { configurable: true, value: view });
    return { frame, postMessage, view };
  }

  async function ready(source: unknown, token: string) {
    await act(async () => {
      window.dispatchEvent(
        new MessageEvent("message", {
          data: { source: "table-tavern-card", kind: "ready", token, reason: "raf", t: { dcl: 1, raf: 2, sent: 2 } },
          source: source as Window,
        }),
      );
    });
  }

  it("同一份殼：快照變動就推；殼回報 ready 再推一次最新的", async () => {
    await render(chatA, "k1");
    const { postMessage, view } = spyFrame("k1");
    await render(chatB, "k1");
    expect(postMessage).toHaveBeenLastCalledWith(
      { source: "table-tavern-host", kind: "chat", token: "k1", chat: chatB, mvu: null },
      "*",
    );
    postMessage.mockClear();
    await ready(view, "k1");
    expect(postMessage).toHaveBeenCalledTimes(1);
    expect(postMessage).toHaveBeenLastCalledWith(
      { source: "table-tavern-host", kind: "chat", token: "k1", chat: chatB, mvu: null },
      "*",
    );
  });

  it("首開直接繪：只有一支、是 front、沒有擋板", async () => {
    await render(chatA, "k1");
    expect(frames()).toHaveLength(1);
    expect(frameOf("k1").dataset.shellRole).toBe("front");
    expect(host!.querySelector("[data-shell-lock]")).toBeNull();
    expect(registry.locked).toBe(false);
  });

  it("同桌換殼：新殼掛在背面、舊殼留著、鎖定；新殼 ready 才翻面，翻面不換元素、舊殼卸掉", async () => {
    await render(chatA, "k1");
    const old = spyFrame("k1");
    await render(chatB, "k2");
    expect(frames().map((frame) => frame.dataset.shellRole)).toEqual(["front", "pending"]);
    const fresh = spyFrame("k2");
    expect(host!.querySelector("[data-shell-lock]")).not.toBeNull();
    expect(frames().every((frame) => frame.inert && frame.tabIndex === -1)).toBe(true);
    expect(registry).toMatchObject({ locked: true, front: { key: "k1" }, pending: { key: "k2" } });
    // 舊殼的 ready、token 不對的 ready 都不翻面
    await ready(old.view, "k1");
    await ready(fresh.view, "k1");
    expect(frames()).toHaveLength(2);
    await ready(fresh.view, "k2");
    expect(frames()).toEqual([fresh.frame]);
    expect(fresh.frame.dataset.shellRole).toBe("front");
    expect(fresh.frame.inert).toBe(false);
    expect(host!.querySelector("[data-shell-lock]")).toBeNull();
    expect(registry).toMatchObject({ locked: false, front: { key: "k2" }, pending: null });
    expect(old.postMessage).not.toHaveBeenCalled();
    expect(fresh.postMessage).toHaveBeenLastCalledWith(
      { source: "table-tavern-host", kind: "chat", token: "k2", chat: chatB, mvu: null },
      "*",
    );
  });

  it("等待期又換殼：丟掉舊的背面那支、front 不動；逾時從鎖定起點算，到時強制翻到最新的", async () => {
    vi.useFakeTimers();
    await render(chatA, "k1");
    await render(chatA, "k2");
    await act(async () => {
      vi.advanceTimersByTime(1000);
    });
    await render(chatA, "k3");
    expect(frames().map((frame) => frame.dataset.shellKey)).toEqual(["k1", "k3"]);
    await act(async () => {
      vi.advanceTimersByTime(499);
    });
    expect(frames()).toHaveLength(2);
    await act(async () => {
      vi.advanceTimersByTime(10);
    });
    expect(frames().map((frame) => [frame.dataset.shellKey, frame.dataset.shellRole])).toEqual([["k3", "front"]]);
  });

  it("換桌（顯示對象不同）硬切：舊的立刻卸、新的直接當 front", async () => {
    await render(chatA, "k1", "桌A");
    await render(chatA, "k2", "桌B");
    expect(frames().map((frame) => [frame.dataset.shellKey, frame.dataset.shellRole])).toEqual([["k2", "front"]]);
    expect(registry.locked).toBe(false);
  });

  it("等待期回到正在顯示的那份：丟掉背面、解鎖", async () => {
    await render(chatA, "k1");
    await render(chatA, "k2");
    await render(chatA, "k1");
    expect(frames().map((frame) => frame.dataset.shellKey)).toEqual(["k1"]);
    expect(registry.locked).toBe(false);
  });

  it("鎖定時焦點在舊殼裡就移到擋板", async () => {
    await render(chatA, "k1");
    frameOf("k1").focus();
    expect(document.activeElement).toBe(frameOf("k1"));
    await render(chatA, "k2");
    expect(document.activeElement).toBe(host!.querySelector("[data-shell-lock]"));
  });

  it("no snapshot, no push", async () => {
    await render(null, "k1");
    const { postMessage, view } = spyFrame("k1");
    await ready(view, "k1");
    expect(postMessage).not.toHaveBeenCalled();
  });

  it("換幕鈕照 advanceDisabled 停用，可用時點一下就呼叫 onAdvanceScene", async () => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    const onAdvanceScene = vi.fn();
    const mount = (advanceDisabled: boolean) =>
      act(async () =>
        root?.render(
          <CardInterfaceOverlay
            generatingName={null}
            shellDoc="<html></html>"
            shellKey="k1"
            shellIdentity="桌A"
            shellRegistry={createShellRegistry()}
            chat={chatA}
            onAdvanceScene={onAdvanceScene}
            advanceDisabled={advanceDisabled}
            onClose={() => {}}
            composer={composer()}
          />,
        ),
      );
    await mount(true);
    const button = [...host.querySelectorAll("button")].find((b) => b.textContent === "換幕")!;
    expect(button.disabled).toBe(true);
    await mount(false);
    await act(async () => button.click());
    expect(onAdvanceScene).toHaveBeenCalledTimes(1);
  });
});

describe("CardInterfaceOverlay 分頁與輸入列", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
  });

  async function mount(props: Partial<Parameters<typeof CardInterfaceOverlay>[0]>) {
    if (root === null) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () =>
      root?.render(
        <CardInterfaceOverlay
          generatingName={null}
          shellDoc="<html></html>"
          shellKey="k1"
          shellIdentity="桌A"
          shellRegistry={createShellRegistry()}
          chat={chatA}
          onAdvanceScene={() => {}}
          advanceDisabled={false}
          onClose={() => {}}
          composer={composer()}
          {...props}
        />,
      ),
    );
  }
  const tabButtons = () => [...host!.querySelectorAll<HTMLButtonElement>(".card-interface-tab")];

  it("兩張以上才出分頁；目前那張 aria-pressed，點了回報卡 id；世界書卡沒名字用後備標籤、title 是全名", async () => {
    await mount({ tabs: [{ key: "world", name: "西幻" }] });
    expect(tabButtons()).toHaveLength(0);
    const onPickCard = vi.fn();
    await mount({
      tabs: [
        { key: "world", name: "" },
        { key: "c9", name: "很長很長很長很長很長的角色名字" },
      ],
      activeCard: "world",
      onPickCard,
    });
    const [world, other] = tabButtons();
    expect(world.textContent).toBe("世界");
    expect(world.getAttribute("aria-pressed")).toBe("true");
    expect(other.getAttribute("aria-pressed")).toBe("false");
    expect(other.title).toBe("很長很長很長很長很長的角色名字");
    // 分頁排在換幕與關閉鈕左邊
    const toolbar = [...host!.querySelector(".card-interface-toolbar")!.children];
    expect(toolbar[0].classList.contains("card-interface-tabs")).toBe(true);
    await act(async () => other.click());
    expect(onPickCard).toHaveBeenCalledWith("c9");
  });

  it("輸入列：打字回報、送出走 onSubmit、停用條件照傳入、錯誤看得到、生成中換停止鈕", async () => {
    const onInputChange = vi.fn();
    const onSubmit = vi.fn((event: { preventDefault: () => void }) => event.preventDefault());
    await mount({ composer: composer({ input: "我拔劍", onInputChange, onSubmit, errorNote: <p>出錯了</p> }) });
    const input = host!.querySelector<HTMLInputElement>(".card-interface-composer input")!;
    expect(input.value).toBe("我拔劍");
    expect(input.placeholder).toBe("對 GM 說");
    expect(host!.querySelector(".card-interface-composer .composer-error")?.textContent).toBe("出錯了");
    const submit = host!.querySelector<HTMLButtonElement>(".card-interface-composer button[type=submit]")!;
    await act(async () => submit.click());
    expect(onSubmit).toHaveBeenCalledTimes(1);

    await mount({ composer: composer({ inputDisabled: true, sendDisabled: true }) });
    expect(host!.querySelector<HTMLInputElement>(".card-interface-composer input")!.disabled).toBe(true);
    expect(host!.querySelector<HTMLButtonElement>(".card-interface-composer button[type=submit]")!.disabled).toBe(true);

    await mount({ composer: composer({ notice: "容量滿了" }) });
    expect(host!.querySelector(".card-interface-composer .scene-capacity-hint")?.textContent).toBe("容量滿了");

    const onStop = vi.fn();
    await mount({ composer: composer({ canStop: true, onStop }) });
    expect(host!.querySelector(".card-interface-composer button[type=submit]")).toBeNull();
    const stop = host!.querySelector<HTMLButtonElement>(".card-interface-composer button")!;
    await act(async () => stop.click());
    expect(onStop).toHaveBeenCalledTimes(1);
  });
});

describe("buildOverlayComposer：與聊天輸入框同一套條件", () => {
  const base = {
    locked: false,
    speaker: "gm",
    castEmpty: false,
    busy: false,
    sceneSummary: null,
    targetName: "GM",
    draft: "我拔劍",
    setDraft: () => {},
    canStop: false,
    onStop: () => {},
    errorNote: null,
  };
  const submitEvent = () => ({ preventDefault: vi.fn() }) as unknown as Parameters<OverlayComposer["onSubmit"]>[0];

  it("可送：送出交給 send（同一份草稿），不另外清草稿", () => {
    const send = vi.fn(async () => {});
    const setDraft = vi.fn();
    const composer = buildOverlayComposer({ ...base, send, setDraft });
    expect(composer.sendDisabled).toBe(false);
    expect(composer.placeholder).toContain("GM");
    composer.onSubmit(submitEvent());
    expect(send).toHaveBeenCalledTimes(1);
    expect(setDraft).not.toHaveBeenCalled();
  });

  it("忙碌、沒對象又沒角色、去空白後為空：停用", () => {
    const send = vi.fn(async () => {});
    expect(buildOverlayComposer({ ...base, send, busy: true })).toMatchObject({ inputDisabled: true, sendDisabled: true });
    expect(buildOverlayComposer({ ...base, send, speaker: "", castEmpty: true })).toMatchObject({ inputDisabled: true, sendDisabled: true });
    expect(buildOverlayComposer({ ...base, send, draft: "  " })).toMatchObject({ inputDisabled: false, sendDisabled: true });
  });

  it("鎖住或換幕容量滿：送出被擋下、不呼叫 send，草稿原封不動", () => {
    const send = vi.fn(async () => {});
    const setDraft = vi.fn();
    const locked = buildOverlayComposer({ ...base, send, setDraft, locked: true });
    const event = submitEvent();
    locked.onSubmit(event);
    expect(event.preventDefault).toHaveBeenCalled();
    const full = buildOverlayComposer({
      ...base,
      send,
      setDraft,
      sceneSummary: { lockable: true, used: 100, cap: 100, gReply: 0, unit: "chars", ratio: 1 } as never,
    });
    expect(full.sendDisabled).toBe(true);
    expect(full.notice).toBe("預估再送一句會超過一次整理的容量，先換幕再繼續吧。");
    full.onSubmit(submitEvent());
    expect(send).not.toHaveBeenCalled();
    expect(setDraft).not.toHaveBeenCalled();
  });
});
