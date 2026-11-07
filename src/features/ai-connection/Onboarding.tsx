// 首次設定卡：transport 走 api、且還沒有可用路線時，才長在遊玩畫面頂端。
// 簡中改推矽基流動貼金鑰；其他語系主路徑是一鍵 OAuth，手動 key 只留在次要 fallback。
import { isSiliconFlow, SILICONFLOW_BASE_URL, SILICONFLOW_KEY_URL } from "./siliconflow";
import { useState } from "react";
import type { FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { currentLang, t } from "../../i18n";
import { checkApiKey } from "./api-key-check";
import {
  createOpenRouterPkce,
  openRouterOnboardingErrorKey,
} from "./openrouter-onboarding";
import { defaultApiProvider } from "./transport-lang";
import { updateConfig } from "../settings/update-config";
import { AppConfig } from "../../shared/contracts/backend-contracts";

type Busy = "oauth" | "manual" | null;

/** 有可用的 AI 路線（命令列工具、矽基流動，或存了 OpenRouter 金鑰）：開房間前要先有 */
export function aiReady(config: AppConfig): boolean {
  const transport = config.preferences["transport"] ?? "api";
  return transport !== "api" || isSiliconFlow(config.preferences["base_url"]) || (config.api_keys["openrouter"] ?? "").trim() !== "";
}

export function Onboarding({ config, onSaved }: { config: AppConfig; onSaved: (c: AppConfig) => void }) {
  const [apiKey, setApiKey] = useState("");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState<Busy>(null);
  const keyWarning = checkApiKey(apiKey, String(config.preferences["base_url"] ?? ""));

  if (aiReady(config)) return null;

  if (defaultApiProvider(currentLang()) === "siliconflow") {
    const cnKeyWarning = checkApiKey(apiKey, SILICONFLOW_BASE_URL);
    async function saveSiliconFlow(event: FormEvent<HTMLFormElement>) {
      event.preventDefault();
      const key = apiKey.trim();
      if (!key) return;
      setMessage("");
      setBusy("manual");
      try {
        const next = await updateConfig({
          api_keys: { siliconflow: key },
          preferences: { transport: "api", base_url: SILICONFLOW_BASE_URL },
        });
        onSaved(next);
      } catch {
        setMessage(t("onboardErrSave"));
      } finally {
        setBusy(null);
      }
    }
    return (
      <section className="settings onboarding" role="note">
        <div className="settings-form">
          <strong>{t("onboardCnTitle")}</strong>
          <p>{t("onboardCnIntro")}</p>
          <button type="button" onClick={() => void openUrl(SILICONFLOW_KEY_URL)}>
            {t("cnGetKeyBtn")}
          </button>
          <form className="settings-form" onSubmit={saveSiliconFlow}>
            <div className="row">
              <input
                type="password"
                aria-label={t("cnKeyLabel")}
                value={apiKey}
                onChange={(event) => setApiKey(event.currentTarget.value)}
                disabled={busy !== null}
              />
              <button type="submit" disabled={busy !== null || !apiKey.trim()}>
                {busy === "manual" ? t("onboardManualSaving") : t("onboardCnSave")}
              </button>
            </div>
            {cnKeyWarning && (
              <span className="field-warn" role="alert">
                {t("apiKeyLooksPastedSf")}
              </span>
            )}
          </form>
          {message && (
            <span role="alert" aria-live="polite">
              {message}
            </span>
          )}
          <small>{t("onboardConnectCliHint")}</small>
        </div>
      </section>
    );
  }

  async function connect() {
    setMessage("");
    setBusy("oauth");
    try {
      const { verifier, challenge } = await createOpenRouterPkce();
      const next = await invoke<AppConfig>("connect_openrouter", {
        codeVerifier: verifier,
        codeChallenge: challenge,
      });
      onSaved(next);
    } catch (reason) {
      setMessage(t(openRouterOnboardingErrorKey(reason)));
    } finally {
      setBusy(null);
    }
  }

  async function saveManual(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!apiKey.trim()) return;
    setMessage("");
    setBusy("manual");
    try {
      const next = await invoke<AppConfig>("save_openrouter_key", { apiKey: apiKey.trim() });
      onSaved(next);
    } catch (reason) {
      setMessage(t(openRouterOnboardingErrorKey(reason)));
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="settings onboarding" role="note">
      <div className="settings-form">
        <strong>{t("onboardConnectTitle")}</strong>
        <p>{t("onboardConnectIntro")}</p>
        <p>{t("onboardConnectFree")}</p>
        <button type="button" onClick={() => void connect()} disabled={busy !== null}>
          {busy === "oauth" ? t("onboardConnecting") : t("onboardConnectBtn")}
        </button>
        <small>{t("onboardBrowserHint")}</small>
        {message && (
          <span role="alert" aria-live="polite">
            {message}
          </span>
        )}

        <details>
          <summary>{t("onboardManualSummary")}</summary>
          <form className="settings-form" onSubmit={saveManual}>
            <p>{t("onboardManualIntro")}</p>
            <div className="row">
              <input
                type="password"
                aria-label={t("apiKeyLabel")}
                value={apiKey}
                onChange={(event) => setApiKey(event.currentTarget.value)}
                placeholder={t("apiKeyPlaceholder")}
                disabled={busy !== null}
              />
              <button type="submit" disabled={busy !== null || !apiKey.trim()}>
                {busy === "manual" ? t("onboardManualSaving") : t("onboardManualSave")}
              </button>
            </div>
            {keyWarning && (
              <span className="field-warn" role="alert">
                {t(keyWarning)}
              </span>
            )}
          </form>
        </details>

        <small>{t("onboardConnectCliHint")}</small>
      </div>
    </section>
  );
}
