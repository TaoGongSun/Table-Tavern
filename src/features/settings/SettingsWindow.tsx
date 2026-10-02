import {
  type KeyboardEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { LANGUAGE_OPTIONS, normalizeLang, t } from "../../i18n";
import { ALL_THEMES, KOFI_URL, resolveTheme, SPONSOR_THEMES, TEXT_SIZE_DEFAULT, TEXT_SIZE_PX, type ThemeId } from "./appearance";
import { AppConfig } from "../../shared/contracts/backend-contracts";
import { type LeaveDecision, useRequestedTab } from "./useRequestedTab";
import { ModalShell } from "../../shared/ui/Dialog";
import { IconClose } from "../../shared/ui/icons";
import { Settings } from "./SettingsForm";
import { UsageTab } from "./UsageTab";
import taoIcon from "../../assets/tao-icon.png";
import { backendText } from "../../shared/ui/backend-text";

const THEME_LABEL_KEYS = { dark: "themeDark", light: "themeLight", parchment: "themeParchment", herbal: "themeHerbal", candlelight: "themeCandlelight", port: "themePort", seamist: "themeSeamist" } as const;
// 色票縮圖用色（與 App.css 各主題 surface-0／accent 同步）
const THEME_SWATCH: Record<string, { bg: string; dot: string }> = {
  dark: { bg: "#20242c", dot: "#e58057" },
  light: { bg: "#e8e8e8", dot: "#b85a35" },
  parchment: { bg: "#eee8d5", dot: "#a2470e" },
  herbal: { bg: "#e2eadb", dot: "#3e6b34" },
  candlelight: { bg: "#251e15", dot: "#e0a24e" },
  port: { bg: "#241a20", dot: "#d9899b" },
  seamist: { bg: "#e1e8eb", dot: "#2c6e86" },
};

const TEXT_SIZE_LABEL_KEYS = {
  xs: "textSizeXS",
  s: "textSizeS",
  m: "textSizeM",
  l: "textSizeL",
  xl: "textSizeXL",
} as const;

const API_MODES = new Set(["auto", "chat_completions", "responses"]);

/** 從設定視窗外面能直接打開的分頁。 */
export type SettingsTab = "appearance" | "ai" | "versions";

const TABS = [
  ["appearance", "appearanceTab"],
  ["ai", "aiTab"],
  ["usage", "usageTab"],
  ["versions", "versionsTab"],
  ["author", "authorTab"],
] as const;

type AnyTab = (typeof TABS)[number][0];

const TAB_ID = (tab: AnyTab) => `settings-tab-${tab}`;
const PANEL_ID = "settings-panel";

// 單一設定入口內分頁（NewPlan §9.4）：外觀為預設頁，不碰 AI 的人打開只見外觀。
// 離開（切分頁、×、底部返回、Esc、遮罩、外部切頁）都走同一道守門：
// 儲存中或 CLI 權限提示開著一律不動；有未儲存修改先問，確認窗同時只開一個。
export function SettingsWindow({
  config,
  onSaved,
  onPreference,
  sponsorUnlocked,
  onSponsorUnlocked,
  onClose,
  initialTab = "appearance",
  requestKey,
  currentWorld,
  versionTab,
}: {
  config: AppConfig;
  onSaved: (c: AppConfig) => void;
  onPreference: (key: string, value: unknown) => void;
  sponsorUnlocked: boolean;
  onSponsorUnlocked: () => void;
  onClose: () => void;
  initialTab?: SettingsTab;
  /** 每次從外面要求開某一頁就加一；視窗已開著時據此切到 initialTab。 */
  requestKey: number;
  currentWorld: string;
  /** 「版本」分頁的內容，由 App 帶著更新與回退的狀態組好。 */
  versionTab: ReactNode;
}) {
  // AI 分頁的未儲存欄位數（外觀分頁即改即存，恆為 0）。只給守門讀、不渲染，所以用 ref：
  // 外部切頁卸載 AI 表單時歸零要立刻生效，緊接著處理下一筆外部請求才不會照舊值再問一次
  const dirtyRef = useRef(0);
  // 阻擋＝儲存中∥CLI 權限提示開著；SettingsForm 在改狀態的同一段處理器裡同步回報，
  // ref 給事件處理器當下判斷、state 讓外部請求在解除時重跑
  const blockedRef = useRef(false);
  const [blocked, setBlocked] = useState(false);
  // 確認窗重入鎖：分頁、關閉、外部切頁共用，不疊兩個系統確認
  const confirmLockRef = useRef(false);
  const [confirmPending, setConfirmPending] = useState(false);
  const focusTabAfterSwitch = useRef(false);
  const modalRef = useRef<HTMLDialogElement>(null);
  const tabButtons = useRef(new Map<AnyTab, HTMLButtonElement>());
  const [tab, setTab] = useRequestedTab<AnyTab>(initialTab, requestKey, confirmDiscard, {
    blocked,
    confirmPending,
    onSwitched: () => {
      focusTabAfterSwitch.current = true;
    },
  });
  const [previewTheme, setPreviewTheme] = useState<ThemeId | null>(null);
  const [sponsorPackError, setSponsorPackError] = useState("");
  const sponsorPackInputRef = useRef<HTMLInputElement>(null);

  const reportDirty = useCallback((count: number) => {
    dirtyRef.current = count;
  }, []);

  function reportBlocking(next: boolean) {
    blockedRef.current = next;
    setBlocked(next);
  }

  async function confirmDiscard(): Promise<LeaveDecision> {
    if (confirmLockRef.current || blockedRef.current) return "busy";
    const dirtyCount = dirtyRef.current;
    if (dirtyCount === 0) return true;
    confirmLockRef.current = true;
    setConfirmPending(true);
    try {
      // 系統確認窗叫不起來就當玩家取消：不關、不切，鎖照樣在 finally 解除
      const leave = await confirm(t("unsavedLeaveConfirm", { n: dirtyCount }), {
        title: t("unsavedLeaveTitle"),
        kind: "warning",
        okLabel: t("unsavedLeaveOk"),
        cancelLabel: t("unsavedLeaveCancel"),
      }).catch(() => false);
      // 確認窗開著時變成阻擋（例如權限提示冒出來）就放棄這次離開
      return blockedRef.current ? "busy" : leave;
    } finally {
      confirmLockRef.current = false;
      setConfirmPending(false);
    }
  }

  async function discardAndClose() {
    if (blockedRef.current) return;
    if ((await confirmDiscard()) === true) onClose();
  }

  async function selectTab(target: AnyTab) {
    if (target === tab || blockedRef.current) return;
    if ((await confirmDiscard()) === true) setTab(target);
  }

  // 方向鍵只移焦點不切頁（切頁可能要確認），Enter／Space 交給按鈕自己的 click
  function onTabKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const keys = ["ArrowLeft", "ArrowRight", "Home", "End"];
    if (!keys.includes(event.key)) return;
    const order = TABS.map(([id]) => id);
    const current = order.findIndex((id) => tabButtons.current.get(id) === event.target);
    if (current < 0) return;
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? order.length - 1
          : (current + (event.key === "ArrowRight" ? 1 : -1) + order.length) % order.length;
    const button = tabButtons.current.get(order[next]);
    button?.focus();
    button?.scrollIntoView?.({ inline: "nearest", block: "nearest" });
  }

  // 外部切頁把原本聚焦的元素卸載了（焦點掉回 body 或跑出視窗）就接到新選中的分頁上
  useEffect(() => {
    if (!focusTabAfterSwitch.current) return;
    focusTabAfterSwitch.current = false;
    const active = document.activeElement;
    if (active === null || active === document.body || !modalRef.current?.contains(active)) {
      tabButtons.current.get(tab)?.focus();
    }
  }, [tab]);

  const textSize = String(config.preferences["text_size"] ?? TEXT_SIZE_DEFAULT);
  const selectedTheme = previewTheme ?? resolveTheme(config, sponsorUnlocked);
  const transport = String(config.preferences["transport"] ?? "api");
  const rawApiMode = String(config.preferences["api_mode"] ?? "auto");
  const apiMode = API_MODES.has(rawApiMode) ? rawApiMode : "auto";
  // 穩定免費固定走 OpenRouter 預設站台的 /chat/completions，這塊相容切換對它沒意義。
  const apiBaseUrl = String(config.preferences["base_url"] ?? "").trim().replace(/\/+$/, "");
  const stableFree =
    transport === "api" &&
    ["", "https://openrouter.ai/api/v1"].includes(apiBaseUrl) &&
    ["stable_free", "smart_free"].includes(
      String(config.preferences["api_model_mode"] ?? "manual"),
    );

  useEffect(() => {
    document.documentElement.dataset.theme = previewTheme ?? resolveTheme(config, sponsorUnlocked);
    return () => {
      document.documentElement.dataset.theme = resolveTheme(config, sponsorUnlocked);
    };
  }, [previewTheme, config, sponsorUnlocked]);

  async function importSponsorPack(file: File) {
    setSponsorPackError("");
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      await invoke("import_sponsor_pack", { data: Array.from(bytes) });
      onSponsorUnlocked();
    } catch (reason) {
      setSponsorPackError(String(reason));
    }
  }

  function selectTheme(theme: ThemeId) {
    if ((SPONSOR_THEMES as readonly string[]).includes(theme) && !sponsorUnlocked) {
      setPreviewTheme(theme);
      return;
    }
    setPreviewTheme(null);
    onPreference("theme", theme);
  }

  return (
    <ModalShell
      ref={modalRef}
      className="settings-modal"
      labelledBy="settings-title"
      onDismiss={() => void discardAndClose()}
      backdrop
    >
      <div className="settings-tabbar">
        <h2 id="settings-title" className="settings-title">
          {t("settingsBtn")}
        </h2>
        <div
          className="settings-tabs"
          role="tablist"
          aria-labelledby="settings-title"
          onKeyDown={onTabKeyDown}
        >
          {TABS.map(([id, labelKey]) => (
            <button
              key={id}
              ref={(node) => {
                if (node) tabButtons.current.set(id, node);
                else tabButtons.current.delete(id);
              }}
              type="button"
              role="tab"
              id={TAB_ID(id)}
              className="settings-tab"
              aria-selected={tab === id}
              aria-controls={PANEL_ID}
              tabIndex={tab === id ? 0 : -1}
              onClick={() => void selectTab(id)}
            >
              {t(labelKey)}
            </button>
          ))}
        </div>
        <button
          type="button"
          className="btn btn-ghost btn-icon settings-close"
          aria-label={t("closeBtn")}
          title={t("closeBtn")}
          onClick={() => void discardAndClose()}
        >
          <IconClose />
        </button>
      </div>
      <div className="settings-body" tabIndex={-1} data-dialog-content="">
        <div
          id={PANEL_ID}
          role="tabpanel"
          aria-labelledby={TAB_ID(tab)}
          className={tab === "ai" ? "settings-panel settings-panel-ai" : "settings-panel"}
          // 非 AI 頁由面板自己捲，可聚焦才能只用鍵盤捲；AI 頁的捲動區在表單裡
          tabIndex={tab === "ai" ? undefined : 0}
        >
          {tab === "appearance" ? (
            <div className="settings-form">
              <label>
                {t("languageLabel")}
                <select
                  value={normalizeLang(config.preferences["language"])}
                  onChange={(e) => onPreference("language", normalizeLang(e.currentTarget.value))}
                >
                  {LANGUAGE_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
              <div className="theme-setting">
                {t("themeLabel")}
                <div className="theme-swatches">
                  {ALL_THEMES.map((theme) => {
                    const locked =
                      (SPONSOR_THEMES as readonly string[]).includes(theme) && !sponsorUnlocked;
                    const name = t(THEME_LABEL_KEYS[theme]);
                    return (
                      <button
                        key={theme}
                        type="button"
                        className="theme-swatch"
                        aria-pressed={selectedTheme === theme}
                        title={name}
                        onClick={() => selectTheme(theme)}
                      >
                        <span
                          className={
                            selectedTheme === theme
                              ? "swatch-chip swatch-chip-selected"
                              : "swatch-chip"
                          }
                          style={{ backgroundColor: THEME_SWATCH[theme].bg }}
                        >
                          {locked && <span className="swatch-kofi">☕</span>}
                          <span
                            className="swatch-dot"
                            style={{ backgroundColor: THEME_SWATCH[theme].dot }}
                          />
                        </span>
                        <span>{name}</span>
                      </button>
                    );
                  })}
                </div>
                {previewTheme && (
                  <p className="theme-preview-hint">
                    {t("themePreviewHint", { name: t(THEME_LABEL_KEYS[previewTheme]) })}{" "}
                    <button type="button" className="link" onClick={() => void openUrl(KOFI_URL)}>
                      {t("sponsorBtn")}
                    </button>
                  </p>
                )}
              </div>
              <label>
                {t("textSizeLabel")}
                <select
                  value={textSize in TEXT_SIZE_PX ? textSize : TEXT_SIZE_DEFAULT}
                  onChange={(e) => onPreference("text_size", e.currentTarget.value)}
                >
                  {(["xs", "s", "m", "l", "xl"] as const).map((size) => (
                    <option key={size} value={size}>
                      {t(TEXT_SIZE_LABEL_KEYS[size])}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          ) : tab === "usage" ? (
            <UsageTab currentWorld={currentWorld} />
          ) : tab === "versions" ? (
            versionTab
          ) : tab === "author" ? (
            <div className="author-page">
              <img src={taoIcon} alt="TaoGongSun" className="avatar-round author-avatar" />
              <strong>TaoGongSun</strong>
              <p className="author-blurb">{t("authorBlurb")}</p>
              <button type="button" className="btn" onClick={() => void openUrl(KOFI_URL)}>
                {t("sponsorBtn")}
              </button>
              {sponsorUnlocked ? (
                <p role="status">{t("sponsorPackUnlocked")}</p>
              ) : (
                <>
                  <button
                    type="button"
                    className="btn"
                    onClick={() => sponsorPackInputRef.current?.click()}
                  >
                    {t("importSponsorPack")}
                  </button>
                  <input
                    ref={sponsorPackInputRef}
                    type="file"
                    accept=".ttpack"
                    hidden
                    onChange={(event) => {
                      const file = event.currentTarget.files?.[0];
                      event.currentTarget.value = "";
                      if (file) void importSponsorPack(file);
                    }}
                  />
                  {sponsorPackError && (
                    <small role="alert">
                      {t("sponsorPackImportError", { reason: backendText(sponsorPackError) })}
                    </small>
                  )}
                </>
              )}
            </div>
          ) : (
            <Settings
              config={config}
              onSaved={onSaved}
              onDirty={reportDirty}
              onBack={() => void discardAndClose()}
              onBlockingChange={reportBlocking}
            >
              {/* 相容格式即存、不算未儲存，看的是已存的 config（不是表單草稿） */}
              {transport === "api" && !stableFree && (
                <div className="settings-form">
                  <details>
                    <summary>{t("apiCompatAdvancedSummary")}</summary>
                    <label>
                      {t("apiFormatLabel")}
                      <select
                        value={apiMode}
                        onChange={(event) => onPreference("api_mode", event.currentTarget.value)}
                      >
                        <option value="auto">{t("apiFormatAuto")}</option>
                        <option value="chat_completions">{t("apiFormatChatCompletions")}</option>
                        <option value="responses">{t("apiFormatResponses")}</option>
                      </select>
                      <small role="note">{t("apiFormatHint")}</small>
                    </label>
                  </details>
                </div>
              )}
            </Settings>
          )}
        </div>
      </div>
    </ModalShell>
  );
}
