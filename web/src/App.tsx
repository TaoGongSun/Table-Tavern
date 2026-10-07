import { useEffect, useState } from "react";
import { t } from "./i18n";
import { CardPicker } from "./features/cards/CardPicker";
import { ChatView } from "./features/chat/ChatView";
import type { GameSetup } from "./features/chat/useChat";
import { DownloadLink } from "./features/funnel/DownloadLink";
import { QuotaPanel } from "./features/funnel/QuotaPanel";
import type { QuotaState } from "./features/funnel/quota";
import { useLatestRelease } from "./features/funnel/useLatestRelease";
import { ConnectPanel } from "./features/openrouter/ConnectPanel";
import type { OAuthCallback } from "./features/openrouter/oauth";
import { useOpenRouterSession } from "./features/openrouter/useOpenRouterSession";

function QuotaBadge({ quota }: { quota: QuotaState }) {
  switch (quota.kind) {
    case "unknown":
      return null;
    case "unlimited":
      return <span className="quota-badge">{t("quotaUnlimited")}</span>;
    case "counted":
      return (
        <span className="quota-badge" data-testid="quota-badge">
          {t("quotaCounted", { remaining: quota.remaining, limit: quota.limit })}
        </span>
      );
    case "exhausted":
      return <span className="quota-badge quota-out">{t("quotaExhaustedBadge")}</span>;
  }
}

export default function App({ callback }: { callback: OAuthCallback | null }) {
  const session = useOpenRouterSession(callback);
  const release = useLatestRelease();
  const [game, setGame] = useState<GameSetup | null>(null);
  const [quotaDismissed, setQuotaDismissed] = useState(false);
  const exhausted = session.quota.kind === "exhausted";

  // 每次重新用完都再提示一次
  useEffect(() => {
    if (!exhausted) setQuotaDismissed(false);
  }, [exhausted]);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-name">Table Tavern</span>
          <span className="brand-sub">{t("brandSub")}</span>
        </div>
        <div className="topbar-actions">
          {session.apiKey && <QuotaBadge quota={session.quota} />}
          <DownloadLink release={release} />
          {session.apiKey && (
            <button
              type="button"
              className="ghost"
              onClick={() => {
                setGame(null);
                session.logout();
              }}
            >
              {t("logout")}
            </button>
          )}
        </div>
      </header>

      <main className="stage">
        {!session.apiKey ? (
          <ConnectPanel session={session} />
        ) : game ? (
          <ChatView game={game} session={session} onBack={() => setGame(null)} />
        ) : (
          <CardPicker onStart={setGame} />
        )}
      </main>

      {exhausted && !quotaDismissed && <QuotaPanel release={release} onClose={() => setQuotaDismissed(true)} />}
    </div>
  );
}
