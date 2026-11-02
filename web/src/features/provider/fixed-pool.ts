// 中國供應商的選模：自動＝寫死名單 ∩ /models 回得到的 ID（/models 只用來刪下架的，不用來補，那裡有付費模型）；
// 手選＝只有玩家選的那一支（失敗就報錯，不換別支）。
import { fnv1a } from "../openrouter/failover";
import type { ApiDeps } from "../openrouter/openrouter-api";
import { NO_FREE_MODEL, type CallPlan } from "../openrouter/smart-call";
import type { ModelPool } from "./ai-session";
import { isPaidId, UNKNOWN_CONTEXT, type CnProvider } from "./providers";

const TTL_SECS = 12 * 3600;

/** `/models` 的模型 ID（帶 `:` 外部計費後綴的不列）；抓不到回 null。401 回 "unauthorized"。 */
export async function fetchModelIds(deps: ApiDeps, apiKey: string | null): Promise<string[] | "unauthorized" | null> {
  try {
    const response = await deps.fetch(`${deps.apiBase}/models`, {
      headers: apiKey ? { Authorization: `Bearer ${apiKey}` } : {},
      signal: AbortSignal.timeout(15_000),
    });
    if (response.status === 401) return "unauthorized";
    if (!response.ok) return null;
    const data = ((await response.json()) as { data?: unknown } | null)?.data;
    if (!Array.isArray(data)) return null;
    return data
      .map((entry) => (entry as { id?: unknown } | null)?.id)
      .filter((id): id is string => typeof id === "string" && id !== "" && !id.includes(":"));
  } catch {
    return null;
  }
}

export class FixedPool implements ModelPool {
  private live: { ids: Set<string>; at: number } | null = null;
  private account = "";

  constructor(
    private provider: CnProvider,
    private deps: ApiDeps,
    /** 玩家手選的模型；null＝自動 */
    private chosen: string | null,
  ) {}

  async refresh(apiKey: string, now: number): Promise<void> {
    this.account = fnv1a([apiKey]);
    if (this.chosen !== null || (this.live && now - this.live.at < TTL_SECS)) return;
    const ids = await fetchModelIds(this.deps, apiKey);
    if (Array.isArray(ids)) this.live = { ids: new Set(ids), at: now };
  }

  /** 這一句可用的名單：手選就只有那一支；自動照寫死順序，抓得到 /models 就刪掉下架的。 */
  lineup(): string[] {
    if (this.chosen !== null) return [this.chosen];
    const ids = this.provider.lineup.map((model) => model.id).filter((id) => !isPaidId(id));
    const live = this.live;
    return live ? ids.filter((id) => live.ids.has(id)) : ids;
  }

  plan(holds: (modelId: string) => boolean): CallPlan {
    const lineup = this.lineup();
    if (lineup.length === 0) throw new Error(NO_FREE_MODEL);
    return { account: this.account, lineup, others: [], fits: holds, names: new Map() };
  }

  contextLength(modelId: string): number {
    return this.provider.lineup.find((model) => model.id === modelId)?.contextLength ?? UNKNOWN_CONTEXT;
  }

  tokenizer(): string | null {
    return null;
  }
}
