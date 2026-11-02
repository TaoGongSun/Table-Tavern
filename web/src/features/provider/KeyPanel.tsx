// 中國供應商的貼令牌面板：照順序點官方頁面、複製、貼上（不出現命令列或程式碼）。
import { useState } from "react";
import { checkApiKey } from "@desktop/features/ai-connection/api-key-check";
import { t } from "../../i18n";
import { CN_PROVIDERS, type CnProviderId } from "./providers";
import type { KeyedSession } from "./useKeyedSession";

export function KeyPanel({ provider, session }: { provider: CnProviderId; session: KeyedSession }) {
  const config = CN_PROVIDERS[provider];
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const hint = checkApiKey(draft, config.apiBase);
  const name = t(`provider_${provider}`);

  return (
    <section className="connect panel" data-testid="key-panel">
      <h1>{t("cnConnectTitle", { provider: name })}</h1>
      <p>{t("cnConnectBody", { provider: name })}</p>
      <ol className="key-steps">
        {config.steps.map((step) => (
          <li key={step.text}>
            {t(step.text)}
            {step.url && (
              <a className="button key-step-link" href={step.url} target="_blank" rel="noopener noreferrer">
                {t("cnOpenPage")}
              </a>
            )}
          </li>
        ))}
      </ol>
      {config.note && <p className="key-note">{t(config.note)}</p>}
      <form
        className="paste"
        onSubmit={(event) => {
          event.preventDefault();
          setSaving(true);
          void session.adoptKey(draft).finally(() => setSaving(false));
        }}
      >
        <input
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={draft}
          placeholder={t("cnPastePlaceholder")}
          aria-label={t("cnPastePlaceholder")}
          onChange={(event) => setDraft(event.target.value)}
        />
        <button type="submit" className="primary" disabled={saving || draft.trim() === ""}>
          {t("pasteSave")}
        </button>
        {hint && <p className="paste-hint">{t(hint)}</p>}
      </form>
      {session.notice && (
        <p className="connect-notice" role="alert">
          {t(session.notice)}
        </p>
      )}
    </section>
  );
}
