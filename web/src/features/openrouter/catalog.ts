// 帳號可用的免費模型清單、排行與穩定名單。照桌面版 src-tauri/src/smart_free/select.rs
// （parse_model :58、eligible_models :193、stable_candidates :263、build_lineup :330）改寫。

export const RESERVED_OUTPUT_TOKENS = 4_096;
/** 穩定名單最多幾支：第 1 名是自動首選，其餘是換模順序。 */
export const LINEUP_SIZE = 4;
const EXPIRY_MARGIN_SECS = 24 * 60 * 60;
const STABLE_AGE_SECS = 7 * 24 * 60 * 60;

export interface FreeModel {
  id: string;
  /** 免費版與付費版共用的模型代號，用來對上角色扮演排行。 */
  canonicalSlug: string;
  name: string;
  created: number;
  contextLength: number;
  expirationAt: number | null;
  supportedParameters: string[];
}

type Json = Record<string, unknown>;

function dataArray(body: unknown): Json[] | null {
  const data = (body as Json | null)?.data;
  return Array.isArray(data) ? (data as Json[]) : null;
}

const trimmed = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null;

function isZeroPrice(value: unknown): boolean {
  const price = typeof value === "number" ? value : typeof value === "string" ? Number(value) : Number.NaN;
  return price === 0;
}

function modalities(entry: Json, field: string): unknown[] | null {
  const list = (entry.architecture as Json | undefined)?.[field];
  return Array.isArray(list) ? list : null;
}

function parseTimestamp(value: unknown): number | null {
  if (typeof value === "number" && Number.isInteger(value) && value >= 0) return value;
  if (typeof value !== "string") return null;
  const raw = value.trim();
  if (/^\d+$/.test(raw)) return Number(raw);
  const ms = Date.parse(raw.length === 10 ? `${raw}T00:00:00Z` : raw);
  return Number.isNaN(ms) ? null : Math.floor(ms / 1000);
}

function parseModel(entry: Json): FreeModel | null {
  const id = trimmed(entry.id);
  if (!id) return null;
  const pricing = entry.pricing as Json | undefined;
  if (!pricing) return null;
  // OpenRouter 只列實際計費的維度：缺鍵＝不計費
  if (!["prompt", "completion", "request"].every((key) => !(key in pricing) || isZeroPrice(pricing[key]))) {
    return null;
  }
  // 輸入要含文字；輸出必須只有文字
  const input = modalities(entry, "input_modalities");
  const output = modalities(entry, "output_modalities");
  if (!input?.includes("text") || !output || output.length === 0 || !output.every((m) => m === "text")) {
    return null;
  }
  let expirationAt: number | null = null;
  if (entry.expiration_date !== undefined && entry.expiration_date !== null) {
    expirationAt = parseTimestamp(entry.expiration_date);
    if (expirationAt === null) return null;
  }
  const params = Array.isArray(entry.supported_parameters)
    ? entry.supported_parameters.filter((value): value is string => typeof value === "string")
    : [];
  return {
    id,
    canonicalSlug: trimmed(entry.canonical_slug) ?? id,
    name: trimmed(entry.name) ?? id,
    created: parseTimestamp(entry.created) ?? 0,
    contextLength: typeof entry.context_length === "number" ? entry.context_length : 0,
    expirationAt,
    supportedParameters: params,
  };
}

/** 壞回應回 null（保留上一份）；空 data 是合法的「目前沒有模型」。 */
export function parseCatalog(body: unknown): FreeModel[] | null {
  const data = dataArray(body);
  return data ? data.map(parseModel).filter((model): model is FreeModel => model !== null) : null;
}

/** `?sort=top-weekly`：已排好順序，只取 id。 */
export function parseRankedIds(body: unknown): string[] | null {
  const data = dataArray(body);
  return data ? data.map((entry) => trimmed(entry.id)).filter((id): id is string => id !== null) : null;
}

/** `?category=roleplay`：排行走 base id，用 canonical_slug 才對得上免費版。 */
export function parseRankedSlugs(body: unknown): string[] | null {
  const data = dataArray(body);
  return data
    ? data.map((entry) => trimmed(entry.canonical_slug) ?? trimmed(entry.id)).filter((id): id is string => id !== null)
    : null;
}

/** `/models/{id}/endpoints` 的上游名稱（排序去重）。 */
export function parseUpstreams(body: unknown): string[] | null {
  const endpoints = ((body as Json | null)?.data as Json | undefined)?.endpoints;
  if (!Array.isArray(endpoints)) return null;
  const names = endpoints.map((endpoint) => trimmed((endpoint as Json).provider_name)).filter((n): n is string => !!n);
  return [...new Set(names)].sort();
}

/** 粗估 token：ASCII 每 4 字 1 個、其他字元每字 1 個（同桌面版 usage/log.rs:237）。 */
export function estimateTokens(text: string): number {
  let ascii = 0;
  let wide = 0;
  for (const ch of text) {
    if (ch.charCodeAt(0) < 128) ascii += 1;
    else wide += 1;
  }
  return Math.floor(ascii / 4) + wide;
}

export function requiredContext(contents: string[]): number {
  return contents.reduce((tokens, content) => tokens + estimateTokens(content) + 4, 2) + RESERVED_OUTPUT_TOKENS;
}

export function eligibleModels(catalog: FreeModel[], required: number, now: number): FreeModel[] {
  return catalog.filter(
    (model) =>
      model.contextLength >= required && (model.expirationAt === null || model.expirationAt > now + EXPIRY_MARGIN_SECS),
  );
}

function preferenceCompare(left: FreeModel, right: FreeModel): number {
  const day = (model: FreeModel) => Math.floor(model.created / 86_400);
  return (
    day(right) - day(left) ||
    right.contextLength - left.contextLength ||
    (left.id < right.id ? -1 : left.id > right.id ? 1 : 0)
  );
}

function isStable(model: FreeModel, now: number): boolean {
  return (
    !model.id.startsWith("stealth/") &&
    model.expirationAt === null &&
    model.created > 0 &&
    model.created <= now - STABLE_AGE_SECS
  );
}

export type StableSource = "roleplay" | "weekly" | "available";

export interface RankedModel {
  model: FreeModel;
  source: StableSource;
}

/** 全部合格的穩定候選：RP 排行 → 七日榜 → 其他合格穩定；限時／stealth／上架未滿 7 天不在其中。 */
export function stableCandidates(
  catalog: FreeModel[],
  roleplaySlugs: string[],
  weeklyIds: string[],
  required: number,
  now: number,
): RankedModel[] {
  const fits = (model: FreeModel) => model.contextLength >= required && isStable(model, now);
  const ranked: RankedModel[] = [];
  const push = (model: FreeModel | undefined, source: StableSource) => {
    if (model && !ranked.some((entry) => entry.model.id === model.id)) ranked.push({ model, source });
  };
  for (const slug of roleplaySlugs) push(catalog.find((m) => m.canonicalSlug === slug && fits(m)), "roleplay");
  for (const id of weeklyIds) push(catalog.find((m) => m.id === id && fits(m)), "weekly");
  for (const model of catalog.filter(fits).sort(preferenceCompare)) push(model, "available");
  return ranked;
}

export interface Lineup {
  entries: RankedModel[];
  /** false＝含上游未知的模型，或少於 4 支。 */
  diversified: boolean;
}

function sameUpstream(upstreams: Map<string, string[]>, left: FreeModel, right: FreeModel): boolean {
  const a = upstreams.get(left.id);
  const b = upstreams.get(right.id);
  return !!a && !!b && a.some((provider) => b.includes(provider));
}

/** 名次為主的貪婪重排：每個位置取剩下候選中名次最高、且與前一支不同上游者；找不到就停、少列。 */
export function buildLineup(ranked: RankedModel[], upstreams: Map<string, string[]>): Lineup {
  const remaining = [...ranked];
  const entries: RankedModel[] = [];
  if (remaining.length > 0) entries.push(remaining.shift()!);
  while (entries.length < LINEUP_SIZE) {
    const previous = entries[entries.length - 1]?.model;
    const index = remaining.findIndex(({ model }) => !previous || !sameUpstream(upstreams, previous, model));
    if (index < 0) break;
    entries.push(remaining.splice(index, 1)[0]);
  }
  const unknown = entries.some(({ model }) => !(upstreams.get(model.id)?.length ?? 0));
  return { entries, diversified: entries.length >= LINEUP_SIZE && !unknown };
}
