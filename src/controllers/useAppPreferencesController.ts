import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { resolveTheme, TEXT_SIZE_DEFAULT, TEXT_SIZE_PX } from "../features/settings/appearance";
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

  // 外觀類偏好（語言、文字大小）：改了立即生效並寫回 config，不設儲存鈕
  async function changePreference(key: string, value: unknown) {
    const current = currentConfigRef.current;
    if (!current) return;
    const updated = { ...current, preferences: { ...current.preferences, [key]: value } };
    currentConfigRef.current = updated;
    setConfig(updated);
    try {
      await invoke("write_config", { config: updated });
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
    const updated = {
      ...current,
      preferences: { ...current.preferences, [cliConnectedKey(String(transport))]: true },
    };
    try {
      await invoke("write_config", { config: updated });
      setConfig(updated);
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
