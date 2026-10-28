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
    expect(opened).toEqual({ front: slot("a"), pending: null, lockedAt: null, frontHealthy: false });
    const waiting = run(opened, slot("b"));
    expect(waiting.pending?.key).toBe("b");
    const again = framesReducer(waiting, { type: "shell", slot: slot("c"), now: 999 });
    expect(again).toMatchObject({ front: { key: "a" }, pending: { key: "c" }, lockedAt: waiting.lockedAt });
  });

  const ok = (key: string) => ({ type: "ready" as const, key, failed: false });
  const failed = (key: string) => ({ type: "ready" as const, key, failed: true });
  /** front a 回報過正常 ready、b 在背面等 */
  const healthyWaiting = () => run(framesReducer(run(EMPTY_FRAMES, slot("a")), ok("a")), slot("b"));

  it("ready 只認目前的 pending（front 的 ready 只記載好了沒）；沒有 pending 時逾時回同一 state", () => {
    const waiting = run(EMPTY_FRAMES, slot("a"), slot("b"));
    expect(framesReducer(waiting, ok("a"))).toEqual({ ...waiting, frontHealthy: true });
    expect(framesReducer(waiting, ok("b"))).toEqual({ front: slot("b"), pending: null, lockedAt: null, frontHealthy: true });
    expect(framesReducer(waiting, ok("zz"))).toBe(waiting);
    const settled = run(EMPTY_FRAMES, slot("a"));
    expect(framesReducer(settled, { type: "timeout" })).toBe(settled);
  });

  it("逾時：front 載好過就留著、解鎖、pending 留在背面；晚到的 ready 照常翻面", () => {
    const waiting = healthyWaiting();
    expect(waiting.lockedAt).not.toBeNull();
    const kept = framesReducer(waiting, { type: "timeout" });
    expect(kept).toEqual({ front: slot("a"), pending: slot("b"), lockedAt: null, frontHealthy: true });
    expect(framesReducer(kept, { type: "timeout" })).toEqual(kept);
    expect(framesReducer(kept, ok("b"))).toEqual({ front: slot("b"), pending: null, lockedAt: null, frontHealthy: true });
  });

  it("逾時後又來新殼：換掉背面那支、重新起算鎖定期；被換掉的那支晚到的 ready 不翻面", () => {
    const kept = framesReducer(healthyWaiting(), { type: "timeout" });
    const next = framesReducer(kept, { type: "shell", slot: slot("c"), now: 5000 });
    expect(next).toEqual({ front: slot("a"), pending: slot("c"), lockedAt: 5000, frontHealthy: true });
    expect(framesReducer(next, ok("b"))).toBe(next);
  });

  it("載入失敗的 ready：front 載好過就比照逾時保留；晚到的失敗殼也不翻上去", () => {
    const waiting = healthyWaiting();
    expect(framesReducer(waiting, failed("b"))).toEqual({ front: slot("a"), pending: slot("b"), lockedAt: null, frontHealthy: true });
    const kept = framesReducer(waiting, { type: "timeout" });
    expect(framesReducer(kept, failed("b"))).toEqual(kept);
  });

  it("front 沒載好過（沒回報或載入失敗）：逾時與失敗照舊翻到 pending；強制翻上來的 front 等自己回報才算載好", () => {
    const unknown = run(EMPTY_FRAMES, slot("a"), slot("b"));
    expect(framesReducer(unknown, { type: "timeout" })).toEqual({ front: slot("b"), pending: null, lockedAt: null, frontHealthy: false });
    const broken = run(framesReducer(run(EMPTY_FRAMES, slot("a")), failed("a")), slot("b"));
    expect(broken.frontHealthy).toBe(false);
    expect(framesReducer(broken, failed("b"))).toEqual({ front: slot("b"), pending: null, lockedAt: null, frontHealthy: false });
    const forced = framesReducer(unknown, { type: "timeout" });
    expect(framesReducer(forced, ok("b")).frontHealthy).toBe(true);
  });

  it("顯示對象不同硬切；回到 front 丟掉 pending；殼沒了全卸", () => {
    const waiting = run(EMPTY_FRAMES, slot("a"), slot("b"));
    expect(framesReducer(waiting, { type: "shell", slot: slot("x", "桌B"), now: 0 })).toEqual({ front: slot("x", "桌B"), pending: null, lockedAt: null, frontHealthy: false });
    expect(framesReducer(waiting, { type: "shell", slot: slot("a"), now: 0 })).toEqual({ front: slot("a"), pending: null, lockedAt: null, frontHealthy: false });
    // 逾時保留舊殼後回到 front（收回上一句）：丟掉背面、front 仍算載好
    const kept = framesReducer(healthyWaiting(), { type: "timeout" });
    expect(framesReducer(kept, { type: "shell", slot: slot("a"), now: 0 })).toEqual({ front: slot("a"), pending: null, lockedAt: null, frontHealthy: true });
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

  it("逾時保留舊殼（解鎖、pending 仍在背面）：front 的 input 照收，storage 仍只收 pending（舊殼設定不保存）", () => {
    const front = frame();
    const pending = frame();
    const registry = { front: { key: "a", frame: front.frame }, pending: { key: "b", frame: pending.frame }, locked: false };
    expect(acceptsInput(registry, front.win, "a")).toBe(true);
    expect(acceptsInput(registry, pending.win, "b")).toBe(false);
    expect(acceptsStorage(registry, front.win, "a")).toBe(false);
    expect(acceptsStorage(registry, pending.win, "b")).toBe(true);
  });
});
