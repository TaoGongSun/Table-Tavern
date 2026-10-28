import { describe, expect, it } from "vitest";
import { cliUpdateOffer } from "./cli-update";

const msg = (code: string, params: Record<string, string> = {}) =>
  `TTMSG:${JSON.stringify({ code, ...params })}`;
const reply = (error: string) => msg("cli_reply_error", { error });
const crashed = (tail: string) => reply(msg("cli_crashed", { status: "exit status: 1", tail }));

describe("cliUpdateOffer", () => {
  it("模型叫不到與 CLI 太舊各自出按鈕", () => {
    expect(
      cliUpdateOffer(`Error: AI_CALL_FAILED: ${msg("cli_model_unavailable", { cli: "codex", model: "gpt-x" })}`, "codex"),
    ).toEqual({ cli: "codex", kind: "model", model: "gpt-x" });
    expect(
      cliUpdateOffer(msg("cli_too_old_required", { cli: "agy", version: "1.1.0", required: "1.2.1" }), "agy"),
    ).toEqual({ cli: "agy", kind: "too_old", required: "1.2.1" });
    expect(cliUpdateOffer(msg("cli_too_old", { cli: "grok", version: "" }), "grok")).toEqual({
      cli: "grok",
      kind: "too_old",
    });
    expect(
      cliUpdateOffer(
        msg("cli_output_unrecognized_older", { cli: "Claude", version: "2.1.1", tested: "2.1.311", tail: "" }),
        "claude",
      ),
    ).toEqual({ cli: "claude", kind: "older" });
  });

  it("無法分類的 CLI 自報錯誤與崩潰才放按鈕", () => {
    expect(cliUpdateOffer(reply("something odd happened"), "claude")).toEqual({ cli: "claude", kind: "unknown" });
    expect(cliUpdateOffer(crashed("segfault"), "grok")).toEqual({ cli: "grok", kind: "unknown" });
  });

  it("排除清單一律不放按鈕", () => {
    const excluded = [
      msg("cli_stalled"),
      msg("cli_stalled_after_output"),
      msg("cli_not_found", { cli: "claude" }),
      msg("cli_updating", { cli: "claude" }),
      msg("cli_output_unrecognized_never", { cli: "Claude", version: "2.1.311", tail: "" }),
      reply("You've hit your usage limit"),
      reply("429 Too Many Requests"),
      reply("Not logged in · Please run /login"),
      reply("401 Unauthorized"),
      reply("API Error: 529 overloaded, retrying"),
      reply("network error: ECONNRESET"),
      "AI_CONTEXT_TOO_LONG: prompt is too long",
      "AI_INCOMPLETE_RESPONSE: finish_reason=max_tokens",
      "AI_STREAM_STALLED: no progress",
    ];
    for (const raw of excluded) expect(cliUpdateOffer(raw, "claude"), raw).toBeNull();
  });

  it("5xx 只認 HTTP 狀態的前後文，版號裡的三位數不算", () => {
    expect(cliUpdateOffer(reply("crashed while loading 2.1.500 plugin"), "claude")).toEqual({
      cli: "claude",
      kind: "unknown",
    });
    for (const raw of [reply("status 503"), reply("502 Bad Gateway"), reply("HTTP/1.1 500"), reply("Error: 504")]) {
      expect(cliUpdateOffer(raw, "claude"), raw).toBeNull();
    }
  });

  it("更新碼跟登入字樣同時出現：給更新鈕，不出重新登入", () => {
    const raw = `AI_CALL_FAILED: ${msg("cli_too_old", { cli: "claude", version: "not logged in" })}`;
    expect(cliUpdateOffer(raw, "claude")).toEqual({ cli: "claude", kind: "too_old" });
  });

  it("不是 CLI 傳輸就不放", () => {
    expect(cliUpdateOffer(reply("odd"), "api")).toBeNull();
    expect(cliUpdateOffer(reply("odd"))).toBeNull();
  });
});
