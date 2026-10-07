// 桌面版版本與下載連結，執行時從 GitHub Releases API 抓（計畫 4.5）。github.com 的
// releases/latest/download 會 302 且沒有 CORS 標頭，瀏覽器抓不到，所以打 api.github.com
// （CORS *、未帶金鑰每 IP 每小時 60 次）。沒有正式版（404）或抓不到就退回 releases 頁。
import { GITHUB_API, RELEASES_PAGE, REPO } from "../../shared/endpoints/origins";

export interface ReleaseInfo {
  /** null＝沒有正式版或抓不到；下載鈕一律退 releases 頁。 */
  version: string | null;
  pageUrl: string;
  windows: string | null;
  mac: string | null;
}

export const FALLBACK_RELEASE: ReleaseInfo = { version: null, pageUrl: RELEASES_PAGE, windows: null, mac: null };

/** 資產檔名由發版 workflow 固定（.ai/plans/desktop-update-detect.md:82）。 */
export function parseRelease(body: unknown): ReleaseInfo {
  if (!body || typeof body !== "object") return FALLBACK_RELEASE;
  const record = body as Record<string, unknown>;
  const tag = typeof record.tag_name === "string" ? record.tag_name : null;
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
    version: tag ? tag.replace(/^v/, "") : null,
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
    return response.ok ? parseRelease(await response.json()) : FALLBACK_RELEASE;
  } catch {
    return FALLBACK_RELEASE;
  }
}
