// 桌面版版本與下載連結，執行時從 GitHub Releases API 抓（計畫 4.5）。github.com 的
// releases/latest/download 會 302 且沒有 CORS 標頭，瀏覽器抓不到，所以打 api.github.com
// （CORS *、未帶金鑰每 IP 每小時 60 次）。只有 404 才算「還沒有正式版」；限流、伺服器錯誤、斷線、
// 回應讀不懂都只是「暫時查不到」，兩種都退回 releases 頁。
import { GITHUB_API, RELEASES_PAGE, REPO } from "../../shared/endpoints/origins";

export interface ReleaseInfo {
  /** loading＝還在查；ok＝查到正式版；none＝API 明確說沒有（404）；error＝查不到（不代表沒有）。 */
  status: "loading" | "ok" | "none" | "error";
  /** 只有 ok 才有；其餘下載鈕一律退 releases 頁。 */
  version: string | null;
  pageUrl: string;
  windows: string | null;
  mac: string | null;
}

const fallback = (status: ReleaseInfo["status"]): ReleaseInfo => ({ status, version: null, pageUrl: RELEASES_PAGE, windows: null, mac: null });

export const LOADING_RELEASE = fallback("loading");
export const NO_RELEASE = fallback("none");
export const RELEASE_UNAVAILABLE = fallback("error");

/** 資產檔名由發版 workflow 固定（.ai/plans/desktop-update-detect.md:82）。 */
export function parseRelease(body: unknown): ReleaseInfo {
  if (!body || typeof body !== "object") return RELEASE_UNAVAILABLE;
  const record = body as Record<string, unknown>;
  const tag = typeof record.tag_name === "string" && record.tag_name.trim() !== "" ? record.tag_name : null;
  if (tag === null) return RELEASE_UNAVAILABLE;
  const page = typeof record.html_url === "string" && record.html_url.startsWith("https://github.com/")
    ? record.html_url
    : RELEASES_PAGE;
  const assets = Array.isArray(record.assets) ? (record.assets as Record<string, unknown>[]) : [];
  const find = (suffix: string) => {
    const asset = assets.find((item) => typeof item.name === "string" && item.name.endsWith(suffix));
    const url = asset?.browser_download_url;
    return typeof url === "string" && url.startsWith("https://github.com/") ? url : null;
  };
  return {
    status: "ok",
    version: tag.replace(/^v/, ""),
    pageUrl: page,
    windows: find("_x64-setup.exe"),
    mac: find("_aarch64.dmg"),
  };
}

export async function fetchLatestRelease(fetchImpl: typeof fetch): Promise<ReleaseInfo> {
  try {
    const response = await fetchImpl(`${GITHUB_API}/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
      signal: AbortSignal.timeout(15_000),
    });
    if (response.status === 404) return NO_RELEASE;
    return response.ok ? parseRelease(await response.json()) : RELEASE_UNAVAILABLE;
  } catch {
    return RELEASE_UNAVAILABLE;
  }
}
