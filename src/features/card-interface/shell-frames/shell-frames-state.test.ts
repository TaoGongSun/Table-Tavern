// 殼雙緩衝的狀態轉移與宿主收訊規則。
import { describe, expect, it } from "vitest";
import {
  acceptsInput,
  acceptsStorage,
  createShellRegistry,
  EMPTY_FRAMES,
  framesReducer,
  type FrameSlot,
  type FramesState,
} from "./shell-frames-state";

const slot = (key: string, identity = "桌A"): FrameSlot => ({ key, doc: `<p>${key}</p>`, identity });
const run = (state: FramesState, ...slots: (FrameSlot | null)[]) =>
  slots.reduce((current, next, index) => framesReducer(current, { type: "shell", slot: next, now: index * 100 }), state);

describe("framesReducer", () => {
  it("首開直接當 front；同桌換殼進 pending 並記鎖定起點；換 pending 不重算起點", () => {
    const opened = run(EMPTY_FRAMES, slot("a"));
    expect(opened).toEqual({ front: slot("a"), pending: null, lockedAt: null });
    const waiting = run(opened, slot("b"));
    expect(waiting.pending?.key).toBe("b");
    const again = framesReducer(waiting, { type: "shell", slot: slot("c"), now: 999 });
    expect(again).toMatchObject({ front: { key: "a" }, pending: { key: "c" }, lockedAt: waiting.lockedAt });
  });

  it("ready 只認目前的 pending；timeout 翻到 pending；沒有 pending 時不動", () => {
    const waiting = run(EMPTY_FRAMES, slot("a"), slot("b"));
    expect(framesReducer(waiting, { type: "ready", key: "a" })).toBe(waiting);
    expect(framesReducer(waiting, { type: "ready", key: "b" })).toEqual({ front: slot("b"), pending: null, lockedAt: null });
    expect(framesReducer(waiting, { type: "timeout" })).toEqual({ front: slot("b"), pending: null, lockedAt: null });
    const settled = run(EMPTY_FRAMES, slot("a"));
    expect(framesReducer(settled, { type: "timeout" })).toBe(settled);
  });

  it("顯示對象不同硬切；回到 front 丟掉 pending；殼沒了全卸", () => {
    const waiting = run(EMPTY_FRAMES, slot("a"), slot("b"));
    expect(framesReducer(waiting, { type: "shell", slot: slot("x", "桌B"), now: 0 })).toEqual({ front: slot("x", "桌B"), pending: null, lockedAt: null });
    expect(framesReducer(waiting, { type: "shell", slot: slot("a"), now: 0 })).toEqual({ front: slot("a"), pending: null, lockedAt: null });
    expect(framesReducer(waiting, { type: "shell", slot: null, now: 0 })).toEqual(EMPTY_FRAMES);
    expect(framesReducer(waiting, { type: "shell", slot: slot("b"), now: 0 })).toBe(waiting);
  });
});

describe("宿主收訊規則", () => {
  const frame = () => {
    const win = {} as Window;
    return { win, frame: { contentWindow: win } as unknown as HTMLIFrameElement };
  };

  it("input 只收 front、不在鎖定期、source 與 token 都對", () => {
    const front = frame();
    const registry = { ...createShellRegistry(), front: { key: "a", frame: front.frame } };
    expect(acceptsInput(registry, front.win, "a")).toBe(true);
    expect(acceptsInput(registry, front.win, "b")).toBe(false);
    expect(acceptsInput(registry, {}, "a")).toBe(false);
    expect(acceptsInput({ ...registry, locked: true }, front.win, "a")).toBe(false);
  });

  it("storage：有 pending 只收 pending 的，沒有才收 front 的", () => {
    const front = frame();
    const pending = frame();
    const registry = { front: { key: "a", frame: front.frame }, pending: { key: "b", frame: pending.frame }, locked: true };
    expect(acceptsStorage(registry, front.win, "a")).toBe(false);
    expect(acceptsStorage(registry, pending.win, "b")).toBe(true);
    expect(acceptsStorage({ ...registry, pending: null, locked: false }, front.win, "a")).toBe(true);
    expect(acceptsStorage(createShellRegistry(), front.win, "a")).toBe(false);
  });
});
