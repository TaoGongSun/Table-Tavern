import { describe, expect, it } from "vitest";
import {
  deviceId,
  inAppBrowser,
  joinedNickname,
  parseInvite,
  rememberJoined,
  withoutPairCode,
} from "./invite";

describe("邀請連結與本機身分", () => {
  it("解析房間碼（大小寫不分）、加入密鑰與出額度者配對碼；不是房間頁回 null", () => {
    expect(parseInvite("https://t.example/r/abcd2345#k=SECRET&p=AB12-CD34")).toEqual({
      code: "ABCD2345",
      joinSecret: "SECRET",
      pairCode: "AB12-CD34",
    });
    expect(parseInvite("https://t.example/r/ABCD2345")).toEqual({
      code: "ABCD2345",
      joinSecret: null,
      pairCode: null,
    });
    expect(parseInvite("https://t.example/")).toBeNull();
  });

  it("加入後把一次性配對碼從網址拿掉，加入密鑰留著", () => {
    expect(withoutPairCode("https://t.example/r/ABCD2345#k=S&p=X")).toBe(
      "https://t.example/r/ABCD2345#k=S",
    );
    expect(withoutPairCode("https://t.example/r/ABCD2345#p=X&k=S")).toBe(
      "https://t.example/r/ABCD2345#k=S",
    );
    expect(withoutPairCode("https://t.example/r/ABCD2345#k=S")).toBe(
      "https://t.example/r/ABCD2345#k=S",
    );
  });

  it("認得手機 QQ 與微信的內建瀏覽器，QQ 瀏覽器 App 與一般瀏覽器不算", () => {
    const qqAndroid =
      "Mozilla/5.0 (Linux; Android 13; PGT-AN10) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/89.0.4389.72 MQQBrowser/6.2 TBS/046295 Mobile Safari/537.36 V1_AND_SQ_8.9.80_4614_YYB_D QQ/8.9.80.12440 NetType/WIFI WebP/0.3.0 AppId/537176902 Pixel/1080";
    const qqIos =
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 QQ/8.9.80.611 V1_IPH_SQ_8.9.80_1_APP_A Pixel/1179 Core/WKWebView";
    const qqBrowserApp =
      "Mozilla/5.0 (Linux; U; Android 13; zh-cn; PGT-AN10) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/109.0.5414.86 MQQBrowser/14.3 Mobile Safari/537.36 COVC/046701";
    const wechat =
      "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 Chrome/116.0 Mobile Safari/537.36 XWEB/1160065 MMWEBSDK/20231202 MMWEBID/1234 MicroMessenger/8.0.50.2701(0x28003253) WeChat/arm64 Weixin NetType/WIFI";
    expect(inAppBrowser(qqAndroid)).toBe("qq");
    expect(inAppBrowser(qqIos)).toBe("qq");
    expect(inAppBrowser(qqBrowserApp)).toBeNull();
    expect(inAppBrowser(wechat)).toBe("wechat");
    expect(
      inAppBrowser(
        "Mozilla/5.0 (iPhone) AppleWebKit/605.1.15 Version/18.0 Mobile/15E148 Safari/604.1",
      ),
    ).toBeNull();
  });

  it("deviceId 是 22 字 base64url、存起來重用；瀏覽器不讓存時照樣給一個（只在這次有效）", () => {
    const map = new Map<string, string>();
    const storage = {
      getItem: (k: string) => map.get(k) ?? null,
      setItem: (k: string, v: string) => void map.set(k, v),
    } as Storage;
    const id = deviceId(storage);
    expect(id).toMatch(/^[A-Za-z0-9_-]{22}$/);
    expect(deviceId(storage)).toBe(id);
    const broken = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    } as unknown as Storage;
    expect(deviceId(broken)).toMatch(/^[A-Za-z0-9_-]{22}$/);
    rememberJoined(broken, "ABCD2345", "阿明");
    expect(joinedNickname(broken, "ABCD2345")).toBeNull();
    rememberJoined(storage, "ABCD2345", "阿明");
    expect(joinedNickname(storage, "ABCD2345")).toBe("阿明");
  });
});
