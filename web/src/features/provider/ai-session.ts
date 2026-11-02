// 聊天只認這個介面：金鑰從哪來、模型名單從哪來、額度怎麼查、要帶什麼請求頭，由各供應商的 session 給。
import type { QuotaEvent, QuotaState } from "../funnel/quota";
import type { RateLimitRule } from "../openrouter/api-failure";
import type { FailoverRuntime } from "../openrouter/failover";
import type { ApiDeps, FreeDaily } from "../openrouter/openrouter-api";
import type { CallPlan } from "../openrouter/smart-call";
import type { ProviderId } from "./providers";

/** 選模素材：OpenRouter 的 FreePool、中國供應商的 FixedPool。 */
export interface ModelPool {
  refresh(apiKey: string, now: number): Promise<void>;
  plan(holds: (modelId: string) => boolean, now: number): CallPlan;
  contextLength(modelId: string): number;
  tokenizer(modelId: string): string | null;
}

export interface AiSession {
  provider: ProviderId;
  apiKey: string | null;
  quota: QuotaState;
  deps: ApiDeps;
  headers: Record<string, string>;
  rateLimit: RateLimitRule;
  pool: ModelPool;
  runtime: FailoverRuntime;
  /** 送出前查今日免費次數；只有 OpenRouter 有 /key，其他家回 null。 */
  dailyCheck: (apiKey: string) => Promise<FreeDaily | null>;
  refreshQuota: () => Promise<void>;
  quotaEvent: (event: QuotaEvent) => void;
  logout: () => void;
}
