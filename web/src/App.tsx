import { useCallback, useEffect, useMemo, useState } from "react";
import type { Lang } from "@desktop/i18n/languages";
import { t } from "./i18n";
import { applyLang, rememberLang } from "./i18n/lang-store";
import { LanguagePicker } from "./features/language/LanguagePicker";
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
import { KeyPanel } from "./features/provider/KeyPanel";
import { SettingsMenu, providerName } from "./features/provider/ProviderSettings";
import { isCn } from "./features/provider/providers";
import { useAiConnection } from "./features/provider/useAiConnection";
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

export default function App({ callback, lang: initialLang }: { callback: OAuthCallback | null; lang: Lang }) {
  // 換語系：整棵重繪、文案跟著換；桌、輸入框草稿都是各自的 state，不受影響。供應商跟著語系分流
  const [lang, setLangState] = useState(initialLang);
  const changeLang = useCallback((next: Lang) => {
    applyLang(next);
    rememberLang(next);
    setLangState(next);
  }, []);
  const connection = useAiConnection(callback, lang);
  const session = connection.session;
  const cn = isCn(connection.provider);
  // 只有一家可選的語系不出供應商設定（OpenRouter 沒有手選模型）
  const hasSettings = connection.choices.length > 1;
  // 中國供應商模式不打 GitHub，下載頁只給 pCloud〔作者裁決 2026-11-03〕
  const release = useLatestRelease(!cn);
  const downloadPage = useDownloadPage();
  const [game, setGame] = useState<GameSetup | null>(null);
  // 回合進行中不能換供應商或模型（舊回合收尾前不換 session）
  const [chatBusy, setChatBusy] = useState(false);
  const busy = game !== null && chatBusy;
  // 換到一家沒存金鑰的供應商＝離開這張桌（同登出）：不然貼完金鑰聊天畫面會從開桌時的快照重掛，
  // 自動存檔還會拿那份舊快照蓋掉較新的存檔。桌都在存檔庫，從「接著玩」回來
  const [leftTable, setLeftTable] = useState(false);
  useEffect(() => {
    if (session.apiKey) setLeftTable(false);
    else if (game) {
      setLeftTable(true);
      setGame(null);
    }
  }, [session.apiKey, game]);
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
          {/* 回合中不能換到會連帶換供應商的語系（同供應商選項停用） */}
          <LanguagePicker
            lang={lang}
            onChange={changeLang}
            locked={(next) => busy && connection.providerFor(next) !== connection.provider}
          />
          {session.apiKey && <QuotaBadge quota={session.quota} />}
          {session.apiKey && hasSettings && (
            <SettingsMenu
              className="provider-menu"
              summary={providerName(connection.provider)}
              testId="provider-menu"
              connection={connection}
              busy={busy}
            />
          )}
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
          <div className="connect-stack">
            {connection.previous && (
              <button type="button" className="ghost connect-back" data-testid="connect-back" onClick={connection.back}>
                {t("connectBack", { provider: providerName(connection.previous) })}
              </button>
            )}
            {isCn(connection.provider) ? (
              // 依供應商重掛：貼到一半的草稿不會跟著換到另一家、送去那家
              <KeyPanel key={connection.provider} provider={connection.provider} session={connection.keyed} />
            ) : (
              <ConnectPanel session={connection.openrouter} />
            )}
            {leftTable && (
              <p className="key-note" data-testid="left-table">
                {t("leftTableHint")}
              </p>
            )}
            {hasSettings && (
              <SettingsMenu className="connect-more" summary={t("settingsToggle")} connection={connection} busy={false} />
            )}
          </div>
        ) : !globalsReady ? null : game ? (
          <ChatView
            game={game}
            session={session}
            saves={saves}
            release={release}
            onBack={() => setGame(null)}
            onBusyChange={setChatBusy}
          />
        ) : (
          <CardPicker onStart={setGame} saves={saves} persisted={persisted} release={release} />
        )}
      </main>

      {exhausted && !quotaDismissed && !cn && <QuotaPanel release={release} onClose={() => setQuotaDismissed(true)} />}
      {downloadPage.open && <DownloadPage release={release} cn={cn} onClose={downloadPage.close} />}
    </div>
  );
}
