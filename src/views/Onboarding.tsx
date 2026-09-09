// 首次設定卡：transport 走 api、且還沒存過 OpenRouter key 時，才長在遊玩畫面頂端。
// 主路徑是一鍵 OAuth；手動 key 只留在次要 fallback，兩條路都交給後端共用保存／bootstrap 邏輯。
import { useState } from "react";
import type { FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { checkApiKey } from "../features/ai-connection/api-key-check";
import {
  createOpenRouterPkce,
  openRouterOnboardingErrorKey,
} from "../features/ai-connection/openrouter-onboarding";
import { AppConfig } from "../shared/contracts/backend-contracts";

type Busy = "oauth" | "manual" | null;

export function Onboarding({ config, onSaved }: { config: AppConfig; onSaved: (c: AppConfig) => void }) {
  const [apiKey, setApiKey] = useState("");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState<Busy>(null);
  const transport = config.preferences["transport"] ?? "api";
  const keyWarning = checkApiKey(apiKey, String(config.preferences["base_url"] ?? ""));

  if (transport !== "api" || (config.api_keys["openrouter"] ?? "").trim()) return null;

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
