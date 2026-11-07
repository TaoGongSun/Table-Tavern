// 簡中碰不到 OpenRouter，清單不含 api；矽基流動只服務中國，其他語系不含 siliconflow。
// CLI 是進階選項，各語系都列。
import type { Lang } from "../../i18n";

const ROWS = ["api", "siliconflow", "claude", "codex", "agy", "grok"] as const;

export type TransportRowId = (typeof ROWS)[number];

/** 這個語系預設的 API 供應商。入門卡與連線清單都看這裡。 */
export function defaultApiProvider(lang: Lang): "siliconflow" | "openrouter" {
  return lang === "zh-CN" ? "siliconflow" : "openrouter";
}

/** 這個語系的連線列，依顯示順序。已選中但不在清單裡的列由畫面另補。 */
export function transportRowsFor(lang: Lang): TransportRowId[] {
  const hide = defaultApiProvider(lang) === "siliconflow" ? "api" : "siliconflow";
  return ROWS.filter((id) => id !== hide);
}
