// 本機 CLI 偵測與顯示名：設定頁與生圖對話框共用。
// 純清單與解析規則在 model-catalog.ts，這裡是會碰 invoke 的那一半。
import { invoke } from "@tauri-apps/api/core";

export interface CliInfo {
  id: string;
  path: string;
  version: string;
}

export const CLI_LABELS: Record<string, string> = {
  claude: "Claude Code CLI",
  codex: "Codex CLI",
  // 引擎是 Google Antigravity CLI，但一般使用者只認識 Gemini 這個名字（2026-07-25 拍板）
  agy: "Gemini CLI",
  grok: "Grok CLI",
};

// 偵測結果跨畫面／跨設定頁開關快取：null＝本次啟動還沒偵測過。
// 設定頁重開時直接吃上次結果，不重探也不再讓使用者看一次「偵測中」。
let cliCache: CliInfo[] | null = null;

export const cachedClis = () => cliCache;

export async function detectClis(force = false): Promise<CliInfo[]> {
  if (!force && cliCache) return cliCache;
  const detected = await invoke<CliInfo[]>("detect_clis");
  cliCache = detected;
  return detected;
}

export function cliConnectedKey(id: string) {
  return `cli_connected:${id}`;
}
