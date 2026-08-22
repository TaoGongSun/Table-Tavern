import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { ErrorNote } from "./atoms";

type UsageRow = {
  source: string;
  model: string;
  rounds: number;
  prompt_tokens: number;
  cached_tokens: number;
  output_tokens: number;
  hit_rate: number | null;
  observed_rounds: number;
  observed_prompt_tokens: number;
  cost_usd: number | null;
  cost_partial: boolean;
  saved_tokens: number;
  priced_tokens: number;
  saved_usd: number | null;
  saved_partial: boolean;
  unreported: number;
  in_use: boolean;
};
type UsageReport = {
  worlds: { id: string; name: string; rounds: number }[];
  rows: UsageRow[];
  total: UsageRow;
  ping: UsageRow;
  caches: { cache: string; rounds: number }[];
  events: number;
  latest: {
    ts: string;
    mode: string | null;
    cache: string | null;
    cache_reason: string | null;
    event: string | null;
    reason: string | null;
    reported: boolean;
  } | null;
};

// 快取結果字典（後端 usage_log.rs 模組頂註解那張表）：短名給統計、長句給最近一輪、燈號。
// 只有 missed 是紅的——它代表**證明得了照理該中**（算得出理論可中量、卻沒中滿）。
// 其餘都不是故障：zero 只是這輪沒省到；expired 只代表超過 app 的保守窗口（實測超時仍可能中）；
// 量不到與本來就沒得中根本不該打分，拿紅燈叫玩家去修不存在的問題最糟。
const CACHE_KEYS = {
  hit: ["usageCacheHit", "usageCacheHitWhy", "good"],
  missed: ["usageCacheMissed", "usageCacheMissedWhy", "bad"],
  partial: ["usageCachePartial", "usageCachePartialWhy", "warn"],
  zero: ["usageCacheZero", "usageCacheZeroWhy", "warn"],
  unknown: ["usageCacheUnknown", "usageCacheUnknownWhy", "idle"],
  "not-expected": ["usageCacheNotExpected", "usageCacheNotExpectedWhy", "idle"],
} as const;

// 後端 usage_report::chip_state 的前端版：統計與「最近一輪」要落在同一個格子，
// 否則同一輪在兩個地方會是不同顏色。
const FAULTY_REASONS = ["below-expected", "skipped"];
function chipState(cache: string, cacheReason: string | null) {
  const faulty = cacheReason !== null && FAULTY_REASONS.includes(cacheReason);
  if (faulty && (cache === "partial" || cache === "zero")) return "missed";
  return cache;
}
// 線事件不是一通呼叫，沒有快取結果，自己一組（一律紅：線被丟掉重來就是出過事）
const EVENT_KEYS = {
  "drop-lane": ["usageEventDropLane", "usageEventDropLaneWhy", "bad"],
} as const;
// 這通送出去的形狀。只在「最近一輪」那句話裡當一個詞出現，不進統計
const MODE_KEYS = {
  resume: "usageModeResume",
  shared: "usageModeShared",
  solo: "usageModeSolo",
  oneshot: "usageModeOneshot",
  ping: "usageModePing",
} as const;
// 沒中的原因；只有算得出理論可中量的續聊線給得出來
const CACHE_REASON_KEYS = {
  expired: "usageCacheReasonExpired",
  "below-expected": "usageCacheReasonBelowExpected",
  skipped: "usageCacheReasonSkipped",
} as const;
const REASON_KEYS = {
  "first-turn": "usageReasonFirstTurn",
  "pending-rewrite": "usageReasonPendingRewrite",
  "scene-changed": "usageReasonSceneChanged",
  "history-rewound": "usageReasonHistoryRewound",
  "history-edited": "usageReasonHistoryEdited",
  "reply-diverged": "usageReasonReplyDiverged",
  "resume-failed": "usageReasonResumeFailed",
  "rewrite-failed": "usageReasonRewriteFailed",
  "ping-truncate-failed": "usageReasonPingTruncateFailed",
} as const;

// 一行 →（短標 key, 長句 key, 燈號）。事件行走事件那組，其餘看快取結果
function latestEntry(latest: UsageReport["latest"]) {
  if (!latest) return undefined;
  if (latest.event) return EVENT_KEYS[latest.event as keyof typeof EVENT_KEYS];
  if (!latest.cache) return undefined;
  const state = chipState(latest.cache, latest.cache_reason);
  return CACHE_KEYS[state as keyof typeof CACHE_KEYS];
}

function tokens(value: number) {
  return value.toLocaleString();
}

// 細項的花費是 CLI 官方回報值，收合處的已省金額是後端拿它估的；兩邊都可能只湊到一部分（標「≥」）。
// 已省在建快取那幾輪還沒回本會是負的，對外收到 0 就好，別讓玩家看到「省下 -$0.003」
// 命中率：整條路一輪都量不到就出「—」，不出 0%——「量不到」和「沒中」是兩件事，
// 混講會讓玩家去修一個不存在的問題。只有部分輪次量得到時把分母說清楚。
function hitRate(row: UsageRow) {
  if (row.hit_rate === null) return <span className="usage-muted">—</span>;
  const rate = `${row.hit_rate.toFixed(1)}%`;
  if (row.observed_rounds >= row.rounds) return rate;
  return (
    <>
      {rate}
      <span className="usage-muted">
        {" "}
        {t("usageHitObserved", { observed: row.observed_rounds, rounds: row.rounds })}
      </span>
    </>
  );
}

function money(value: number | null, partial: boolean) {
  if (value === null) return "—";
  return `${partial ? "≥ " : ""}$${Math.max(0, value).toFixed(3)}`;
}

// 額度分頁（快取包 6）：讀後端彙總好的 prompt-cache.jsonl，以桌為主視圖、桌內按模型分行
export function UsageTab({ currentWorld }: { currentWorld: string }) {
  // null＝所有桌總計；"" ＝未標桌（加桌欄位之前的舊紀錄）
  const [scope, setScope] = useState<string | null>(currentWorld || null);
  const [report, setReport] = useState<UsageReport | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    let stale = false;
    invoke<UsageReport>("usage_report", { worldId: scope })
      .then((loaded) => {
        if (!stale) setReport(loaded);
      })
      .catch((reason) => {
        if (!stale) setError(String(reason));
      });
    return () => {
      stale = true;
    };
  }, [scope]);

  if (error) return <ErrorNote text={error} />;
  if (!report) return null;
  if (report.worlds.length === 0) return <p className="usage-empty">{t("usageEmpty")}</p>;

  // 「最近一輪怎麼樣」是某一桌的事；看總計時沒有「最近一輪」這回事，不出這句話
  const latest = scope === null ? null : report.latest;
  const entry = latestEntry(latest);
  const reasonKey = latest?.reason ? REASON_KEYS[latest.reason as keyof typeof REASON_KEYS] : undefined;
  // 沒中的原因（續聊線才給得出）與線重開的原因是兩件事，兩句都要出得來
  const cacheReasonKey = latest?.cache_reason
    ? CACHE_REASON_KEYS[latest.cache_reason as keyof typeof CACHE_REASON_KEYS]
    : undefined;
  // 這通送出去的形狀：舊紀錄推不出來就不寫，不拿「單發」冒充
  const modeKey = latest?.mode ? MODE_KEYS[latest.mode as keyof typeof MODE_KEYS] : undefined;
  // 非綠燈＝值得在收合狀態就看到；中了與不打分的留在細項裡就好
  const light = entry ? entry[2] : "idle";
  const alert = entry && latest && light === "bad";
  // 量不到要另外說一句，否則玩家看到空白的命中率會去修一個不存在的問題
  const blindNote = latest && !latest.reported ? ` — ${t("usageLatestBlind")}` : "";
  const bodyRows = [...report.rows];
  if (report.ping.rounds > 0) bodyRows.push({ ...report.ping, source: "", model: "ping" });
  // 第一眼只講省下多少（總用量與花費留在細項）。保溫也是真的花掉的錢，一併算進來
  const totals = {
    prompt_tokens: report.total.prompt_tokens + report.ping.prompt_tokens,
    cached_tokens: report.total.cached_tokens + report.ping.cached_tokens,
    observed_prompt_tokens:
      report.total.observed_prompt_tokens + report.ping.observed_prompt_tokens,
    saved_tokens: report.total.saved_tokens + report.ping.saved_tokens,
    priced_tokens: report.total.priced_tokens + report.ping.priced_tokens,
    saved_usd:
      report.total.saved_usd === null && report.ping.saved_usd === null
        ? null
        : (report.total.saved_usd ?? 0) + (report.ping.saved_usd ?? 0),
    saved_partial: report.total.saved_partial || report.ping.saved_partial,
  };
  // null＝一輪都量不到。此時不可畫進度條：0% 的條會被讀成「全額付費、一點都沒省」，
  // 而真相是「不知道」（Sol 驗收 2026-08-21）
  const hit =
    totals.observed_prompt_tokens === 0
      ? null
      : (totals.cached_tokens * 100) / totals.observed_prompt_tokens;
  const savedPct =
    totals.priced_tokens === 0 ? 0 : Math.max(0, (totals.saved_tokens * 100) / totals.priced_tokens);
  // 金額湊不齊（混了不回報用量或計價不明的來源）就不出現在第一眼，改由細項逐列交代
  const showSavedUsd = totals.saved_usd !== null && !totals.saved_partial;

  return (
    <div className="usage-tab">
      <label className="usage-scope">
        {t("usageScopeLabel")}
        <select
          value={scope ?? "*"}
          onChange={(event) => setScope(event.currentTarget.value === "*" ? null : event.currentTarget.value)}
        >
          <option value="*">{t("usageAllTables")}</option>
          {report.worlds.map((world) => (
            <option key={world.id} value={world.id}>
              {world.name || (world.id ? t("usageDeletedTable") : t("usageGenesis"))}
            </option>
          ))}
        </select>
      </label>

      {bodyRows.length === 0 && <p className="usage-empty">{t("usageEmpty")}</p>}

      {totals.prompt_tokens > 0 && (
        <>
          {totals.priced_tokens > 0 && (
            <p className="usage-headline">
              <span className="usage-headline-saved">
                {t("usageSavedHeadline", { pct: savedPct.toFixed(0) })}
              </span>
              {showSavedUsd && (
                <span className="usage-headline-cost">
                  {t("usageSavedAbout")} {money(totals.saved_usd, false)}
                </span>
              )}
            </p>
          )}
          {hit === null ? (
            <p className="usage-note">{t("usageCacheBlind")}</p>
          ) : (
            <div
              className="usage-bar"
              role="img"
              aria-label={`${t("usageBarHit")} ${hit.toFixed(0)}%`}
            >
              <span className="usage-bar-hit" style={{ width: `${hit}%` }} title={t("usageBarHit")}>
                {hit >= 18 && `${t("usageBarHit")} ${hit.toFixed(0)}%`}
              </span>
              <span className="usage-bar-full" style={{ width: `${100 - hit}%` }} title={t("usageBarFull")}>
                {hit <= 82 && `${t("usageBarFull")} ${(100 - hit).toFixed(0)}%`}
              </span>
            </div>
          )}
        </>
      )}

      {/* 狀況不正常時在收合狀態出聲，正常就只留在細項裡——同一句話不重複出現兩次 */}
      {alert && (
        <p className={`usage-latest usage-${light}`}>
          <span className="usage-dot" aria-hidden="true" />
          <strong>{t(entry[0])}</strong>
          {modeKey && <span className="usage-muted"> {t(modeKey)}</span>}
          {" — "}
          {t(entry[1])}
          {cacheReasonKey && `（${t(cacheReasonKey)}）`}
          {reasonKey && `（${t(reasonKey)}）`}
          {blindNote}
        </p>
      )}

      {bodyRows.length > 0 && (
      <details className="usage-details">
        <summary>{t("usageDetailsToggle")}</summary>

        {entry && latest && !alert && (
          <p className={`usage-latest usage-${light}`}>
            <span className="usage-dot" aria-hidden="true" />
            <strong>{t(entry[0])}</strong>
            {modeKey && <span className="usage-muted"> {t(modeKey)}</span>}
            {" — "}
            {t(entry[1])}
            {cacheReasonKey && `（${t(cacheReasonKey)}）`}
            {reasonKey && `（${t(reasonKey)}）`}
            {blindNote}
            <span className="usage-latest-ts">{latest.ts}</span>
          </p>
        )}

        <div className="usage-table-wrap">
          <table className="usage-table">
            <thead>
              <tr>
                <th>{t("usageModel")}</th>
                <th>{t("usageRounds")}</th>
                <th>{t("usageInputTokens")}</th>
                <th>{t("usageCached")}</th>
                <th>{t("usageHitRate")}</th>
                <th>{t("usageOutput")}</th>
                <th>{t("usageCost")}</th>
              </tr>
            </thead>
            <tbody>
              {bodyRows.map((row) => (
                <tr key={`${row.source}/${row.model}`} className={row.model === "ping" ? "usage-ping-row" : undefined}>
                  <th scope="row">
                    {row.model === "ping" ? (
                      t("usagePing")
                    ) : (
                      <>
                        <span className="usage-source">{row.source}</span> {row.model}
                        {row.in_use && <span className="usage-badge">{t("usageInUse")}</span>}
                      </>
                    )}
                  </th>
                  <td>{row.rounds}</td>
                  <td colSpan={row.unreported === row.rounds ? 4 : 1}>
                    {row.unreported === row.rounds ? (
                      <span className="usage-muted">{t("usageNoUsage")}</span>
                    ) : (
                      tokens(row.prompt_tokens)
                    )}
                  </td>
                  {row.unreported !== row.rounds && (
                    <>
                      <td>
                        {row.hit_rate === null ? (
                          <span className="usage-muted">—</span>
                        ) : (
                          tokens(row.cached_tokens)
                        )}
                      </td>
                      <td>{hitRate(row)}</td>
                      <td>{tokens(row.output_tokens)}</td>
                    </>
                  )}
                  <td>{money(row.cost_usd, row.cost_partial)}</td>
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr>
                <th scope="row">{t("usageTotal")}</th>
                <td>{report.total.rounds}</td>
                <td>{tokens(report.total.prompt_tokens)}</td>
                <td>
                  {report.total.hit_rate === null ? (
                    <span className="usage-muted">—</span>
                  ) : (
                    tokens(report.total.cached_tokens)
                  )}
                </td>
                <td>{hitRate(report.total)}</td>
                <td>{tokens(report.total.output_tokens)}</td>
                <td>{money(report.total.cost_usd, report.total.cost_partial)}</td>
              </tr>
            </tfoot>
          </table>
        </div>

        <p className="usage-diags">
          {report.caches.map((count) => {
            const label = CACHE_KEYS[count.cache as keyof typeof CACHE_KEYS];
            return label ? (
              <span key={count.cache} className={`usage-chip usage-${label[2]}`}>
                {t(label[0])} ×{count.rounds}
              </span>
            ) : null;
          })}
        </p>
        {/* 線事件不是一通呼叫，不進上面那排的分母，所以另起一行 */}
        {report.events > 0 && (
          <p className="usage-diags">
            <span className="usage-chip usage-bad">
              {t("usageEventDropLane")} ×{report.events}
            </span>
          </p>
        )}
        {/* 全盲時首頁那句已經講過，這裡只補「部分輪次量不到」的情況 */}
        {hit !== null && report.total.observed_rounds < report.total.rounds && (
          <p className="usage-note">{t("usageCacheBlind")}</p>
        )}
        <p className="usage-note">{t("usageCostNote")}</p>
      </details>
      )}
    </div>
  );
}
