// @vitest-environment happy-dom
// 卡片介面 controller：面板開關跟著殼走；讀訊息快照、doc 與 key 只在該變的時候變。

import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { type CardInterface } from "./interface-card";
import { type CardInterfaceController, useCardInterfaceController } from "./useCardInterfaceController";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";

const backend = vi.hoisted(() => ({
  shell: null as string | null,
  mode: null as string | null,
}));

// 不用 $1 的殼：每一樓產出的殼字串都一樣，跟讀本樓的狀態欄殼同型
const fixedCard: CardInterface = {
  character_id: "c1",
  character_name: "卡",
  scripts: [
    {
      name: "殼",
      find_regex: "/<UI>([\\s\\S]*?)<\\/UI>/s",
      replace_string: "```html\n<!DOCTYPE html><head></head><body>固定殼</body>\n```",
      trim_strings: [],
      min_depth: null,
      max_depth: null,
    },
  ],
  unsupported: null,
  opening: "<UI>開場白</UI>",
};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "card_interfaces") return [fixedCard];
    if (command === "refactor_interface_shell") return backend.shell;
    if (command === "refactor_table_mode") return backend.mode;
    return null;
  }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const gm = (raw: string): TranscriptEvent => ({
  ts: "t",
  speaker_id: "gm",
  speaker_name: "GM",
  kind: "narration",
  text: "正文",
  raw,
});
const player = (text: string): TranscriptEvent => ({ ts: "t", speaker_id: "", speaker_name: "玩家", kind: "player", text });
const submitText = async () => {};

describe("useCardInterfaceController", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let controller: CardInterfaceController | null = null;
  let props = { worldId: "w1", events: [] as TranscriptEvent[], tableTree: {} as Record<string, unknown> };

  function Probe() {
    controller = useCardInterfaceController({
      worldId: props.worldId,
      events: props.events,
      tableTree: props.tableTree as never,
      submitText,
    });
    return null;
  }

  async function settle() {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }

  async function render(next: Partial<typeof props> = {}) {
    props = { ...props, ...next };
    if (root === null) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () => root?.render(createElement(Probe)));
    await settle();
  }

  beforeEach(() => {
    backend.mode = null;
    backend.shell = null;
    props = { worldId: "w1", events: [gm("<UI>第一樓</UI>")], tableTree: {} };
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => void store.set(key, String(value)),
      removeItem: (key: string) => void store.delete(key),
      clear: () => store.clear(),
    });
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
    controller = null;
    vi.unstubAllGlobals();
  });

  it("closes the panel when the shell goes away and stays closed when it comes back", async () => {
    backend.mode = "interface";
    backend.shell = "<UI>{{世界.時間}}</UI>";
    await render({ events: [{ ...gm("沒有標籤"), raw: undefined }], tableTree: { 世界: { 時間: "黃昏" } } });
    expect(controller!.shellReady).toBe(true);

    await act(async () => controller!.open());
    expect(controller!.uiOpen).toBe(true);

    backend.shell = null;
    await act(async () => {
      await controller!.refreshShell("w1");
    });
    expect(controller!.shellReady).toBe(false);
    expect(controller!.uiOpen).toBe(false);

    backend.shell = "<UI>{{世界.時間}}</UI>";
    await act(async () => {
      await controller!.refreshShell("w1");
    });
    expect(controller!.shellReady).toBe(true);
    expect(controller!.uiOpen).toBe(false);
  });

  it("same shell, same floor, new text: key changes so the iframe remounts", async () => {
    await render();
    const before = controller!.shellKey;
    await render({ events: [gm("<UI>第一樓改過</UI>")] });
    expect(controller!.chat?.currentId).toBe(0);
    expect(controller!.shellKey).not.toBe(before);
  });

  it("switching tables with the same shell and text still changes the key", async () => {
    await render();
    const before = controller!.shellKey;
    await render({ worldId: "w2" });
    expect(controller!.shellKey).not.toBe(before);
  });

  it("a new player floor updates the snapshot without remounting", async () => {
    await render();
    await act(async () => controller!.open());
    const key = controller!.shellKey;
    const doc = controller!.shellDoc;
    await render({ events: [gm("<UI>第一樓</UI>"), player("新的一句")] });
    expect(controller!.shellKey).toBe(key);
    expect(controller!.shellDoc).toBe(doc);
    expect(controller!.chat?.floors.map((floor) => floor.message)).toEqual(["<UI>第一樓</UI>", "新的一句"]);
  });

  it("unrelated renders keep doc and key", async () => {
    await render();
    await act(async () => controller!.open());
    const key = controller!.shellKey;
    const doc = controller!.shellDoc;
    await render({ tableTree: { 無關: { 欄位: "1" } } });
    expect(controller!.shellKey).toBe(key);
    expect(controller!.shellDoc).toBe(doc);
  });

  it("reopening after other floors changed embeds the latest snapshot before the card runs", async () => {
    await render();
    await act(async () => controller!.open());
    expect(controller!.shellDoc).not.toContain("面板關著時新增");
    await act(async () => controller!.close());
    expect(controller!.shellDoc).toBeNull();
    await render({ events: [gm("<UI>第一樓</UI>"), player("面板關著時新增")] });
    await act(async () => controller!.open());
    expect(controller!.shellDoc).toContain("面板關著時新增");
  });

  it("popping the latest floor moves the current floor back", async () => {
    await render({ events: [gm("<UI>第一樓</UI>"), player("x"), gm("<UI>第三樓</UI>")] });
    expect(controller!.chat?.currentId).toBe(2);
    const key = controller!.shellKey;
    await render({ events: [gm("<UI>第一樓</UI>"), player("x")] });
    expect(controller!.chat?.currentId).toBe(0);
    expect(controller!.shellKey).not.toBe(key);
  });

  it("skeleton tables keep the synthesized text on the current floor after a push", async () => {
    backend.mode = "interface";
    backend.shell = "<UI>骨架 {{世界.時間}}</UI>";
    const noTag = { ...gm("沒有標籤"), raw: undefined };
    await render({ events: [noTag], tableTree: { 世界: { 時間: "黃昏" } } });
    expect(controller!.chat?.floors[0].message).toBe("<UI>骨架 黃昏</UI>");
    await render({ events: [noTag, player("新的一句")] });
    expect(controller!.chat?.currentId).toBe(0);
    expect(controller!.chat?.floors[0].message).toBe("<UI>骨架 黃昏</UI>");
  });
});
