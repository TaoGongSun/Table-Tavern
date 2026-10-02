// @vitest-environment happy-dom
// 面板開著時殼沒了（套用沒產殼的重構）：面板狀態一起收掉，殼回來（undo）也不會自己跳開。

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

const card: CardInterface = {
  character_id: "c1",
  character_name: "卡",
  scripts: [
    {
      name: "殼",
      find_regex: "/<UI>([\\s\\S]*?)<\\/UI>/s",
      replace_string: "```html\n<!DOCTYPE html><body>$1</body>\n```",
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
    if (command === "card_interfaces") return [card];
    if (command === "refactor_interface_shell") return backend.shell;
    if (command === "refactor_table_mode") return backend.mode;
    return null;
  }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const events: TranscriptEvent[] = [
  { ts: "t", speaker_id: "gm", speaker_name: "GM", kind: "narration", text: "正文" },
];
const tree = { 世界: { 時間: "黃昏" } };
const submitText = async () => {};

describe("useCardInterfaceController", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let controller: CardInterfaceController | null = null;

  function Probe() {
    controller = useCardInterfaceController({ worldId: "w1", events, tableTree: tree, submitText });
    return null;
  }

  async function settle() {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }

  beforeEach(() => {
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
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root?.render(createElement(Probe)));
    await settle();
    expect(controller!.shellReady).toBe(true);

    await act(async () => controller!.open());
    expect(controller!.uiOpen).toBe(true);

    // 套用沒產殼的重構
    backend.shell = null;
    await act(async () => {
      await controller!.refreshShell("w1");
    });
    expect(controller!.shellReady).toBe(false);
    expect(controller!.uiOpen).toBe(false);

    // undo 把殼寫回：按鈕回來，面板不自己跳開
    backend.shell = "<UI>{{世界.時間}}</UI>";
    await act(async () => {
      await controller!.refreshShell("w1");
    });
    expect(controller!.shellReady).toBe(true);
    expect(controller!.uiOpen).toBe(false);
  });
});
