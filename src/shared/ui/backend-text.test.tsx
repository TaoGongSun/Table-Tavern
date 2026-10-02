// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { setLang, t } from "../../i18n";
import { ErrorNote } from "../../views/atoms";
import { explainAiError } from "./ai-error";
import { backendCode, backendText } from "./backend-text";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// 正式字典目前只有 io_failed；多掛一個帶非 error 參數與數字參數的測試碼，驗證代入規則。
vi.mock("../../i18n/features/backend-msg", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../i18n/features/backend-msg")>();
  return {
    ...actual,
    BACKEND_MSG_PARAMS: {
      ...actual.BACKEND_MSG_PARAMS,
      test_pair: { path: "string", count: "number", error: "string" },
    },
    isBackendMsgKey: (key: string) => key === "be_test_pair" || actual.isBackendMsgKey(key),
    backendMsgMessage: (lang: "zh-TW", key: Parameters<typeof actual.backendMsgMessage>[1]) =>
      (key as string) === "be_test_pair"
        ? "P={path} N={count} E={error}"
        : actual.backendMsgMessage(lang, key),
  };
});

const io = (error: string) => `TTMSG:${JSON.stringify({ code: "io_failed", error })}`;

describe("backendText", () => {
  afterEach(() => setLang("zh-TW"));

  it("沒有標記的原文（含舊版中文）原樣", () => {
    expect(backendText("這張桌正在處理中，請稍候再試")).toBe("這張桌正在處理中，請稍候再試");
    expect(backendText("")).toBe("");
    expect(backendText(new Error("x"))).toBe("Error: x");
  });

  it("依目前語系翻譯，前後文字保留", () => {
    expect(backendText(io("disk full"))).toBe("讀寫檔案失敗：disk full");
    setLang("ru");
    expect(backendText(`前 ${io("disk full")} 後`)).toBe(
      "前 Не удалось прочитать или записать файл: disk full 後",
    );
  });

  it("Error: 包裝與 AI_HTTP_STATUS_ 前綴在前都只翻標記本身", () => {
    expect(backendText(`Error: ${io("x")}`)).toBe("Error: 讀寫檔案失敗：x");
    const raw = `AI_HTTP_STATUS_503: ${io("upstream")}`;
    expect(backendText(raw)).toBe("AI_HTTP_STATUS_503: 讀寫檔案失敗：upstream");
    expect(explainAiError(raw, "api")).toBe("errApiUpstream");
  });

  it("同串多個標記都翻", () => {
    expect(backendText(`${io("a")} / ${io("b")}`)).toBe("讀寫檔案失敗：a / 讀寫檔案失敗：b");
  });

  it("JSON 字串裡的引號、跳脫與括號不會截斷物件", () => {
    expect(backendText(io('a } " { \\ b'))).toBe('讀寫檔案失敗：a } " { \\ b');
  });

  it("壞 JSON 原文保留，後面的好標記照解", () => {
    expect(backendText(`TTMSG:{"code":"io_failed" ${io("ok")}`)).toBe(
      'TTMSG:{"code":"io_failed" 讀寫檔案失敗：ok',
    );
    expect(backendText(`TTMSG:{code:io_failed} ${io("ok")}`)).toBe(
      "TTMSG:{code:io_failed} 讀寫檔案失敗：ok",
    );
    expect(backendText(`TTMSG: ${io("ok")}`)).toBe("TTMSG: 讀寫檔案失敗：ok");
    expect(backendText('TTMSG:["io_failed"]')).toBe('TTMSG:["io_failed"]');
  });

  it("括號成對但語法壞掉時，夾在裡面的好標記照解", () => {
    expect(backendText(`TTMSG:{bad ${io("ok")}}`)).toBe("TTMSG:{bad 讀寫檔案失敗：ok}");
  });

  it("未知 code、缺參數、型別不對都整段原文", () => {
    for (const raw of [
      'TTMSG:{"code":"nope","error":"x"}',
      'TTMSG:{"code":"io_failed"}',
      'TTMSG:{"code":"io_failed","error":3}',
      'TTMSG:{"error":"x"}',
      'TTMSG:{"code":"toString","error":"x"}',
      'TTMSG:{"code":"test_pair","path":"/a","count":"1","error":"x"}',
    ]) {
      expect(backendText(raw)).toBe(raw);
    }
  });

  it("參數含 {x} 不會被再代入", () => {
    const raw = `TTMSG:${JSON.stringify({ code: "test_pair", path: "{error}", count: 2, error: "{path}" })}`;
    expect(backendText(raw)).toBe("P={error} N=2 E={path}");
  });

  it("只有 error 欄位做巢狀翻譯，其他參數裡藏的 TTMSG 原文代入", () => {
    const raw = `TTMSG:${JSON.stringify({ code: "test_pair", path: io("p"), count: 1, error: io("e") })}`;
    expect(backendText(raw)).toBe(`P=${io("p")} N=1 E=讀寫檔案失敗：e`);
  });

  it("巢狀最多翻三層，更深的原文保留", () => {
    const deep = io(io(io(io("root"))));
    expect(backendText(deep)).toBe(`讀寫檔案失敗：讀寫檔案失敗：讀寫檔案失敗：${io("root")}`);
  });
});

describe("backendCode", () => {
  it("只認起首的合法標記，容許 Error: 包裝", () => {
    expect(backendCode(io("x"))).toBe("io_failed");
    expect(backendCode(`Error: ${io("x")}`)).toBe("io_failed");
    expect(backendCode(new Error(io("x")))).toBe("io_failed");
  });

  it("夾在中間、前綴在前、壞碼或未知碼都回 null", () => {
    expect(backendCode(`前文 ${io("x")}`)).toBeNull();
    expect(backendCode(`AI_HTTP_STATUS_500: ${io("x")}`)).toBeNull();
    expect(backendCode('TTMSG:{"code":"io_failed"')).toBeNull();
    expect(backendCode('TTMSG:{"code":"nope"}')).toBeNull();
    expect(backendCode('TTMSG:{"code":"io_failed","error":1}')).toBeNull();
    expect(backendCode("這張桌正在處理中，請稍候再試")).toBeNull();
  });
});

// t() 是 backendText 的代入出口：參數值（供應商原文、路徑）裡的 {名} 不能被再換。
describe("t() 參數代入", () => {
  afterEach(() => setLang("zh-TW"));

  it("對模板只掃一次：先代入的值含 {名} 不會被後面的參數再換", () => {
    expect(t("smartFreeDailyLeft", { remaining: "{limit}", limit: 5 })).toBe(
      "今日免費配額 {limit}/5（全模型共用）",
    );
    expect(t("smartFreeDailyLeft", { limit: "{remaining}", remaining: 3 })).toBe(
      "今日免費配額 3/{remaining}（全模型共用）",
    );
  });

  it("同一佔位符出現多次都換；值裡的 $ 樣式照字面", () => {
    expect(t("readOnlyBanner", { version: "$&1.2" })).toBe(
      "這張桌由 $&1.2 版建立或轉換，要繼續請更新到 $&1.2 版或更新版本",
    );
  });

  it("沒給的佔位符原樣留著，不帶參數就回模板", () => {
    expect(t("smartFreeDailyLeft", { remaining: 1 })).toBe("今日免費配額 1/{limit}（全模型共用）");
    expect(t("needsRepair_io")).toBe("修復時讀寫失敗，原桌、備份與另存都還在：{error}");
  });

  it("補充字典的鍵跟主字典一樣走語系", () => {
    setLang("ru");
    expect(t("be_io_failed", { error: "x" })).toBe("Не удалось прочитать или записать файл: x");
  });
});

// ErrorNote：分流吃原文、顯示走 backendText。
describe("ErrorNote", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
    setLang("zh-TW");
  });

  function show(text: string, transport?: string): HTMLElement {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);
    act(() => root!.render(<ErrorNote text={text} transport={transport} />));
    return host.querySelector("[role=alert]") as HTMLElement;
  }

  it("分流沒命中：主文字顯示翻譯後的後端訊息", () => {
    setLang("ru");
    expect(show(io("disk full")).textContent).toBe(
      "Не удалось прочитать или записать файл: disk full",
    );
  });

  it("分流吃原文：前綴照認，小字翻譯", () => {
    const note = show(`AI_HTTP_STATUS_429: ${io("busy")}`, "api");
    expect(note.querySelector("small")?.textContent).toBe(
      "AI_HTTP_STATUS_429: 讀寫檔案失敗：busy",
    );
    expect(note.firstChild?.textContent).toBe(t("errQuotaApi"));
  });
});
