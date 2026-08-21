import { describe, expect, it } from "vitest";
import { explainAiError } from "./ai-error";

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
});
