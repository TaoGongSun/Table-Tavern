import { describe, expect, it } from "vitest";
import { deviceId, isWeChat, joinedNickname, parseInvite, rememberJoined, withoutPairCode } from "./invite";

describe("邀請連結與本機身分", () => {
  it("解析房間碼（大小寫不分）、加入密鑰與出額度者配對碼；不是房間頁回 null", () => {
    expect(parseInvite("https://t.example/r/abcd2345#k=SECRET&p=AB12-CD34")).toEqual({ code: "ABCD2345", joinSecret: "SECRET", pairCode: "AB12-CD34" });
    expect(parseInvite("https://t.example/r/ABCD2345")).toEqual({ code: "ABCD2345", joinSecret: null, pairCode: null });
    expect(parseInvite("https://t.example/")).toBeNull();
  });

  it("加入後把一次性配對碼從網址拿掉，加入密鑰留著", () => {
    expect(withoutPairCode("https://t.example/r/ABCD2345#k=S&p=X")).toBe("https://t.example/r/ABCD2345#k=S");
    expect(withoutPairCode("https://t.example/r/ABCD2345#p=X&k=S")).toBe("https://t.example/r/ABCD2345#k=S");
    expect(withoutPairCode("https://t.example/r/ABCD2345#k=S")).toBe("https://t.example/r/ABCD2345#k=S");
  });

  it("認得微信內建瀏覽器", () => {
    expect(isWeChat("Mozilla/5.0 (Linux; Android 14) MicroMessenger/8.0.50")).toBe(true);
    expect(isWeChat("Mozilla/5.0 (iPhone) Safari/604.1")).toBe(false);
  });

  it("deviceId 是 22 字 base64url、存起來重用；瀏覽器不讓存時照樣給一個（只在這次有效）", () => {
    const map = new Map<string, string>();
    const storage = { getItem: (k: string) => map.get(k) ?? null, setItem: (k: string, v: string) => void map.set(k, v) } as Storage;
    const id = deviceId(storage);
    expect(id).toMatch(/^[A-Za-z0-9_-]{22}$/);
    expect(deviceId(storage)).toBe(id);
    const broken = { getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } } as unknown as Storage;
    expect(deviceId(broken)).toMatch(/^[A-Za-z0-9_-]{22}$/);
    rememberJoined(broken, "ABCD2345", "阿明");
    expect(joinedNickname(broken, "ABCD2345")).toBeNull();
    rememberJoined(storage, "ABCD2345", "阿明");
    expect(joinedNickname(storage, "ABCD2345")).toBe("阿明");
  });
});
