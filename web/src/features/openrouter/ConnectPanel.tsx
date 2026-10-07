import { useState } from "react";
import { checkApiKey } from "@desktop/features/ai-connection/api-key-check";
import { t, type MsgKey } from "../../i18n";
import type { OpenRouterSession } from "./useOpenRouterSession";

export function ConnectPanel({ session }: { session: OpenRouterSession }) {
  const [pasting, setPasting] = useState(false);
  const [draft, setDraft] = useState("");
  const hint = checkApiKey(draft, "");
  const notice = session.notice as MsgKey | null;

  return (
    <section className="connect panel">
      <h1>{t("connectTitle")}</h1>
      <p>{t("connectBody")}</p>
      {session.connecting ? (
        <p className="connect-progress">{t("connecting")}</p>
      ) : (
        <button type="button" className="primary large" onClick={() => void session.connect()}>
          {t("connectButton")}
        </button>
      )}
      {notice && (
        <p className="connect-notice" role="alert">
          {t(notice)}
        </p>
      )}
      {pasting ? (
        <form
          className="paste"
          onSubmit={(event) => {
            event.preventDefault();
            const key = draft.trim();
            if (key) session.adoptKey(key);
          }}
        >
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={draft}
            placeholder={t("pastePlaceholder")}
            onChange={(event) => setDraft(event.target.value)}
          />
          <button type="submit" disabled={draft.trim() === ""}>
            {t("pasteSave")}
          </button>
          {hint && <p className="paste-hint">{t(hint)}</p>}
        </form>
      ) : (
        <button type="button" className="link" onClick={() => setPasting(true)}>
          {t("pasteToggle")}
        </button>
      )}
    </section>
  );
}
