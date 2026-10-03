// @vitest-environment happy-dom
// 覆蓋層的讀訊息推送：快照變動時推、iframe load 時補推，帶目前這支殼的 token，只送進目前的 iframe。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CardInterfaceOverlay } from "./CardInterfaceOverlay";
import { type CardChat } from "./card-chat-shim";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const chatA: CardChat = { currentId: 0, floors: [{ name: "GM", role: "assistant", message: "A" }] };
const chatB: CardChat = {
  currentId: 0,
  floors: [
    { name: "GM", role: "assistant", message: "A" },
    { name: "玩家", role: "user", message: "B" },
  ],
};

describe("CardInterfaceOverlay chat push", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
  });

  async function render(chat: CardChat | null, shellKey: string) {
    if (root === null) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () =>
      root?.render(
        <CardInterfaceOverlay generatingName={null} shellDoc="<html></html>" shellKey={shellKey} chat={chat} onClose={() => {}} />,
      ),
    );
  }

  function spyFrame() {
    const frame = host!.querySelector("iframe")!;
    const postMessage = vi.fn();
    Object.defineProperty(frame, "contentWindow", { configurable: true, value: { postMessage } });
    return { frame, postMessage };
  }

  it("updates before load are pushed, and load pushes the latest snapshot again", async () => {
    await render(chatA, "k1");
    const { frame, postMessage } = spyFrame();
    // 還沒 load 就有新一樓
    await render(chatB, "k1");
    expect(postMessage).toHaveBeenLastCalledWith(
      { source: "table-tavern-host", kind: "chat", token: "k1", chat: chatB, mvu: null },
      "*",
    );
    postMessage.mockClear();
    await act(async () => {
      frame.dispatchEvent(new Event("load"));
    });
    expect(postMessage).toHaveBeenCalledTimes(1);
    expect(postMessage).toHaveBeenLastCalledWith(
      { source: "table-tavern-host", kind: "chat", token: "k1", chat: chatB, mvu: null },
      "*",
    );
  });

  it("a new shell key mounts a new iframe and pushes carry the new token only", async () => {
    await render(chatA, "k1");
    const old = spyFrame();
    await render(chatB, "k2");
    const fresh = spyFrame();
    expect(fresh.frame).not.toBe(old.frame);
    await act(async () => {
      fresh.frame.dispatchEvent(new Event("load"));
    });
    expect(old.postMessage).not.toHaveBeenCalled();
    expect(fresh.postMessage).toHaveBeenLastCalledWith(
      { source: "table-tavern-host", kind: "chat", token: "k2", chat: chatB, mvu: null },
      "*",
    );
  });

  it("no snapshot, no push", async () => {
    await render(null, "k1");
    const { frame, postMessage } = spyFrame();
    await act(async () => {
      frame.dispatchEvent(new Event("load"));
    });
    expect(postMessage).not.toHaveBeenCalled();
  });
});
