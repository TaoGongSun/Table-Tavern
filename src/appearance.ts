import { AppConfig } from "./backend-contracts";

// 外觀與贊助解鎖共用的常數。
export const KOFI_URL = "https://ko-fi.com/s/027754730c";

// 主題清單：free 兩套隨點隨存；sponsor 五套未解鎖只能試看（關設定視窗即復原）
export const FREE_THEMES = ["dark", "light"] as const;
export const SPONSOR_THEMES = ["parchment", "herbal", "candlelight", "port", "seamist"] as const;
export const ALL_THEMES = [...FREE_THEMES, ...SPONSOR_THEMES] as const;
export type ThemeId = (typeof ALL_THEMES)[number];

// 主題不跟系統走（2026-07-25 使用者拍板）：config.preferences.theme 寫在 <html data-theme>，預設深色
export function resolveTheme(config: AppConfig | null | undefined, sponsorUnlocked: boolean): ThemeId {
  const theme = String(config?.preferences["theme"] ?? "dark");
  if (!ALL_THEMES.includes(theme as ThemeId)) return "dark";
  if (
    (SPONSOR_THEMES as readonly string[]).includes(theme) &&
    !sponsorUnlocked
  ) {
    return "dark";
  }
  return theme as ThemeId;
}

// 文字大小偏好：存 config.preferences.text_size，套在 html 根字級（rem 版面跟著縮放）。
// 五檔偏小取向（大螢幕看長文要小字）；預設 l（16px）＝原本的視覺大小
export const TEXT_SIZE_PX: Record<string, string> = {
  xs: "10px",
  s: "12px",
  m: "14px",
  l: "16px",
  xl: "18px",
};
export const TEXT_SIZE_DEFAULT = "l";
