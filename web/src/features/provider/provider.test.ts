import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "../../i18n";
import { explainError, redactKeys } from "../chat/error-text";
import { classifyFailure, dailyEvidence, failureFromHttp } from "../openrouter/api-failure";
import { FailoverRuntime } from "../openrouter/failover";
import { MemoryStorage } from "../openrouter/memory-storage";
import { runSmartCall } from "../openrouter/smart-call";
import { FixedPool } from "./fixed-pool";
import { effectiveProvider, providersFor } from "./provider-choice";
import { CN_PROVIDERS, isPaidId, priceTag } from "./providers";

afterEach(() => setLang("zh-TW"));

const modelsFetch = (ids: string[] | null) =>
  (async () => (ids ? new Response(JSON.stringify({ data: ids.map((id) => ({ id })) })) : new Response("", { status: 500 }))) as typeof fetch;

const pool = (ids: string[] | null, chosen: string | null = null) =>
  new FixedPool(CN_PROVIDERS.modelscope, { fetch: modelsFetch(ids), apiBase: "http://fake/ms/v1" }, chosen);

describe("provider by language", () => {
  it("lists only ModelScope and SiliconFlow in Simplified Chinese, only OpenRouter elsewhere", () => {
    expect(providersFor("zh-CN")).toEqual(["modelscope", "siliconflow"]);
    for (const lang of ["zh-TW", "en", "ja"] as const) expect(providersFor(lang)).toEqual(["openrouter"]);
  });

  it("never lands on OpenRouter in Simplified Chinese, even with an OpenRouter key stored", () => {
    const storage = new MemoryStorage();
    storage.setItem("tt-web:openrouter-key", "sk-or-v1-old");
    expect(effectiveProvider("zh-CN", null, storage)).toBe("modelscope");
    expect(effectiveProvider("zh-CN", "openrouter", storage)).toBe("modelscope");
    expect(effectiveProvider("zh-TW", null, storage)).toBe("openrouter");
    expect(effectiveProvider("en", "siliconflow", storage)).toBe("openrouter");
    storage.setItem("tt-web:key:siliconflow", "sk-sf");
    expect(effectiveProvider("zh-CN", "openrouter", storage)).toBe("siliconflow");
    expect(effectiveProvider("zh-CN", "modelscope", storage)).toBe("modelscope");
  });
});
describe("never auto-picks a paid model", () => {
  it("keeps Pro/, LoRA/ and `:` provider-suffixed ids out of every hard-coded lineup", () => {
    for (const provider of Object.values(CN_PROVIDERS)) {
      expect(provider.lineup.filter((model) => isPaidId(model.id))).toEqual([]);
    }
  });

  it("uses /models only to drop retired lineup entries, never to add models", async () => {
    const live = pool(["Qwen/Qwen3.5-397B-A17B", "MiniMax/MiniMax-M3", "Pro/some/paid", "other/big-model"]);
    await live.refresh("ms-token", 0);
    const plan = live.plan(() => true);
    expect(plan.lineup).toEqual(["Qwen/Qwen3.5-397B-A17B", "MiniMax/MiniMax-M3"]);
    expect(plan.others).toEqual([]);
  });

  it("falls back to the whole lineup when /models cannot be read", async () => {
    const offline = pool(null);
    await offline.refresh("ms-token", 0);
    expect(offline.plan(() => true).lineup).toEqual(CN_PROVIDERS.modelscope.lineup.map((model) => model.id));
  });

  it("sends a hand-picked model alone, with a conservative context budget", async () => {
    const picked = pool(["a/b"], "other/big-model");
    await picked.refresh("ms-token", 0);
    expect(picked.plan(() => true).lineup).toEqual(["other/big-model"]);
    expect(picked.contextLength("other/big-model")).toBe(32_768);
    expect(picked.contextLength("ZhipuAI/GLM-5.2")).toBe(131_072);
  });

  it("does not switch a hand-picked model to anything else when it keeps failing", async () => {
    const picked = pool(null, "other/big-model");
    const runtime = new FailoverRuntime();
    const sent: string[] = [];
    const fail = failureFromHttp(503, new Headers(), "");
    for (let turn = 0; turn < 3; turn += 1) {
      await runSmartCall(picked.plan(() => true), runtime, {
        signal: new AbortController().signal,
        now: () => 1,
        dailyRemaining: async () => null,
        rateLimit: "per-model",
        send: async (model) => (sent.push(model), { kind: "failed", failure: fail }),
      });
    }
    expect(new Set(sent)).toEqual(new Set(["other/big-model"]));
  });

  it("labels SiliconFlow models by what is known and leaves ModelScope (Magic Points only) unlabelled", () => {
    expect(priceTag("siliconflow", "Qwen/Qwen3-8B")).toBe("free");
    expect(priceTag("siliconflow", "Pro/Qwen/Qwen3-8B")).toBe("paid");
    expect(priceTag("siliconflow", "LoRA/Qwen/Qwen2.5-7B-Instruct")).toBe("paid");
    expect(priceTag("siliconflow", "deepseek-ai/DeepSeek-V3")).toBe("maybe-paid");
    expect(priceTag("modelscope", "other/big-model")).toBeNull();
  });
});

describe("429 by provider", () => {
  const headers = new Headers({ "x-ratelimit-remaining": "0" });
  const limited = failureFromHttp(429, headers, '{"error":{"message":"rate limited"}}');

  it("keeps OpenRouter's account-level reading of remaining=0", () => {
    expect(classifyFailure(limited, { kind: "not-queried" })).toBe("account");
    expect(classifyFailure(limited, { kind: "not-queried" }, "openrouter")).toBe("account");
  });

  it("treats a Chinese provider's 429 as a per-model limit that counts toward switching and never as daily exhaustion", () => {
    expect(classifyFailure(limited, { kind: "not-queried" }, "per-model")).toBe("unknown-rate-limit");
    expect(dailyEvidence(limited, { kind: "remaining", remaining: 0 }, "per-model")).toBe(false);
  });
});

describe("Chinese provider errors", () => {
  it("reads SiliconFlow's root-level error body", () => {
    const failure = failureFromHttp(401, new Headers(), '{"code":30014,"message":"Token is invalid.","data":null}');
    expect(failure.detail).toMatchObject({ code: 30014, message: "Token is invalid." });
  });

  it("explains 401/403/429 per provider, hedging on causes it cannot see", () => {
    setLang("zh-CN");
    const ms = { provider: "modelscope" as const, apiKey: "ms-secret-token" };
    expect(explainError("AI_HTTP_STATUS_401: body=x", ms)).toContain("阿里云");
    const forbidden = explainError("AI_HTTP_STATUS_403: status=403 body=ms-secret-token denied", ms);
    expect(forbidden).toContain("本站不会改用付费模型");
    expect(forbidden).toContain("可能是今天的魔粒不足");
    expect(forbidden).not.toContain("ms-secret-token");
    expect(explainError("AI_HTTP_STATUS_429: x", { provider: "siliconflow" })).toContain("也可能是额度不足");
    expect(explainError("AI_FREE_ALL_BUSY: x", ms)).toContain("魔粒");
    expect(explainError("AI_HTTP_STATUS_429: x")).toContain("OpenRouter");
  });

  it("masks generic sk- keys as well as OpenRouter keys", () => {
    expect(redactKeys("key sk-abcdefghijklmnopqrstuvwxyz and sk-or-v1-abc")).toBe("key sk-… and sk-or-…");
  });
});
