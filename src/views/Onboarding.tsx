// 首次設定卡：transport 走 api、且還沒存過 OpenRouter key 時，才長在遊玩畫面頂端。
// 主路徑是一鍵 OAuth；手動 key 只留在次要 fallback，兩條路都交給後端共用保存／bootstrap 邏輯。
import { useState } from "react";
import type { FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { normalizeLang, t } from "../i18n";
import { checkApiKey } from "../features/ai-connection/api-key-check";
import {
  createOpenRouterPkce,
  openRouterOnboardingCopy,
  openRouterOnboardingError,
} from "../features/ai-connection/openrouter-onboarding";
import { AppConfig } from "../shared/contracts/backend-contracts";

type Busy = "oauth" | "manual" | null;

export function Onboarding({ config, onSaved }: { config: AppConfig; onSaved: (c: AppConfig) => void }) {
  const [apiKey, setApiKey] = useState("");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState<Busy>(null);
  const transport = config.preferences["transport"] ?? "api";
  const lang = normalizeLang(config.preferences["language"]);
  const copy = openRouterOnboardingCopy(lang);
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
      setMessage(openRouterOnboardingError(lang, reason));
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
      setMessage(openRouterOnboardingError(lang, reason));
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="settings onboarding" role="note">
      <div className="settings-form">
        <strong>{copy.title}</strong>
        <p>{copy.intro}</p>
        <p>{copy.freeNote}</p>
        <button type="button" onClick={() => void connect()} disabled={busy !== null}>
          {busy === "oauth" ? copy.connecting : copy.connect}
        </button>
        <small>{copy.browserHint}</small>
        {message && (
          <span role="alert" aria-live="polite">
            {message}
          </span>
        )}

        <details>
          <summary>{copy.manualSummary}</summary>
          <form className="settings-form" onSubmit={saveManual}>
            <p>{copy.manualIntro}</p>
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
                {busy === "manual" ? copy.savingKey : copy.saveKey}
              </button>
            </div>
            {keyWarning && (
              <span className="field-warn" role="alert">
                {t(keyWarning)}
              </span>
            )}
          </form>
        </details>

        <small>{copy.cliHint}</small>
      </div>
    </section>
  );
}
