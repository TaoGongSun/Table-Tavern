import { describe, expect, it } from "vitest";
import { explainAiError, redactAiErrorDetail } from "./ai-error";

describe("explainAiError", () => {
  it("先認傳輸層的失敗碼，三態各自分流", () => {
    expect(explainAiError("AI_EMPTY_RESPONSE: model=x finish_reason=stop")).toBe("errEmptyReply");
    expect(explainAiError("AI_INCOMPLETE_RESPONSE: model=x finish_reason=length")).toBe(
      "errIncompleteReply",
    );
    expect(explainAiError("AI_CONTENT_FILTERED: model=x finish_reason=content_filter")).toBe(
      "errFiltered",
    );
  });

  it("碼被包裝過也認得（Tauri 錯誤在不同呼叫層會被套 Error:）", () => {
    expect(explainAiError("Error: AI_EMPTY_RESPONSE: 空白回合不寫進故事")).toBe("errEmptyReply");
  });

  it("供應商原話不帶碼，走既有正則——免費層限流要看到「額度用完」", () => {
    expect(explainAiError("Rate limit exceeded: free-models-per-day")).toBe("errQuota");
    expect(explainAiError("Provider disconnected unexpectedly")).toBe(null);
  });

  it("既有分流不受影響", () => {
    expect(explainAiError("REFUSED")).toBe("errRefused");
    expect(explainAiError("401 unauthorized")).toBe("errAuth");
  });

  it("認證失敗依傳輸指路：API 換金鑰、CLI 重新登入、認不出來就別亂指", () => {
    const raw = "status=401 Unauthorized body=";
    expect(explainAiError(raw, "api")).toBe("errAuthApi");
    for (const cli of ["claude", "codex", "agy", "grok"]) {
      expect(explainAiError(raw, cli), cli).toBe("errAuthCli");
    }
    // 設定還沒載入、或來源是沒見過的值：走中性文案，不可硬猜成 CLI
    expect(explainAiError(raw, undefined)).toBe("errAuth");
    expect(explainAiError(raw, "")).toBe("errAuth");
    expect(explainAiError(raw, "something-else")).toBe("errAuth");
  });

  // 以下是 API 路的真實 HTTP 狀態（transport.rs 掛在開頭）。不診斷根因，只指下一步——
  // 聚合 router 把上游任何失敗都轉包成自己的 5xx，真原因在轉包時就沒了。
  it("HTTP 碼壓過 body 裡的數字：狀態 503、body 自稱 429，仍算上游失敗", () => {
    // 2026-08-21 實測的原句：TokenRouter 對 qwen 的長 prompt 一律回這個
    const real =
      'AI_HTTP_STATUS_503: status=503 Service Unavailable body={"error":{"message":"openai_error","type":"bad_response_status_code"},"id":157975}';
    expect(explainAiError(real, "api")).toBe("errApiUpstream");
    // body 裡出現 429／unauthorized 都不該翻盤，否則等於讓供應商偽造狀態碼
    expect(
      explainAiError('AI_HTTP_STATUS_503: status=503 body={"message":"upstream 429 rate limit"}', "api"),
    ).toBe("errApiUpstream");
    expect(
      explainAiError('AI_HTTP_STATUS_500: status=500 body={"message":"unauthorized upstream"}', "api"),
    ).toBe("errApiUpstream");
  });

  it("HTTP 碼也壓過 body 裡抄到的失敗碼與暗號", () => {
    // 錯誤字串後半是供應商原封不動的 body，裡面出現什麼都不該翻盤前面的真實狀態
    expect(
      explainAiError('AI_HTTP_STATUS_503: status=503 body={"message":"AI_EMPTY_RESPONSE"}', "api"),
    ).toBe("errApiUpstream");
    expect(
      explainAiError('AI_HTTP_STATUS_503: status=503 body={"message":"REFUSED"}', "api"),
    ).toBe("errApiUpstream");
    expect(
      explainAiError('AI_HTTP_STATUS_403: status=403 body={"message":"AI_CONTENT_FILTERED: x"}', "api"),
    ).toBe("errApiForbidden");
  });

  it("傳輸層失敗碼同樣只認開頭：body 抄到字樣不算數", () => {
    expect(explainAiError("AI_EMPTY_RESPONSE: 空白回合")).toBe("errEmptyReply");
    expect(explainAiError("Error: AI_EMPTY_RESPONSE: 空白回合")).toBe("errEmptyReply");
    expect(explainAiError('某個失敗：{"log":"AI_EMPTY_RESPONSE"}')).toBe(null);
  });

  it("四類狀態各自指路：401 換金鑰、403 換模型、429 額度、其他 4xx 請求被擋", () => {
    expect(explainAiError("AI_HTTP_STATUS_401: status=401 body=", "api")).toBe("errAuthApi");
    expect(
      explainAiError("AI_HTTP_STATUS_403: status=403 body=This token has no access to model", "api"),
    ).toBe("errApiForbidden");
    expect(explainAiError("AI_HTTP_STATUS_429: status=429 body=", "api")).toBe("errQuotaApi");
    expect(explainAiError("AI_HTTP_STATUS_402: status=402 body=", "api")).toBe("errQuotaApi");
    expect(explainAiError("AI_HTTP_STATUS_400: status=400 body=", "api")).toBe("errApiRequest");
    expect(explainAiError("AI_HTTP_STATUS_404: status=404 body=", "api")).toBe("errApiRequest");
    expect(explainAiError("AI_HTTP_STATUS_502: status=502 body=", "api")).toBe("errApiUpstream");
    // 碼本身就證明來源是 API，沒傳 transport 也照樣指得出路
    expect(explainAiError("AI_HTTP_STATUS_403: status=403 body=")).toBe("errApiForbidden");
    // Tauri 包過一層仍認得
    expect(explainAiError("Error: AI_HTTP_STATUS_403: status=403 body=")).toBe("errApiForbidden");
  });

  it("碼只認開頭：供應商把它抄進 body 也不算數", () => {
    const forged = 'status=500 body={"message":"AI_HTTP_STATUS_403: 假的"}';
    expect(explainAiError(forged, "api")).toBe(null);
  });

  it("穩定免費連續擁擠：認 AI_FREE_MODEL_BUSY 開頭，內層的 HTTP 碼與 body 字樣翻不了盤", () => {
    expect(explainAiError("AI_FREE_MODEL_BUSY: AI_HTTP_STATUS_503: status=503 body=", "api")).toBe(
      "errFreeModelBusy",
    );
    expect(explainAiError("Error: AI_FREE_MODEL_BUSY: AI_HTTP_STATUS_502: rate limit", "api")).toBe(
      "errFreeModelBusy",
    );
    // 只認開頭：供應商把它抄進 body 不算數
    expect(
      explainAiError('AI_HTTP_STATUS_503: status=503 body={"message":"AI_FREE_MODEL_BUSY: 假的"}', "api"),
    ).toBe("errApiUpstream");
    // 無證據 429 後端不掛這個碼：保留原本的額度分流，不宣稱擁擠
    expect(explainAiError("AI_HTTP_STATUS_429: status=429 body=", "api")).toBe("errQuotaApi");
  });

  it("呼叫模型失敗才給保底人話；沒經過那個邊界的失敗維持原文", () => {
    // lib.rs 的 stream_via_transport 對沒有更精確碼的失敗掛上這個
    expect(explainAiError("AI_CALL_FAILED: error sending request")).toBe("errAiUnknown");
    expect(explainAiError("Error: AI_CALL_FAILED: stream closed")).toBe("errAiUnknown");
    // 讀卡、寫逐字稿、換桌等本機失敗不經過那個邊界，沒碼＝不冒充 AI 出問題
    expect(explainAiError("failed to write transcript: No space left on device")).toBe(null);
    expect(explainAiError("world not found")).toBe(null);
    // 碼只認開頭：供應商把它抄進 body 不算數
    expect(explainAiError('status=500 body={"message":"AI_CALL_FAILED: 假的"}')).toBe(null);
  });

  it("保底是最後一道：CLI 原話被包了一層，仍要保住原本更準的分類", () => {
    // 這一條是迴歸防線——保底若排在原文正則之前，玩家會從「額度用完」退化成「再試一次」
    expect(explainAiError("AI_CALL_FAILED: Rate limit exceeded", "claude")).toBe("errQuota");
    expect(explainAiError("AI_CALL_FAILED: usage limit reached", "codex")).toBe("errQuota");
    expect(explainAiError("AI_CALL_FAILED: not logged in", "grok")).toBe("errAuthCli");
    expect(explainAiError("AI_CALL_FAILED: 401 unauthorized", "api")).toBe("errAuthApi");
    // 內容規範的拒絕同理
    expect(explainAiError("AI_CALL_FAILED: 違反內容規範", "claude")).toBe("errRefused");
  });

  it("限流的原話依傳輸分流：API 不敢斷言額度用完，CLI 維持原本準確的說法", () => {
    const raw = "Rate limit exceeded: free-models-per-day";
    expect(explainAiError(raw, "api")).toBe("errQuotaApi");
    expect(explainAiError(raw, "claude")).toBe("errQuota");
    expect(explainAiError(raw, undefined)).toBe("errQuota");
  });
});

describe("redactAiErrorDetail", () => {
  const head = "AI_HTTP_STATUS_429: status=429 Too Many Requests body=";

  it("遮掉 user_id、其他診斷原樣保留", () => {
    const raw = `${head}{"error":{"message":"rate-limited","code":429},"user_id":"user_abc123"}`;
    expect(redactAiErrorDetail(raw)).toBe(
      `${head}{"error":{"message":"rate-limited","code":429},"user_id":"[redacted]"}`,
    );
  });

  it("鍵值之間夾空白或換行", () => {
    const raw = `${head}{\n  "user_id" :\n    "user_abc",\n  "code": 429\n}`;
    expect(redactAiErrorDetail(raw)).toBe(
      `${head}{\n  "user_id" :\n    "[redacted]",\n  "code": 429\n}`,
    );
  });

  it("同一段裡有多個 user_id", () => {
    const raw = `${head}{"user_id":"u1","error":{"metadata":{"user_id":"u2"}},"x":"y"}`;
    const shown = redactAiErrorDetail(raw);
    expect(shown).not.toMatch(/u1|u2/);
    expect(shown.match(/\[redacted\]/g)).toHaveLength(2);
    expect(shown).toContain('"x":"y"');
  });

  it("值裡含跳脫引號不會提早收尾", () => {
    const raw = `${head}{"user_id":"user_\\"quoted\\"_tail","code":429}`;
    expect(redactAiErrorDetail(raw)).toBe(`${head}{"user_id":"[redacted]","code":429}`);
  });

  it("值在中途被截斷：遮到截斷標記或字串結尾，截斷標記保留", () => {
    const cut = `${head}{"code":429,"user_id":"user_abcdef…[truncated]`;
    expect(redactAiErrorDetail(cut)).toBe(`${head}{"code":429,"user_id":"[redacted]"…[truncated]`);
    const end = `${head}{"code":429,"user_id":"user_abc`;
    expect(redactAiErrorDetail(end)).toBe(`${head}{"code":429,"user_id":"[redacted]"`);
    const slash = `${head}{"user_id":"user_ab\\`;
    expect(redactAiErrorDetail(slash)).toBe(`${head}{"user_id":"[redacted]"`);
  });

  it("非字串值也遮", () => {
    expect(redactAiErrorDetail('{"user_id": 98765, "code": 1}')).toBe(
      '{"user_id": "[redacted]", "code": 1}',
    );
  });

  it("沒有 user_id 時維持原文", () => {
    const raw = `${head}{"error":{"message":"rate-limited","provider_name":"Venice"},"userid":"keep"}`;
    expect(redactAiErrorDetail(raw)).toBe(raw);
  });
});

describe("explainAiError 生圖專屬碼", () => {
  it("免費 key 碼包著 402／403，要先於 HTTP 狀態認", () => {
    expect(
      explainAiError("AI_IMAGE_FREE_KEY: AI_HTTP_STATUS_402: status=402 body={}", "api"),
    ).toBe("errImageFreeKey");
    expect(explainAiError("Error: AI_IMAGE_FREE_KEY: AI_HTTP_STATUS_403: status=403")).toBe(
      "errImageFreeKey",
    );
  });

  it("沒有免費 key 碼的 402／403 照舊分流", () => {
    expect(explainAiError("AI_HTTP_STATUS_402: status=402 body={}", "api")).toBe("errQuotaApi");
    expect(explainAiError("AI_HTTP_STATUS_403: status=403 body={}", "api")).toBe(
      "errApiForbidden",
    );
  });

  it("body 裡抄到的生圖碼不翻盤", () => {
    expect(
      explainAiError("AI_HTTP_STATUS_402: status=402 body=AI_IMAGE_FREE_KEY: x", "api"),
    ).toBe("errQuotaApi");
  });

  it("格式不支援、解碼失敗、下載失敗各自分流", () => {
    expect(explainAiError("AI_IMAGE_UNSUPPORTED_FORMAT: svg")).toBe("errImageUnsupportedFormat");
    expect(explainAiError("AI_IMAGE_DECODE_FAILED: unexpected EOF")).toBe("errImageDecodeFailed");
    expect(explainAiError("AI_IMAGE_DOWNLOAD_FAILED: status=404 Not Found")).toBe(
      "errImageDownloadFailed",
    );
  });
});
