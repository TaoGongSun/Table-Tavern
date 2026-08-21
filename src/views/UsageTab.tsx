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
  diags: { diag: string; rounds: number }[];
  latest: { ts: string; diag: string; reason: string | null } | null;
};

// 診斷標籤字典（後端 usage_log.rs 模組頂註解那張表）：短名給統計、長句給最近一輪。
// 燈號純規則：正常綠、暖機／過期黃、該中沒中紅、單發模式不打分。
const DIAG_KEYS = {
  ok: ["usageDiagOk", "usageWhyOk", "good"],
  ping: ["usageDiagPing", "usageWhyPing", "good"],
  warmup: ["usageDiagWarmup", "usageWhyWarmup", "warn"],
  expired: ["usageDiagExpired", "usageWhyExpired", "warn"],
  single: ["usageDiagSingle", "usageWhySingle", "idle"],
  "prefix-broken": ["usageDiagPrefixBroken", "usageWhyPrefixBroken", "bad"],
  "cache-skipped": ["usageDiagCacheSkipped", "usageWhyCacheSkipped", "bad"],
  "no-cache": ["usageDiagNoCache", "usageWhyNoCache", "bad"],
  "drop-lane": ["usageDiagDropLane", "usageWhyDropLane", "bad"],
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

function diagEntry(diag: string) {
  return DIAG_KEYS[diag as keyof typeof DIAG_KEYS];
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
  const entry = latest ? diagEntry(latest.diag) : undefined;
  const reasonKey = latest?.reason ? REASON_KEYS[latest.reason as keyof typeof REASON_KEYS] : undefined;
  // 非綠燈＝值得在收合狀態就看到；正常與單發留在細項裡就好
  const alert = entry && latest && entry[2] !== "good" && entry[2] !== "idle";
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
  const hit =
    totals.observed_prompt_tokens === 0
      ? 0
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
        </>
      )}

      {/* 狀況不正常時在收合狀態出聲，正常就只留在細項裡——同一句話不重複出現兩次 */}
      {alert && (
        <p className={`usage-latest usage-${entry[2]}`}>
          <span className="usage-dot" aria-hidden="true" />
          <strong>{t(entry[0])}</strong>
          {" — "}
          {t(entry[1])}
          {reasonKey && `（${t(reasonKey)}）`}
        </p>
      )}

      {bodyRows.length > 0 && (
      <details className="usage-details">
        <summary>{t("usageDetailsToggle")}</summary>

        {entry && latest && !alert && (
          <p className={`usage-latest usage-${entry[2]}`}>
            <span className="usage-dot" aria-hidden="true" />
            <strong>{t(entry[0])}</strong>
            {" — "}
            {t(entry[1])}
            {reasonKey && `（${t(reasonKey)}）`}
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
          {report.diags.map((count) => {
            const label = diagEntry(count.diag);
            return label ? (
              <span key={count.diag} className={`usage-chip usage-${label[2]}`}>
                {t(label[0])} ×{count.rounds}
              </span>
            ) : null;
          })}
        </p>
        {report.total.observed_rounds < report.total.rounds && (
          <p className="usage-note">{t("usageCacheBlind")}</p>
        )}
        <p className="usage-note">{t("usageCostNote")}</p>
      </details>
      )}
    </div>
  );
}
