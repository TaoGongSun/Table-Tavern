import { describe, expect, it } from "vitest";
import { detectRefactorCardState, probeFailureKind, stripTransportPrefix } from "./refactor-mode";
import { type CardInterface, type InterfaceScript } from "../card-interface/interface-card";

function script(): InterfaceScript {
  return { name: "s", find_regex: "/x/", replace_string: "<div>x</div>", trim_strings: [], min_depth: null, max_depth: null };
}

function card(overrides: Partial<CardInterface>): CardInterface {
  return { character_id: "c1", character_name: "角色", scripts: [], unsupported: null, opening: null, ...overrides };
}

describe("detectRefactorCardState", () => {
  it("無介面卡＝none（免問直跑角色線）", () => {
    expect(detectRefactorCardState([])).toBe("none");
    expect(detectRefactorCardState([card({})])).toBe("none");
  });

  it("有可用顯示腳本＝supported（進二選一）", () => {
    expect(detectRefactorCardState([card({ scripts: [script()] })])).toBe("supported");
  });

  it("只有 DRM／雲端載入器卡＝unsupported（擋下）", () => {
    expect(detectRefactorCardState([card({ unsupported: "remote_loader" })])).toBe("unsupported");
    expect(detectRefactorCardState([card({ unsupported: "scrypt" })])).toBe("unsupported");
  });

  it("任一張原卡讀不出＝unreadable，優先於 supported 與 unsupported", () => {
    const unreadable = card({ character_id: "c3", unsupported: "unreadable" });
    expect(detectRefactorCardState([unreadable])).toBe("unreadable");
    expect(detectRefactorCardState([card({ scripts: [script()] }), unreadable])).toBe("unreadable");
    expect(detectRefactorCardState([card({ unsupported: "remote_loader" }), unreadable])).toBe("unreadable");
  });

  it("混合桌取還有得救優先：一張可接管＋一張擋下＝supported", () => {
    expect(
      detectRefactorCardState([card({ unsupported: "remote_loader" }), card({ character_id: "c2", scripts: [script()] })]),
    ).toBe("supported");
  });
});

describe("probeFailureKind（初判失敗分類，2026-10-12 作者裁決）", () => {
  const msg = (code: string, params: Record<string, string> = {}) =>
    `TTMSG:${JSON.stringify({ code, ...params })}`;

  it("白名單碼（含 Error: 包裝）＝not_sent", () => {
    for (const raw of [
      msg("cli_risk_not_accepted"),
      `Error: ${msg("cli_risk_not_accepted")}`,
      msg("cli_not_found", { cli: "claude" }),
      msg("agy_too_old", { version: "1.0.0" }),
      msg("unknown_transport", { transport: "x" }),
      msg("update_gate_closed"),
      msg("tier_model_missing", { tier: "gm" }),
      msg("no_free_models"),
      msg("no_stable_free_model"),
      msg("openrouter_api_key_missing"),
      `AI_CALL_FAILED: ${msg("openrouter_api_key_missing")}`,
      `Error: AI_CALL_FAILED: ${msg("openrouter_api_key_missing")}`,
      `AI_HTTP_STATUS_429: ${msg("smart_free_daily_exhausted")}`,
      `Error: AI_HTTP_STATUS_429: ${msg("smart_free_daily_exhausted")}`,
    ]) {
      expect(probeFailureKind(raw), raw).toBe("not_sent");
    }
  });

  it("送出後失敗、殼內非白名單、認不出的＝failed", () => {
    for (const raw of [
      "AI_CALL_FAILED: error sending request",
      "AI_HTTP_STATUS_429: status=429 body=rate limited",
      "AI_EMPTY_RESPONSE: ",
      `AI_CALL_FAILED: ${msg("cli_reply_error", { error: "boom" })}`,
      `AI_CALL_FAILED: ${msg("cli_not_found", { cli: "claude" })}`,
      `AI_HTTP_STATUS_429: status=429 body=${msg("smart_free_daily_exhausted")}`,
      msg("some_future_code"),
      "讀卡失敗",
    ]) {
      expect(probeFailureKind(raw), raw).toBe("failed");
    }
  });

  it("剝掉傳輸層前綴只留後端代碼", () => {
    const daily = msg("smart_free_daily_exhausted");
    expect(stripTransportPrefix(`Error: AI_HTTP_STATUS_429: ${daily}`)).toBe(daily);
    const key = msg("openrouter_api_key_missing");
    expect(stripTransportPrefix(`AI_CALL_FAILED: ${key}`)).toBe(key);
    expect(stripTransportPrefix(msg("cli_risk_not_accepted"))).toBe(msg("cli_risk_not_accepted"));
  });

  it("sentinel 去掉 Error: 後精確比對", () => {
    expect(probeFailureKind("refactor-aborted")).toBe("aborted");
    expect(probeFailureKind(new Error("refactor-aborted"))).toBe("aborted");
    expect(probeFailureKind("refactor-recommend-unparsable")).toBe("unparsable");
    expect(probeFailureKind("AI_CALL_FAILED: refactor-aborted")).toBe("failed");
  });
});
