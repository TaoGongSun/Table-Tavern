// 穩定免費的選模素材：帳號免費清單、RP 排行、七日榜、上游，12 小時內不重抓（同桌面版
// src-tauri/src/smart_free/mod.rs:33、:580 refresh_if_stale）。只存在記憶體，重新整理就重抓。
import {
  buildLineup,
  eligibleModels,
  requiredContext,
  RESERVED_OUTPUT_TOKENS,
  stableCandidates,
  type FreeModel,
} from "./catalog";
import { fnv1a } from "./failover";
import { fetchRoleplaySlugs, fetchUpstreams, fetchUserCatalog, fetchWeeklyIds, type ApiDeps } from "./openrouter-api";
import { NO_FREE_MODEL, type CallPlan } from "./smart-call";

const TTL_SECS = 12 * 3600;

interface Cached<T> {
  value: T;
  at: number;
}

export class FreePool {
  private account = "";
  /** 查詢世代：換帳號或登出時推進；await 回來對不上就整包丟掉，不寫快取。 */
  private generation = 0;
  private catalog: Cached<FreeModel[]> | null = null;
  private weekly: Cached<string[]> | null = null;
  private roleplay: Cached<string[]> | null = null;
  private upstreams: Cached<Map<string, string[]>> | null = null;

  constructor(private deps: ApiDeps) {}

  private stale<T>(entry: Cached<T> | null, now: number) {
    return !entry || now - entry.at >= TTL_SECS;
  }

  /** 登出或改用另一把金鑰：清掉帳號專屬快取，在途查詢回來一律丟棄。 */
  invalidate(): void {
    this.generation += 1;
    this.account = "";
    this.catalog = null;
    this.upstreams = null;
  }

  async refresh(apiKey: string, now: number): Promise<void> {
    const account = fnv1a([apiKey]);
    if (account !== this.account) {
      this.invalidate();
      this.account = account;
    }
    const generation = this.generation;
    const stillMine = () => this.generation === generation && this.account === account;
    const [catalog, weekly, roleplay] = await Promise.all([
      this.stale(this.catalog, now) ? fetchUserCatalog(this.deps, apiKey) : null,
      this.stale(this.weekly, now) ? fetchWeeklyIds(this.deps) : null,
      this.stale(this.roleplay, now) ? fetchRoleplaySlugs(this.deps) : null,
    ]);
    if (!stillMine()) return;
    if (catalog) this.catalog = { value: catalog, at: now };
    if (weekly) this.weekly = { value: weekly, at: now };
    if (roleplay) this.roleplay = { value: roleplay, at: now };
    const changed = catalog !== null || weekly !== null || roleplay !== null;
    if (!this.catalog || (!changed && !this.stale(this.upstreams, now))) return;
    // 全部穩定候選都抓（數量本來就少）；單支失敗保留舊值，沒有舊值＝上游未知
    const ids = this.candidates(RESERVED_OUTPUT_TOKENS, now).map(({ model }) => model.id);
    const previous = this.upstreams?.value ?? new Map<string, string[]>();
    const fetched = await Promise.all(ids.map((id) => fetchUpstreams(this.deps, apiKey, id)));
    if (!stillMine()) return;
    const next = new Map<string, string[]>();
    ids.forEach((id, index) => {
      const providers = fetched[index] ?? previous.get(id);
      if (providers) next.set(id, providers);
    });
    this.upstreams = { value: next, at: now };
  }

  private candidates(required: number, now: number) {
    return stableCandidates(this.catalog?.value ?? [], this.roleplay?.value ?? [], this.weekly?.value ?? [], required, now);
  }

  /** 這一句的選模素材；沒有可用免費模型時丟出 NO_FREE_MODEL。 */
  plan(contents: string[], now: number): CallPlan {
    const catalog = this.catalog?.value ?? [];
    const fits = eligibleModels(catalog, requiredContext(contents), now);
    const ranked = this.candidates(RESERVED_OUTPUT_TOKENS, now);
    const lineup = buildLineup(ranked, this.upstreams?.value ?? new Map()).entries.map(({ model }) => model.id);
    if (fits.length === 0 || lineup.length === 0) throw new Error(NO_FREE_MODEL);
    return {
      account: this.account,
      lineup,
      others: ranked.map(({ model }) => model.id).filter((id) => !lineup.includes(id)),
      fits: new Set(fits.map((model) => model.id)),
      names: new Map(catalog.map((model) => [model.id, model.name])),
    };
  }
}
