import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { LANGUAGE_OPTIONS, normalizeLang, t } from "../i18n";
import { ALL_THEMES, KOFI_URL, resolveTheme, SPONSOR_THEMES, TEXT_SIZE_DEFAULT, TEXT_SIZE_PX, type ThemeId } from "../appearance";
import { AppConfig } from "../backend-contracts";
import { Settings } from "./SettingsForm";
import { UsageTab } from "./UsageTab";
import taoIcon from "../assets/tao-icon.png";

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

// 單一設定入口內分頁（NewPlan §9.4）：外觀為預設頁，不碰 AI 的人打開只見外觀
export function SettingsWindow({
  config,
  onSaved,
  onPreference,
  sponsorUnlocked,
  onSponsorUnlocked,
  onClose,
  initialTab = "appearance",
  currentWorld,
}: {
  config: AppConfig;
  onSaved: (c: AppConfig) => void;
  onPreference: (key: string, value: unknown) => void;
  sponsorUnlocked: boolean;
  onSponsorUnlocked: () => void;
  onClose: () => void;
  initialTab?: "appearance" | "ai" | "author";
  currentWorld: string;
}) {
  const [tab, setTab] = useState<"appearance" | "ai" | "usage" | "author">(initialTab);
  const [previewTheme, setPreviewTheme] = useState<ThemeId | null>(null);
  const [sponsorPackError, setSponsorPackError] = useState("");
  const sponsorPackInputRef = useRef<HTMLInputElement>(null);
  // AI 分頁的未儲存欄位數（外觀分頁即改即存，恆為 0）
  const [dirtyCount, setDirtyCount] = useState(0);

  // 有未儲存修改時先確認再離開；返回 true 表示可以離開
  async function confirmDiscard() {
    if (dirtyCount === 0) return true;
    return confirm(t("unsavedLeaveConfirm", { n: dirtyCount }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
    });
  }

  async function discardAndClose() {
    if (await confirmDiscard()) onClose();
  }

  async function switchTab(target: "appearance" | "usage" | "author") {
    if (await confirmDiscard()) setTab(target);
  }

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") void discardAndClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const textSize = String(config.preferences["text_size"] ?? TEXT_SIZE_DEFAULT);
  const selectedTheme = previewTheme ?? resolveTheme(config, sponsorUnlocked);

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
    <div className="modal-overlay" onClick={() => void discardAndClose()}>
      <div
        className="modal"
        role="dialog"
        aria-label={t("settingsBtn")}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="modal-header">
          <nav className="tabs" aria-label={t("settingsBtn")}>
            <button
              className={tab === "appearance" ? "tab tab-active" : "tab"}
              onClick={() => void switchTab("appearance")}
            >
              {t("appearanceTab")}
            </button>
            <button className={tab === "ai" ? "tab tab-active" : "tab"} onClick={() => setTab("ai")}>
              {t("aiTab")}
            </button>
            <button
              className={tab === "usage" ? "tab tab-active" : "tab"}
              onClick={() => void switchTab("usage")}
            >
              {t("usageTab")}
            </button>
            <button
              className={tab === "author" ? "tab tab-active" : "tab"}
              onClick={() => void switchTab("author")}
            >
              {t("authorTab")}
            </button>
          </nav>
          <div className="row">
            {dirtyCount > 0 && (
              <span className="unsaved-hint" role="status">
                {t("unsavedChanges", { n: dirtyCount })}
              </span>
            )}
            {tab === "ai" && (
              <button type="submit" form="ai-settings-form">
                {t("saveSettings")}
              </button>
            )}
            <button onClick={() => void discardAndClose()}>
              {dirtyCount > 0 ? t("settingsDiscard") : t("settingsBack")}
            </button>
          </div>
        </header>
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
                  const locked = (SPONSOR_THEMES as readonly string[]).includes(theme) && !sponsorUnlocked;
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
                        className={selectedTheme === theme ? "swatch-chip swatch-chip-selected" : "swatch-chip"}
                        style={{ backgroundColor: THEME_SWATCH[theme].bg }}
                      >
                        {locked && <span className="swatch-kofi">☕</span>}
                        <span className="swatch-dot" style={{ backgroundColor: THEME_SWATCH[theme].dot }} />
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
        ) : tab === "author" ? (
          <div className="author-page">
            <img src={taoIcon} alt="TaoGongSun" className="avatar-round author-avatar" />
            <strong>TaoGongSun</strong>
            <p className="author-blurb">{t("authorBlurb")}</p>
            <button type="button" onClick={() => void openUrl(KOFI_URL)}>
              {t("sponsorBtn")}
            </button>
            {sponsorUnlocked ? (
              <p role="status">{t("sponsorPackUnlocked")}</p>
            ) : (
              <>
                <button type="button" onClick={() => sponsorPackInputRef.current?.click()}>
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
                {sponsorPackError && <small role="alert">{t("sponsorPackImportError", { reason: sponsorPackError })}</small>}
              </>
            )}
          </div>
        ) : (
          <Settings config={config} onSaved={onSaved} onDirty={setDirtyCount} />
        )}
      </div>
    </div>
  );
}
