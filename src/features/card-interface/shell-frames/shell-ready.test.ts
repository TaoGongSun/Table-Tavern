// @vitest-environment happy-dom
// 殼內 ready 墊片：DOMContentLoaded 後兩次 rAF 送 raf；可見但 rAF 不跑 300 ms 送 timer；隱藏時等轉可見；只送一次；ping 回 pong。
import { afterEach, describe, expect, it, vi } from "vitest";
import { buildReadySignalSource, parseReady } from "./shell-ready";

type Frame = () => void;

function install(token = "T1") {
  const frames: Frame[] = [];
  const posts: Record<string, unknown>[] = [];
  vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
    frames.push(() => callback(performance.now()));
    return frames.length;
  });
  vi.spyOn(window, "postMessage").mockImplementation((message: unknown) => {
    posts.push(message as Record<string, unknown>);
  });
  new Function(buildReadySignalSource(JSON.stringify(token)))();
  const runFrames = () => frames.splice(0).forEach((frame) => frame());
  return { posts, runFrames };
}

function setVisibility(state: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", { configurable: true, get: () => state });
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
  setVisibility("visible");
});

const readies = (posts: Record<string, unknown>[]) => posts.filter((post) => post.kind === "ready");

describe("buildReadySignalSource", () => {
  it("DOMContentLoaded 後第二次 rAF 送 raf，帶 dcl／raf／sent，只送一次", () => {
    vi.useFakeTimers();
    const { posts, runFrames } = install();
    document.dispatchEvent(new Event("DOMContentLoaded"));
    runFrames();
    expect(readies(posts)).toHaveLength(0);
    runFrames();
    vi.advanceTimersByTime(1000);
    const [ready] = readies(posts);
    expect(readies(posts)).toHaveLength(1);
    expect(parseReady(ready)).toMatchObject({ token: "T1", reason: "raf" });
    const t = parseReady(ready)!.t;
    expect(t.dcl).not.toBeNull();
    expect(t.raf).not.toBeNull();
  });

  it("可見但 rAF 300 ms 內一次都沒跑：送 timer，raf 留空", () => {
    vi.useFakeTimers();
    const { posts } = install();
    document.dispatchEvent(new Event("DOMContentLoaded"));
    vi.advanceTimersByTime(299);
    expect(readies(posts)).toHaveLength(0);
    vi.advanceTimersByTime(1);
    expect(parseReady(readies(posts)[0])).toMatchObject({ reason: "timer", t: { raf: null } });
  });

  it("隱藏時不計時，轉可見後才排 rAF", () => {
    vi.useFakeTimers();
    setVisibility("hidden");
    const { posts, runFrames } = install();
    document.dispatchEvent(new Event("DOMContentLoaded"));
    vi.advanceTimersByTime(5000);
    runFrames();
    expect(readies(posts)).toHaveLength(0);
    setVisibility("visible");
    document.dispatchEvent(new Event("visibilitychange"));
    runFrames();
    runFrames();
    expect(parseReady(readies(posts)[0])).toMatchObject({ reason: "raf" });
  });

  it("宿主 ping（token 相符）回 pong；token 不符不回", () => {
    const { posts } = install("T9");
    window.dispatchEvent(new MessageEvent("message", { data: { source: "table-tavern-host", kind: "ping", token: "XX", t0: 1 }, source: window }));
    window.dispatchEvent(new MessageEvent("message", { data: { source: "table-tavern-host", kind: "ping", token: "T9", t0: 1 }, source: window }));
    const pongs = posts.filter((post) => post.kind === "pong");
    expect(pongs).toHaveLength(1);
    expect(pongs[0]).toMatchObject({ source: "table-tavern-card", token: "T9", t0: 1 });
  });
});

describe("parseReady", () => {
  it("形狀不對回 null", () => {
    const ok = { source: "table-tavern-card", kind: "ready", token: "a", reason: "raf", t: { dcl: 1, raf: 2, sent: 3 } };
    expect(parseReady(ok)).not.toBeNull();
    expect(parseReady({ ...ok, reason: "load" })).toBeNull();
    expect(parseReady({ ...ok, token: 1 })).toBeNull();
    expect(parseReady({ ...ok, t: { dcl: "x", raf: 2, sent: 3 } })).toBeNull();
    expect(parseReady({ ...ok, source: "other" })).toBeNull();
    expect(parseReady(null)).toBeNull();
  });
});
