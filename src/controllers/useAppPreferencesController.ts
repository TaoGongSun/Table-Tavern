import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { resolveTheme, TEXT_SIZE_DEFAULT, TEXT_SIZE_PX } from "../features/settings/appearance";
import { updateConfig } from "../features/settings/update-config";
import { AppConfig } from "../shared/contracts/backend-contracts";
import { cliConnectedKey } from "../features/ai-connection/cli";
import { normalizeLang, setLang } from "../i18n";

const CLI_IDS = ["claude", "codex", "agy", "grok"] as const;

// 認證失敗的下一步依傳輸而異：API 是換金鑰、CLI 是重新登入。
// 設定還沒載入就回 undefined——猜錯會把人指去錯的地方，中性文案還比較誠實。
const transportOf = (config: AppConfig | null) =>
  config ? String(config.preferences["transport"] ?? "api") : undefined;

interface AppPreferencesControllerOptions {
  onError: (message: string) => void;
}

export function useAppPreferencesController({ onError }: AppPreferencesControllerOptions) {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [sponsorUnlocked, setSponsorUnlocked] = useState(false);

  // 語系跟著 config 走；render 前同步進 i18n 模組，之後子樹的 t() 都拿到正確語言
  const language = normalizeLang(config?.preferences["language"]);
  setLang(language);

  // 串流期間 config 可能已被設定頁改寫，走 ref 取最新值，避免舊閉包蓋掉剛存的設定
  const currentConfigRef = useRef(config);
  currentConfigRef.current = config;
  // updateConfig 依呼叫順序回。較早那份快照還沒有後一次點的鍵，直接套用會把樂觀值蓋回去。
  // 語言、文字大小、聊天裡記下的 CLI 連線共用這個序號，只套用最後一次偏好請求的回傳。
  const preferenceWrite = useRef(0);

  // 外觀類偏好（語言、文字大小）：先改本地讓畫面立刻跟上，再寫回。不設儲存鈕
  async function changePreference(key: string, value: unknown) {
    const current = currentConfigRef.current;
    if (!current) return;
    const updated = { ...current, preferences: { ...current.preferences, [key]: value } };
    currentConfigRef.current = updated;
    setConfig(updated);
    const ticket = ++preferenceWrite.current;
    try {
      const saved = await updateConfig({ preferences: { [key]: value } });
      if (ticket !== preferenceWrite.current) return;
      currentConfigRef.current = saved;
      setConfig(saved);
    } catch (reason) {
      onError(String(reason));
    }
  }

  const markCliConnectedFromChat = useCallback(async () => {
    const current = currentConfigRef.current;
    if (!current) return;
    const transport = current.preferences["transport"];
    if (
      !CLI_IDS.includes(transport as (typeof CLI_IDS)[number]) ||
      current.preferences[cliConnectedKey(String(transport))] === true
    ) {
      return;
    }
    const key = cliConnectedKey(String(transport));
    const ticket = ++preferenceWrite.current;
    try {
      const saved = await updateConfig({ preferences: { [key]: true } });
      if (ticket !== preferenceWrite.current) return;
      currentConfigRef.current = saved;
      setConfig(saved);
    } catch (reason) {
      onError(String(reason));
    }
  }, [onError]);

  const textSize = String(config?.preferences["text_size"] ?? TEXT_SIZE_DEFAULT);
  useEffect(() => {
    document.documentElement.style.fontSize =
      TEXT_SIZE_PX[textSize] ?? TEXT_SIZE_PX[TEXT_SIZE_DEFAULT];
  }, [textSize]);

  useEffect(() => {
    void invoke<boolean>("sponsor_status")
      .then(setSponsorUnlocked)
      .catch(() => {});
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = resolveTheme(config, sponsorUnlocked);
  }, [config, sponsorUnlocked]);

  // 中日韓共用同一批 Unicode 碼位但字形不同，斷行規則也各異，靠 lang 屬性讓 webview 挑對字形
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  return {
    config,
    setConfig,
    sponsorUnlocked,
    setSponsorUnlocked,
    language,
    changePreference,
    markCliConnectedFromChat,
    transport: transportOf(config),
    currentConfigRef,
  };
}
