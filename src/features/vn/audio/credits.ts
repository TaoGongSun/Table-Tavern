// 音訊素材署名的對照。完結畫面用精簡行，音樂頁用逐筆授權標籤。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { AudioEntry, AudioView } from "../../../shared/contracts/backend-contracts";

const AUTHOR_CAP = 8;

/** CC 授權原文對成短標；認不得的留原文（去掉頭尾空白）。 */
export function licenseLabel(license: string): string {
  const text = license.trim();
  if (text === "https://creativecommons.org/licenses/by/4.0/") return "CC BY 4.0";
  if (text === "Attribution") return "CC BY";
  if (text === "Creative Commons 0") return "CC0";
  return text;
}

/** 來源連結的主機名。解析失敗回空字串，呼叫端就不要顯示。 */
export function sourceHost(url: string): string {
  try {
    return new URL(url.trim()).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

export interface CreditLine {
  kind: "bgm" | "sfx";
  authors: string[];
  hosts: string[];
  licenses: string[];
  /** 名單上沒寫出的作者人數。超過 8 位才大於 0。 */
  more: number;
}

function unique(values: string[]): string[] {
  return [...new Set(values.filter(Boolean))];
}

/** bgm、sfx 各一行。沒有筆的組不出；作者、主機、授權各自去重並保住第一次出現的順序。 */
export function creditLines(entries: readonly AudioEntry[]): CreditLine[] {
  const lines: CreditLine[] = [];
  for (const kind of ["bgm", "sfx"] as const) {
    const rows = entries.filter((entry) => entry.kind === kind);
    if (rows.length === 0) continue;
    const authors = unique(rows.map((row) => row.author.trim()));
    lines.push({
      kind,
      authors: authors.slice(0, AUTHOR_CAP),
      hosts: unique(rows.map((row) => sourceHost(row.source_url))),
      licenses: unique(rows.map((row) => licenseLabel(row.license))),
      more: Math.max(0, authors.length - AUTHOR_CAP),
    });
  }
  return lines;
}

/** 掛載時讀一次。失敗或沒有目錄都當沒有筆，不往上拋。 */
export function useCreditEntries(worldId: string): AudioEntry[] {
  const [entries, setEntries] = useState<AudioEntry[]>([]);
  useEffect(() => {
    let live = true;
    invoke<AudioView | null>("vn_audio_read", { worldId })
      .then((view) => {
        if (!live) return;
        setEntries(Array.isArray(view?.entries) ? view.entries : []);
      })
      .catch(() => {
        if (live) setEntries([]);
      });
    return () => {
      live = false;
    };
  }, [worldId]);
  return entries;
}
