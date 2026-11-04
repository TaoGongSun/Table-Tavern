// 圖庫請求撞到限流：等 retryAfterMs 到期自動重送一次，再失敗才交給畫面顯示
import { describe, expect, it, vi } from "vitest";
import { ask } from "./CardGallery";

describe("圖庫請求的限流重送", () => {
  it("撞到限流等到期重送一次；其他錯誤與第二次限流照回", async () => {
    vi.useFakeTimers();
    const act = vi.fn()
      .mockResolvedValueOnce({ ok: false, code: "E_RATE_LIMITED", retryAfterMs: 1500 })
      .mockResolvedValueOnce({ ok: true, result: { items: [], more: 0 } });
    const reply = ask(act, { file: "1.png" });
    await vi.advanceTimersByTimeAsync(1499);
    expect(act).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(await reply).toEqual({ ok: true, result: { items: [], more: 0 } });
    expect(act).toHaveBeenLastCalledWith("card.gallery", { file: "1.png" });
    const twice = vi.fn().mockResolvedValue({ ok: false, code: "E_RATE_LIMITED", retryAfterMs: 10 });
    const second = ask(twice, {});
    await vi.advanceTimersByTimeAsync(10);
    expect((await second).ok).toBe(false);
    expect(twice).toHaveBeenCalledTimes(2);
    const other = vi.fn().mockResolvedValue({ ok: false, code: "E_FORBIDDEN" });
    expect(await ask(other, {})).toMatchObject({ code: "E_FORBIDDEN" });
    expect(other).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });
});
