// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { setLang, t } from "../../i18n";
import { ErrorNote } from "./atoms";
import { explainAiError } from "./ai-error";
import { backendCode, backendText } from "./backend-text";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// 多掛一個同時帶非 error 字串、數字與 error 參數的測試碼，驗證代入規則。
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

// 重構餘段 audit：title 是後端產生當時的完整餘段標題，畫面照目前語系翻句子、標題原樣。
describe("refactor_span_leftover", () => {
  afterEach(() => setLang("zh-TW"));
  const leftover = (title: string) =>
    `TTMSG:${JSON.stringify({ code: "refactor_span_leftover", title })}`;

  it("解得出新參數，切語系後仍引用產生當時的標題", () => {
    const raw = leftover("條目A (leftover)");
    expect(backendCode(raw)).toBe("refactor_span_leftover");
    expect(backendText(raw)).toBe("此段未獲有效路由，已併入「條目A (leftover)」條目照搬。");
    setLang("ru");
    expect(backendText(raw)).toBe(
      "У этого фрагмента нет подходящего назначения, поэтому он перенесён как есть в запись «條目A (leftover)».",
    );
  });

  it("標題含引號、反斜線、{name}、TTMSG: 都原樣代入", () => {
    const title = '「引號」"q" \\ {name} TTMSG:{"code":"world_busy"} (leftover)';
    setLang("en");
    expect(backendText(leftover(title))).toBe(
      `This span had no valid route, so it was merged into the “${title}” entry as-is.`,
    );
  });

  it("舊存檔沒有 title 參數：照既有規則整段原文", () => {
    const old = 'TTMSG:{"code":"refactor_span_leftover"}';
    expect(backendText(old)).toBe(old);
  });
});

// t() 是 backendText 的代入出口：參數值（供應商原文、路徑）裡的 {名} 不能被再換。
describe("桌資料面代碼", () => {
  afterEach(() => setLang("zh-TW"));
  const busy = 'TTMSG:{"code":"world_busy"}';

  it("world_busy 判碼不看語系，ru 下顯示俄文", () => {
    setLang("ru");
    expect(backendCode(busy)).toBe("world_busy");
    expect(backendCode(`Error: ${busy}`)).toBe("world_busy");
    expect(backendText(busy)).toBe(t("worldBusy"));
    expect(backendText(busy)).not.toMatch(/[\u4e00-\u9fff]/);
    // 舊版後端的繁中原句不再被當成忙碌碼
    expect(backendCode("這張桌正在處理中，請稍候再試")).toBeNull();
  });

  it("數字與路徑參數照原文代入，error 包裹的代碼也翻", () => {
    setLang("ru");
    expect(backendText('TTMSG:{"code":"scene_not_found","scene":3}')).toBe("Акта 3 не существует.");
    expect(
      backendText(io('TTMSG:{"code":"rename_failed","from":"/a/世界","to":"/b"}')),
    ).toBe("Не удалось прочитать или записать файл: Не удалось переименовать: /a/世界 → /b");
    // 參數型別不對（scene 應為數字）就整段原文
    const wrong = 'TTMSG:{"code":"scene_not_found","scene":"3"}';
    expect(backendText(wrong)).toBe(wrong);
  });
});

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

  it("state 存原文：繁中觸發桌忙碌後切成俄文，重繪就換成俄文", () => {
    const busy = show('TTMSG:{"code":"world_busy"}');
    expect(busy.textContent).toBe("這張桌正在處理中，請稍候再試");
    setLang("ru");
    act(() => root!.render(<ErrorNote text='TTMSG:{"code":"world_busy"}' />));
    expect(host!.querySelector("[role=alert]")?.textContent).toBe(t("worldBusy"));
    expect(host!.textContent).not.toMatch(/[一-鿿]/);
  });

  it("分流吃原文：前綴照認，小字翻譯", () => {
    const note = show(`AI_HTTP_STATUS_429: ${io("busy")}`, "api");
    expect(note.querySelector("small")?.textContent).toBe(
      "AI_HTTP_STATUS_429: 讀寫檔案失敗：busy",
    );
    expect(note.firstChild?.textContent).toBe(t("errQuotaApi"));
  });
});

// 包 4：AI 連線面的錯誤改代碼後，AI_* 前綴仍在起首、explainAiError 的分流跟改碼前一致。
describe("AI 連線面代碼", () => {
  afterEach(() => setLang("zh-TW"));
  const msg = (code: string, params: Record<string, string> = {}) =>
    `TTMSG:${JSON.stringify({ code, ...params })}`;
  const keyMissing = msg("openrouter_api_key_missing");

  it("AI_* 前綴留在起首：分流照認，代碼只翻後面那段", () => {
    setLang("ru");
    const daily = `AI_HTTP_STATUS_429: ${msg("smart_free_daily_exhausted")}`;
    expect(explainAiError(daily, "api")).toBe("errQuotaApi");
    expect(backendCode(daily)).toBeNull();
    expect(backendText(daily)).toBe(
      "AI_HTTP_STATUS_429: Сегодняшние запросы к бесплатным моделям OpenRouter закончились. Попробуй снова после сброса лимита.",
    );
    // 傳輸層自己的失敗態後面改成不帶語言的診斷欄
    const http = 'AI_HTTP_STATUS_503: status=503 Service Unavailable body={"error":"x"}';
    expect(explainAiError(http, "api")).toBe("errApiUpstream");
    expect(backendText(http)).toBe(http);
    expect(explainAiError("AI_INCOMPLETE_RESPONSE: model=m status=no_terminal_event")).toBe(
      "errIncompleteReply",
    );
    expect(explainAiError("AI_EMPTY_RESPONSE: no_text_after_control_lines raw_len=12")).toBe(
      "errEmptyReply",
    );
  });

  it("缺 OpenRouter key 的分流跟改碼前的繁中原句一致", () => {
    const old = "尚未設定 OpenRouter API key，請先到設定貼上";
    for (const transport of ["api", "claude", undefined]) {
      expect(explainAiError(keyMissing, transport)).toBe(explainAiError(old, transport));
      expect(explainAiError(`AI_CALL_FAILED: ${keyMissing}`, transport)).toBe(
        explainAiError(`AI_CALL_FAILED: ${old}`, transport),
      );
      expect(explainAiError(`Error: AI_CALL_FAILED: ${keyMissing}`, transport)).toBe(
        explainAiError(`Error: AI_CALL_FAILED: ${old}`, transport),
      );
    }
    expect(explainAiError(keyMissing, "api")).toBe("errAuthApi");
  });

  it("CLI 原話包進 cli_reply_error 後，額度／登入分流不變", () => {
    const quota = `AI_CALL_FAILED: ${msg("cli_reply_error", { error: "Rate limit exceeded" })}`;
    expect(explainAiError(quota, "claude")).toBe("errQuota");
    const login = `AI_CALL_FAILED: ${msg("cli_reply_error", { error: "not logged in" })}`;
    expect(explainAiError(login, "grok")).toBe("errAuthCli");
    // 沒有可認的原話就落到保底，不被代碼本身的字樣誤判
    const stalled = `AI_CALL_FAILED: ${msg("cli_reply_error", { error: msg("cli_stalled") })}`;
    expect(explainAiError(stalled, "claude")).toBe("errAiUnknown");
    expect(explainAiError(msg("cli_not_found", { cli: "agy" }), "agy")).toBeNull();
    expect(explainAiError(msg("cli_risk_not_accepted"), "claude")).toBeNull();
  });

  it("生圖暗號只剩字樣本身，分流不變", () => {
    expect(explainAiError("AI_CALL_FAILED: REFUSED", "codex")).toBe("errRefused");
    expect(explainAiError("NO_IMAGE", "codex")).toBe("errNoImage");
  });

  it("ru 下顯示俄文，巢狀的 CLI 失敗也翻、原話照代入", () => {
    setLang("ru");
    const crashed = msg("cli_reply_error", {
      error: msg("cli_crashed", { status: "exit status: 3", tail: "proxy connection reset" }),
    });
    const text = backendText(`AI_CALL_FAILED: ${crashed}`);
    expect(text).toBe(
      "AI_CALL_FAILED: Ошибка CLI: CLI неожиданно завершился (exit status: 3): proxy connection reset",
    );
    expect(text).not.toMatch(/[一-鿿]/);
    expect(backendText(keyMissing)).toBe("API key OpenRouter ещё не задан. Вставь его в настройках.");
  });
});

// 包 5：畫面說明（重構審閱 detail、帳本 detail、收據名稱、官方別名）落檔存代碼，舊檔是中文原句。
describe("畫面說明代碼", () => {
  afterEach(() => setLang("zh-TW"));
  const msg = (code: string, params: Record<string, string> = {}) =>
    `TTMSG:${JSON.stringify({ code, ...params })}`;

  it("新代碼依語系翻，舊中文原樣，同一份帳本可混存", () => {
    setLang("ru");
    const ledger = [msg("ledger_scaffold_absorbed"), "機制鷹架條目，已由本地機制接管，不再送入提示詞。"];
    expect(ledger.map(backendText)).toEqual([
      "Служебная запись механики: теперь её обрабатывает локальная механика приложения, в промпт она больше не попадает.",
      "機制鷹架條目，已由本地機制接管，不再送入提示詞。",
    ]);
    expect(backendText(msg("receipt_refactor_apply"))).toBe("ИИ-разбор карточки");
    expect(backendText(msg("cli_model_alias", { alias: "opus" }))).toBe("opus (официальный алиас)");
  });

  it("AI 給的照搬理由當參數原文代入，裡面的大括號與標記字樣不再被換", () => {
    const reason = msg("refactor_carry_reason", { reason: "保留 {name} 與 TTMSG: 字樣" });
    expect(backendText(reason)).toBe("照搬理由：保留 {name} 與 TTMSG: 字樣");
    setLang("en");
    expect(backendText(msg("refactor_person_span_invalid", { name: "伊利亞" }))).toBe(
      "“伊利亞” uses mode=clean but references invalid spans; sent back to the expand queue.",
    );
  });
});
