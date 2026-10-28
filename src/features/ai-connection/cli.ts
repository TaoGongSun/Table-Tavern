// 本機 CLI 偵測與顯示名：設定頁與生圖對話框共用。
// 純清單與解析規則在 model-catalog.ts，這裡是會碰 invoke 的那一半。
import { invoke } from "@tauri-apps/api/core";
import { CLI_CHANGED_EVENT } from "../../shared/ui/cli-update";

export interface CliInfo {
  id: string;
  path: string;
  version: string;
  /** 已裝版本比 app 測過的舊時＝測過的版本（claude、codex、grok） */
  tested?: string;
  /** updating＝更新或啟動復原中；stuck＝更新程序停不下來、請重新啟動 app；broken＝還原失敗、請重新安裝 */
  update_state?: "updating" | "stuck" | "broken";
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

// 更新鈕換了 CLI 版本：下次偵測重探
if (typeof window !== "undefined") {
  window.addEventListener(CLI_CHANGED_EVENT, () => {
    cliCache = null;
  });
}

export async function detectClis(force = false): Promise<CliInfo[]> {
  if (!force && cliCache) return cliCache;
  const detected = await invoke<CliInfo[]>("detect_clis");
  // 帶更新狀態的結果是一時的（更新、啟動復原會結束）：不快取，下次開設定頁一律重探
  cliCache = detected.some((cli) => cli.update_state) ? null : detected;
  return detected;
}

/** 設定頁「有測試過的新版本」提示過哪一版（實際顯示時才寫）。 */
export function cliTestedPromptedKey(id: string) {
  return `cli_tested_prompted:${id}`;
}

export function cliConnectedKey(id: string) {
  return `cli_connected:${id}`;
}
