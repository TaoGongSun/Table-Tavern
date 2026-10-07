import { useCallback, useEffect, useMemo, useState } from "react";
import { t } from "./i18n";
import { CardPicker } from "./features/cards/CardPicker";
import { ChatView } from "./features/chat/ChatView";
import { GLOBAL_VARIABLES, type GameSetup } from "./features/chat/useChat";
import { DownloadLink } from "./features/funnel/DownloadLink";
import { DownloadPage } from "./features/funnel/DownloadPage";
import { useDownloadPage } from "./features/funnel/download-route";
import { QuotaPanel } from "./features/funnel/QuotaPanel";
import type { QuotaState } from "./features/funnel/quota";
import { useLatestRelease } from "./features/funnel/useLatestRelease";
import { ConnectPanel } from "./features/openrouter/ConnectPanel";
import type { OAuthCallback } from "./features/openrouter/oauth";
import { useOpenRouterSession } from "./features/openrouter/useOpenRouterSession";
import { openSaveStore, requestPersistence, restoreGlobals } from "./features/saves/save-store";

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
  const downloadPage = useDownloadPage();
  const [game, setGame] = useState<GameSetup | null>(null);
  const [quotaDismissed, setQuotaDismissed] = useState(false);
  const exhausted = session.quota.kind === "exhausted";
  const saves = useMemo(() => openSaveStore(), []);
  const [persisted, setPersisted] = useState<boolean | null>(null);
  // 跨對話 global 變數照 ST 存在瀏覽器（D29）：讀回來之前不開桌，免得這桌從空的 global 起算
  const [globalsReady, setGlobalsReady] = useState(saves === null);
  const [globalsFailed, setGlobalsFailed] = useState(false);
  const loadGlobals = useCallback(async () => {
    const loaded = await restoreGlobals(saves, GLOBAL_VARIABLES);
    setGlobalsFailed(!loaded);
    setGlobalsReady(true);
  }, [saves]);
  useEffect(() => {
    void loadGlobals();
  }, [loadGlobals]);

  // 存檔在 IndexedDB：一開站就請瀏覽器別在空間吃緊時自動清掉（Safari 的 ITP 七天清資料另外提示）
  useEffect(() => {
    void requestPersistence().then(setPersisted);
  }, []);

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

      {globalsFailed && (
        <p className="chat-error globals-notice" role="alert" data-testid="globals-failed">
          {t("globalsLoadFailed")}
          <button type="button" className="ghost" onClick={() => void loadGlobals()}>
            {t("globalsRetry")}
          </button>
        </p>
      )}

      <main className="stage">
        {!session.apiKey ? (
          <ConnectPanel session={session} />
        ) : !globalsReady ? null : game ? (
          <ChatView game={game} session={session} saves={saves} release={release} onBack={() => setGame(null)} />
        ) : (
          <CardPicker onStart={setGame} saves={saves} persisted={persisted} release={release} />
        )}
      </main>

      {exhausted && !quotaDismissed && <QuotaPanel release={release} onClose={() => setQuotaDismissed(true)} />}
      {downloadPage.open && <DownloadPage release={release} onClose={downloadPage.close} />}
    </div>
  );
}
