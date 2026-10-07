import { describe, expect, it, vi } from "vitest";
import { splitFrontends } from "./frontend-blocks";
import { createFrontendHost, MAX_FRAME_HEIGHT, type FrontendHostDeps } from "./frontend-host";

describe("frontend blocks (TH renderer: a code block with html>, <head> or <body is a frontend)", () => {
  it("cuts frontends out of the message and keeps the text around them in order", () => {
    const text = "前言\n```html\n<!DOCTYPE html><html><body>介面</body></html>\n```\n後記";
    expect(splitFrontends(text)).toEqual([
      { kind: "text", text: "前言\n" },
      { kind: "frontend", html: "<!DOCTYPE html><html><body>介面</body></html>\n" },
      { kind: "text", text: "\n後記" },
    ]);
  });

  it("any fence label counts when the content looks like a page; other code blocks stay text", () => {
    const text = "```text\n<body>頁</body>\n```\n```js\nconsole.log(1)\n```";
    expect(splitFrontends(text)).toEqual([
      { kind: "frontend", html: "<body>頁</body>\n" },
      { kind: "text", text: "\n```js\nconsole.log(1)\n```" },
    ]);
  });

  it("a message without frontends is one text segment; an unterminated fence is not a frontend", () => {
    expect(splitFrontends("只有字")).toEqual([{ kind: "text", text: "只有字" }]);
    expect(splitFrontends("```html\n<html>沒收尾")).toEqual([{ kind: "text", text: "```html\n<html>沒收尾" }]);
  });
});

function fakeWindow() {
  return { postMessage: vi.fn() } as unknown as Window & { postMessage: ReturnType<typeof vi.fn> };
}

function setup() {
  const deps: FrontendHostDeps = {
    onInput: vi.fn(),
    onStorage: vi.fn(),
    mvu: { write: vi.fn(), evaluate: vi.fn() },
  };
  const host = createFrontendHost({ current: deps });
  const frame = fakeWindow();
  const onHeight = vi.fn();
  const unregister = host.register(frame, { token: "T1", floor: 2, onHeight });
  // 預設：從掛著的那支 iframe 送來、不透明來源、帶對的 token
  const send = (data: unknown, source: unknown = frame, origin = "null") =>
    host.handle({ source, origin, data } as unknown as MessageEvent);
  const card = (fields: Record<string, unknown>) => ({ source: "table-tavern-card", token: "T1", ...fields });
  return { deps, host, frame, onHeight, unregister, send, card };
}

describe("frontend host: only the mounted frames, the right token and the right shape get through", () => {
  it("routes input, storage and height from a mounted frame", () => {
    const { deps, onHeight, send, card } = setup();
    send(card({ kind: "input", text: "推開門" }));
    send(card({ kind: "storage", entries: { theme: "dark", bad: 3 } }));
    send(card({ kind: "height", height: 321.4 }));
    expect(deps.onInput).toHaveBeenCalledWith("推開門");
    // 非字串值照桌面版 sanitize 丟掉
    expect(deps.onStorage).toHaveBeenCalledWith({ theme: "dark" });
    expect(onHeight).toHaveBeenCalledWith(322);
  });

  it("every kind needs the document token: right frame with a wrong or missing token is dropped", () => {
    const { deps, onHeight, send, card } = setup();
    for (const token of ["OLD", undefined]) {
      send(card({ kind: "input", text: "被導走的頁", token }));
      send(card({ kind: "storage", entries: { a: "b" }, token }));
      send(card({ kind: "height", height: 10, token }));
      send(card({ kind: "mvu-write", token }));
      send(card({ kind: "mvu-eval", token }));
    }
    expect(deps.onInput).not.toHaveBeenCalled();
    expect(deps.onStorage).not.toHaveBeenCalled();
    expect(onHeight).not.toHaveBeenCalled();
    expect(deps.mvu!.write).not.toHaveBeenCalled();
    expect(deps.mvu!.evaluate).not.toHaveBeenCalled();
  });

  it("only opaque-origin messages count: a real origin from the same window is dropped", () => {
    const { deps, frame, send, card } = setup();
    send(card({ kind: "input", text: "同站頁" }), frame, "http://127.0.0.1:4317");
    send(card({ kind: "input", text: "外站頁" }), frame, "https://evil.example");
    expect(deps.onInput).not.toHaveBeenCalled();
  });

  it("a forged message from another window or an old frame after unmount is dropped", () => {
    const { deps, send, unregister, card } = setup();
    send(card({ kind: "input", text: "偽造" }), fakeWindow());
    send(card({ kind: "input", text: "沒有來源" }), null);
    unregister();
    send(card({ kind: "input", text: "卸載後" }));
    expect(deps.onInput).not.toHaveBeenCalled();
  });

  it("bad shapes are dropped: non-string input, oversized or non-object storage, non-finite height", () => {
    const { deps, onHeight, send, card } = setup();
    send(card({ kind: "input", text: 1 }));
    send(card({ kind: "storage", entries: ["x"] }));
    // 上限量 UTF-8 位元組：兩萬五千個「中」約 75 KB，超過 64 KiB
    send(card({ kind: "storage", entries: { big: "中".repeat(25_000) } }));
    send(card({ kind: "height", height: Number.NaN }));
    send({ source: "other", token: "T1", kind: "input", text: "別的來源標記" });
    send("字串");
    expect(deps.onInput).not.toHaveBeenCalled();
    expect(deps.onStorage).not.toHaveBeenCalled();
    expect(onHeight).not.toHaveBeenCalled();
  });

  it("height is clamped and MVU messages reach the channel with a reply bound to the same frame and token", () => {
    const { deps, frame, onHeight, send, card } = setup();
    send(card({ kind: "height", height: 10 ** 9 }));
    expect(onHeight).toHaveBeenCalledWith(MAX_FRAME_HEIGHT);
    send(card({ kind: "mvu-eval", requestId: "r1" }));
    const [record, data, reply] = vi.mocked(deps.mvu!.evaluate).mock.calls[0];
    expect(record.floor).toBe(2);
    expect(data.requestId).toBe("r1");
    reply({ kind: "mvu-eval-result", requestId: "r1", ok: true, value: 1 });
    expect(frame.postMessage).toHaveBeenCalledWith(
      { source: "table-tavern-host", token: "T1", kind: "mvu-eval-result", requestId: "r1", ok: true, value: 1 },
      { targetOrigin: "*" },
    );
  });
});
