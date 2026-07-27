import { FormEvent, useEffect, useRef, useState } from "react";
import Cropper, { Area } from "react-easy-crop";
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { confirm, save } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { Lang, LANGUAGE_OPTIONS, normalizeLang, setLang, t } from "./i18n";
import "./App.css";

type Tier = "best" | "balanced" | "fast" | "default";

interface CharacterMeta {
  name: string;
  color: string;
  avatar: string;
  tier: Tier;
  show_image: boolean;
  archived: boolean;
}

interface CharacterCard extends CharacterMeta {
  public_md: string;
  private_md: string;
}

interface TranscriptEvent {
  ts: string;
  speaker: string;
  kind: "dialogue" | "narration" | "player" | "system";
  text: string;
}

interface WorldState {
  model_bindings: Record<string, string>;
  current_scene: number;
  catchup_summaries: Record<string, string>;
  // 換幕順手取的幕名：key 是內部場號字串（0 起算），對應後端 WorldState.scene_titles
  scene_titles: Record<string, string>;
}

type Visibility =
  | { type: "gm" }
  | { type: "public" }
  | { type: "characters"; characters: string[] };

interface WorldbookEntry {
  uid: number;
  title: string;
  keys: string[];
  content: string;
  constant: boolean;
  order: number;
  disabled: boolean;
  visibility: Visibility;
}

interface WorldbookDraft {
  uid: number | null;
  title: string;
  keys: string;
  content: string;
  constant: boolean;
  enabled: boolean;
  order: number;
  visibility: Visibility["type"];
  characters: string[];
}

interface AppConfig {
  api_keys: Record<string, string>;
  tier_models: Record<string, string>;
  preferences: Record<string, unknown>;
}

// 檔位預設模型只是設定欄的預填建議（存進 config.json 後由使用者作主），程式邏輯不讀它
const SUGGESTED_TIER_MODELS: Record<string, string> = {
  best: "anthropic/claude-opus-4.8",
  balanced: "anthropic/claude-sonnet-5",
  fast: "google/gemini-3.5-flash",
};

// 檔位只是三個插槽，UI 以品質高低命名；內部 key（卡片 frontmatter／config.json）維持不變
const TIER_LABEL_KEYS = {
  best: "tierBest",
  balanced: "tierBalanced",
  fast: "tierFast",
  default: "tierDefault",
} as const;
const tierLabel = (tier: keyof typeof TIER_LABEL_KEYS) => t(TIER_LABEL_KEYS[tier]);

const PALETTE = ["#e07a5f", "#3d84a8", "#81b29a", "#f2a541", "#9b5de5", "#e56399"];

// 側欄寬度是純 UI 狀態，存瀏覽器端即可，不進 config.json。
// 下限擋在這裡，上限交給 CSS 的 max-width: 50%（視窗縮小時自動夾住）。
const SIDEBAR_WIDTH_KEY = "sidebar_width";
const TABLE_LIST_OPEN_KEY = "table_list_open";
const SIDEBAR_DEFAULT_WIDTH = 224;
const SIDEBAR_MIN_WIDTH = 176;
const SIDEBAR_KEY_STEP = 16;

interface CliInfo {
  id: string;
  path: string;
  version: string;
}

type CliInstallStage = "detect" | "install" | "login" | "verify" | "done" | "error";

interface CliInstallProgress {
  provider: string;
  stage: CliInstallStage;
  detail?: string;
  logPath?: string;
}

function cliInstallStageText(stage: CliInstallStage) {
  switch (stage) {
    case "detect":
      return t("cliInstallStageDetect");
    case "install":
      return t("cliInstallStageInstall");
    case "login":
      return t("cliInstallStageLogin");
    case "verify":
      return t("cliInstallStageVerify");
    case "done":
      return t("cliInstallStageDone");
    case "error":
      return t("cliInstallStageError");
  }
}

const CLI_LABELS: Record<string, string> = {
  claude: "Claude Code CLI",
  codex: "Codex CLI",
  // 引擎是 Google Antigravity CLI，但一般使用者只認識 Gemini 這個名字（2026-07-25 拍板）
  agy: "Gemini CLI",
  grok: "Grok CLI",
};

const CLI_INSTALL_URLS: Record<string, string> = {
  claude: "claude.ai",
  codex: "chatgpt.com/codex",
  agy: "antigravity.google",
  grok: "x.ai/cli",
};

const CLI_IDS = ["claude", "codex", "agy", "grok"] as const;

function cliConnectedKey(id: string) {
  return `cli_connected:${id}`;
}

const CLI_RISK_KEYS = ["risk1", "risk2", "risk3", "risk4"] as const;

// 換場提醒門檻：粗略以字元數估算紀錄長度，不精算 token，超過就提示玩家可以換場省額度
const SCENE_LENGTH_HINT_CHARS = 8000;

function nowTs() {
  return new Date().toISOString();
}

function Onboarding({ config, onSaved }: { config: AppConfig; onSaved: (c: AppConfig) => void }) {
  const [apiKey, setApiKey] = useState("");
  const [message, setMessage] = useState("");
  const transport = config.preferences["transport"] ?? "api";

  if (transport !== "api" || (config.api_keys["openrouter"] ?? "").trim()) return null;

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    const next: AppConfig = {
      ...config,
      api_keys: { ...config.api_keys, openrouter: apiKey.trim() },
    };
    try {
      await invoke("write_config", { config: next });
      onSaved(next);
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <section className="settings onboarding" role="note">
      <form className="settings-form" onSubmit={save}>
        <strong>{t("onboardTitle")}</strong>
        <p>{t("onboardIntro")}</p>
        <ol>
          <li>
            {t("onboardStep1")}
            <button type="button" onClick={() => void openUrl("https://openrouter.ai/")}>
              {t("onboardStep1Btn")}
            </button>
          </li>
          <li>{t("onboardStep2")}</li>
          <li>
            {t("onboardStep3")}
            <button
              type="button"
              onClick={() => void openUrl("https://openrouter.ai/settings/keys")}
            >
              {t("onboardStep3Btn")}
            </button>
          </li>
        </ol>
        <p>{t("onboardCost")}</p>
        <div className="row">
          <input
            type="password"
            aria-label={t("apiKeyLabel")}
            value={apiKey}
            onChange={(event) => setApiKey(event.currentTarget.value)}
            placeholder="sk-or-..."
          />
          <button type="submit">{t("onboardSaveBtn")}</button>
        </div>
        {message && <span role="alert">{message}</span>}
        <small>{t("onboardCliHint")}</small>
      </form>
    </section>
  );
}

function Settings({
  config,
  onSaved,
  onDirty,
}: {
  config: AppConfig;
  onSaved: (c: AppConfig) => void;
  onDirty: (count: number) => void;
}) {
  const [apiKey, setApiKey] = useState(config.api_keys["openrouter"] ?? "");
  const [tierModels, setTierModels] = useState<Record<string, string>>({
    ...SUGGESTED_TIER_MODELS,
    ...config.tier_models,
  });
  const [baseUrl, setBaseUrl] = useState(String(config.preferences["base_url"] ?? ""));
  const [claudeCompatBaseUrl, setClaudeCompatBaseUrl] = useState(
    String(config.preferences["claude_base_url"] ?? ""),
  );
  const [claudeCompatKey, setClaudeCompatKey] = useState(config.api_keys["claude_compat"] ?? "");
  const [gmTier, setGmTier] = useState(String(config.preferences["gm_tier"] ?? "best"));
  const [maxRound, setMaxRound] = useState(String(config.preferences["max_round_speakers"] ?? 3));
  const [transport, setTransport] = useState(String(config.preferences["transport"] ?? "api"));
  const [riskAccepted, setRiskAccepted] = useState(config.preferences["cli_risk_accepted"] === true);
  const [clis, setClis] = useState<CliInfo[]>([]);
  const [models, setModels] = useState<{ id: string; name: string }[]>([]);
  const [cliCatalogs, setCliCatalogs] = useState<Record<string, { id: string; label: string }[]>>({});
  const [customTiers, setCustomTiers] = useState<Record<string, boolean>>({});
  const [message, setMessage] = useState("");
  const [installingCli, setInstallingCli] = useState<string | null>(null);
  const [installProgress, setInstallProgress] = useState<Record<string, CliInstallProgress>>({});
  const cliPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  function stopCliPolling() {
    if (cliPollRef.current !== null) {
      clearInterval(cliPollRef.current);
      cliPollRef.current = null;
    }
  }

  useEffect(() => {
    invoke<CliInfo[]>("detect_clis").then(setClis).catch(() => setClis([]));
    // CLI 模型下拉目錄：讀各 CLI 本機快取（後端 list_cli_models）
    for (const id of ["claude", "codex", "agy", "grok"]) {
      invoke<{ id: string; label: string }[]>("list_cli_models", { cli: id })
        .then((options) => setCliCatalogs((prev) => ({ ...prev, [id]: options })))
        .catch(() => {});
    }
    // OpenRouter 公開模型清單（免 key）；拿不到就退化成純手動輸入
    fetch("https://openrouter.ai/api/v1/models")
      .then((res) => res.json())
      .then((body: { data?: { id?: string; name?: string }[] }) =>
        setModels((body.data ?? []).flatMap((m) => (m.id ? [{ id: m.id, name: m.name ?? m.id }] : []))),
      )
      .catch(() => {});
    return stopCliPolling;
  }, []);

  // 監聽器只掛一次；config／onSaved 走 ref 取最新值，避免安裝中重掛掉事件
  const configRef = useRef(config);
  configRef.current = config;
  const onSavedRef = useRef(onSaved);
  onSavedRef.current = onSaved;
  useEffect(() => {
    let disposed = false;
    let stopListening: (() => void) | undefined;
    void listen<CliInstallProgress>("cli-install-progress", (event) => {
      setInstallProgress((previous) => ({
        ...previous,
        [event.payload.provider]: event.payload,
      }));
      if (event.payload.stage === "done" || event.payload.stage === "error") {
        setInstallingCli((current) => (current === event.payload.provider ? null : current));
        const base = configRef.current;
        const next = {
          ...base,
          preferences: {
            ...base.preferences,
            [cliConnectedKey(event.payload.provider)]: event.payload.stage === "done",
          },
        };
        void invoke("write_config", { config: next }).then(() => onSavedRef.current(next)).catch(() => {});
        if (event.payload.stage === "done") {
          void invoke<CliInfo[]>("detect_clis").then(setClis).catch(() => {});
        }
      }
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        stopListening = unlisten;
      }
    });
    return () => {
      disposed = true;
      stopListening?.();
    };
  }, []);

  async function installCli(provider: string) {
    const repeat = installingCli === provider;
    if (!repeat) {
      setInstallingCli(provider);
      setInstallProgress((previous) => {
        const next = { ...previous };
        delete next[provider];
        return next;
      });
    }
    const params = {
      provider: CLI_LABELS[provider],
      url: CLI_INSTALL_URLS[provider],
    };
    try {
      await invoke("install_cli", {
        provider,
        messages: {
          start: t("cliInstallStart", params),
          loginHint: t("cliInstallLoginHint", params),
          success: t("cliInstallSuccess", params),
          fail: t("cliInstallFail", params),
        },
      });
    } catch (reason) {
      const error = String(reason);
      const cooldown = error.match(/^login-cooldown:(\d+)$/);
      setMessage(cooldown ? t("cliLoginCooldown", { secs: cooldown[1] }) : error);
      if (!repeat) setInstallingCli(null);
      return;
    }

    let elapsed = 0;
    stopCliPolling();
    cliPollRef.current = setInterval(() => {
      elapsed += 3_000;
      invoke<CliInfo[]>("detect_clis")
        .then((detected) => {
          setClis(detected);
          if (detected.some((cli) => cli.id === provider) || elapsed >= 600_000) {
            stopCliPolling();
            setInstallingCli(null);
          }
        })
        .catch(() => {
          if (elapsed >= 600_000) {
            stopCliPolling();
            setInstallingCli(null);
          }
        });
    }, 3_000);
  }

  // 未儲存偵測：與 config 現值逐欄比對（比對值採 save() 相同的正規化），改幾欄算幾項
  const dirtyCount = [
    apiKey.trim() !== (config.api_keys["openrouter"] ?? ""),
    baseUrl.trim() !== String(config.preferences["base_url"] ?? ""),
    claudeCompatBaseUrl.trim() !== String(config.preferences["claude_base_url"] ?? ""),
    claudeCompatKey.trim() !== (config.api_keys["claude_compat"] ?? ""),
    gmTier !== String(config.preferences["gm_tier"] ?? "best"),
    String(Math.max(1, Number(maxRound) || 3)) !==
      String(config.preferences["max_round_speakers"] ?? 3),
    transport !== String(config.preferences["transport"] ?? "api"),
    riskAccepted !== (config.preferences["cli_risk_accepted"] === true),
    JSON.stringify(tierModels) !==
      JSON.stringify({ ...SUGGESTED_TIER_MODELS, ...config.tier_models }),
  ].filter(Boolean).length;

  useEffect(() => {
    onDirty(dirtyCount);
    return () => onDirty(0);
  }, [dirtyCount, onDirty]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    if (transport !== "api" && !riskAccepted) {
      setMessage(t("riskRequired"));
      return;
    }
    const next: AppConfig = {
      ...config,
      api_keys: {
        ...config.api_keys,
        openrouter: apiKey.trim(),
        claude_compat: claudeCompatKey.trim(),
      },
      tier_models: tierModels,
      preferences: {
        ...config.preferences,
        base_url: baseUrl.trim(),
        claude_base_url: claudeCompatBaseUrl.trim(),
        transport,
        cli_risk_accepted: riskAccepted,
        gm_tier: gmTier,
        max_round_speakers: Math.max(1, Number(maxRound) || 3),
      },
    };
    try {
      await invoke("write_config", { config: next });
      onSaved(next);
      setMessage(t("saved"));
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <form id="ai-settings-form" onSubmit={save} className="settings-form">
        <fieldset className="transport-choice">
          <legend>{t("transportLegend")}</legend>
          <label className="inline">
            <input
              type="radio"
              name="transport"
              checked={transport === "api"}
              onChange={() => setTransport("api")}
            />
            {t("transportApi")}
          </label>
          {(["claude", "codex", "agy", "grok"] as const).map((id) => {
            const found = clis.find((c) => c.id === id);
            const progress = installProgress[id];
            const connected = config.preferences[cliConnectedKey(id)] === true;
            return (
              <label key={id} className="inline">
                <input
                  type="radio"
                  name="transport"
                  disabled={!found}
                  checked={transport === id}
                  onChange={() => setTransport(id)}
                />
                {CLI_LABELS[id]}
                {t("cliSubscriptionSuffix")}
                {found ? (
                  <>
                    <span className="cli-version">{t("cliDetected", { version: found.version })}</span>
                    {connected && installingCli !== id ? (
                      <>
                        <span className="cli-connected">{t("cliConnectedBadge")}</span>
                        <button
                          type="button"
                          disabled={installingCli !== null && installingCli !== id}
                          onClick={() => void installCli(id)}
                        >
                          {t("cliReverifyBtn")}
                        </button>
                      </>
                    ) : (
                      <button
                        type="button"
                        disabled={installingCli !== null && installingCli !== id}
                        onClick={() => void installCli(id)}
                      >
                        {installingCli === id
                          ? t("cliInstalling", { provider: CLI_LABELS[id] })
                          : t("cliLoginVerifyBtn")}
                      </button>
                    )}
                  </>
                ) : (
                  <>
                    <span className="cli-version">{t("cliNotDetected")}</span>
                    <button
                      type="button"
                      disabled={installingCli !== null && installingCli !== id}
                      onClick={() => void installCli(id)}
                    >
                      {installingCli === id
                        ? t("cliInstalling", { provider: CLI_LABELS[id] })
                        : t("cliInstallBtn")}
                    </button>
                  </>
                )}
                {progress && (
                  <span
                    className={`cli-install-progress${progress.stage === "error" ? " cli-install-error" : ""}`}
                    role={progress.stage === "error" ? "alert" : "status"}
                  >
                    <strong>{cliInstallStageText(progress.stage)}</strong>
                    {progress.detail && (
                      <span className="cli-install-detail">{progress.detail}</span>
                    )}
                    {progress.logPath && (
                      <small>{t("cliInstallLogPath", { path: progress.logPath })}</small>
                    )}
                  </span>
                )}
              </label>
            );
          })}
        </fieldset>
        {transport !== "api" && (
          <div className="risk-box" role="note">
            <strong>{t("riskTitle")}</strong>
            <ul>
              {CLI_RISK_KEYS.map((key) => (
                <li key={key}>{t(key)}</li>
              ))}
            </ul>
            <label className="inline">
              <input
                type="checkbox"
                checked={riskAccepted}
                onChange={(e) => setRiskAccepted(e.currentTarget.checked)}
              />
              {t("riskAccept")}
            </label>
          </div>
        )}
        {/* OpenRouter 專屬欄位只在 API 直連時顯示，避免 CLI 使用者誤以為必填 */}
        {transport === "api" && (
          <label>
            {t("apiKeyLabel")}
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.currentTarget.value)}
              placeholder="sk-or-..."
            />
          </label>
        )}
        {transport === "api" ? (
          <>
            {(["best", "balanced", "fast"] as const).map((tier) => (
              <label key={tier}>
                {t("tierModelApiLabel", { tier: tierLabel(tier) })}
                <input
                  list="openrouter-models"
                  value={tierModels[tier] ?? ""}
                  onChange={(e) =>
                    setTierModels({ ...tierModels, [tier]: e.currentTarget.value })
                  }
                />
              </label>
            ))}
            <datalist id="openrouter-models">
              {models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))}
            </datalist>
          </>
        ) : (
          <>
            {(["best", "balanced", "fast"] as const).map((tier) => {
              const key = `${transport}:${tier}`;
              const value = tierModels[key] ?? "";
              const catalog = cliCatalogs[transport] ?? [];
              const custom =
                customTiers[key] ?? (value !== "" && !catalog.some((m) => m.id === value));
              return (
                <label key={key}>
                  {t("tierModelCliLabel", { tier: tierLabel(tier) })}
                  <select
                    value={custom ? "__custom__" : value}
                    onChange={(e) => {
                      const next = e.currentTarget.value;
                      if (next === "__custom__") {
                        setCustomTiers({ ...customTiers, [key]: true });
                      } else {
                        setCustomTiers({ ...customTiers, [key]: false });
                        setTierModels({ ...tierModels, [key]: next });
                      }
                    }}
                  >
                    <option value="">{t("cliDefaultOption")}</option>
                    {catalog.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.label}
                      </option>
                    ))}
                    <option value="__custom__">{t("customModelOption")}</option>
                  </select>
                  {custom && (
                    <input
                      value={value}
                      placeholder={t("customModelPlaceholder")}
                      onChange={(e) =>
                        setTierModels({ ...tierModels, [key]: e.currentTarget.value })
                      }
                    />
                  )}
                </label>
              );
            })}
            <p className="cli-version" role="note">
              {transport === "claude"
                ? t("cliCatalogClaude")
                : transport === "agy"
                  ? t("cliCatalogAgy")
                  : transport === "grok"
                    ? t("cliCatalogGrok")
                  : t("cliCatalogCodex")}
            </p>
          </>
        )}
        {transport === "claude" && (
          <details>
            <summary>{t("claudeCompatSummary")}</summary>
            <label>
              {t("claudeCompatBaseUrlLabel")}
              <input
                value={claudeCompatBaseUrl}
                onChange={(e) => setClaudeCompatBaseUrl(e.currentTarget.value)}
                placeholder="https://api.example.com"
              />
            </label>
            <label>
              {t("claudeCompatKeyLabel")}
              <input
                type="password"
                value={claudeCompatKey}
                onChange={(e) => setClaudeCompatKey(e.currentTarget.value)}
              />
            </label>
            <p role="note">{t("claudeCompatHint")}</p>
          </details>
        )}
        <label>
          {t("gmTierLabel")}
          <select value={gmTier} onChange={(e) => setGmTier(e.currentTarget.value)}>
            {(["best", "balanced", "fast", "default"] as const).map((tier) => (
              <option key={tier} value={tier}>
                {tierLabel(tier)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t("maxRoundLabel")}
          <input
            type="number"
            min={1}
            max={10}
            value={maxRound}
            onChange={(e) => setMaxRound(e.currentTarget.value)}
          />
        </label>
        {transport === "api" && (
          <label>
            {t("baseUrlLabel")}
            <input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.currentTarget.value)}
              placeholder="https://openrouter.ai/api/v1"
            />
          </label>
        )}
      {message && (
        <div className="row">
          <span role="status">{message}</span>
        </div>
      )}
    </form>
  );
}

// 文字大小偏好：存 config.preferences.text_size，套在 html 根字級（rem 版面跟著縮放）。
// 五檔偏小取向（大螢幕看長文要小字）；預設 l（16px）＝原本的視覺大小
const TEXT_SIZE_PX: Record<string, string> = {
  xs: "10px",
  s: "12px",
  m: "14px",
  l: "16px",
  xl: "18px",
};
const TEXT_SIZE_LABEL_KEYS = {
  xs: "textSizeXS",
  s: "textSizeS",
  m: "textSizeM",
  l: "textSizeL",
  xl: "textSizeXL",
} as const;
const TEXT_SIZE_DEFAULT = "l";

// 單一設定入口內分頁（NewPlan §9.4）：外觀為預設頁，不碰 AI 的人打開只見外觀
function SettingsWindow({
  config,
  onSaved,
  onPreference,
  onClose,
}: {
  config: AppConfig;
  onSaved: (c: AppConfig) => void;
  onPreference: (key: string, value: unknown) => void;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<"appearance" | "ai">("appearance");
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

  async function switchToAppearance() {
    if (await confirmDiscard()) setTab("appearance");
  }

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") void discardAndClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const textSize = String(config.preferences["text_size"] ?? TEXT_SIZE_DEFAULT);

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
              onClick={() => void switchToAppearance()}
            >
              {t("appearanceTab")}
            </button>
            <button className={tab === "ai" ? "tab tab-active" : "tab"} onClick={() => setTab("ai")}>
              {t("aiTab")}
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
            <label>
              {t("themeLabel")}
              <select
                value={config.preferences["theme"] === "light" ? "light" : "dark"}
                onChange={(e) => onPreference("theme", e.currentTarget.value)}
              >
                <option value="dark">{t("themeDark")}</option>
                <option value="light">{t("themeLight")}</option>
              </select>
            </label>
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
        ) : (
          <Settings config={config} onSaved={onSaved} onDirty={setDirtyCount} />
        )}
      </div>
    </div>
  );
}

// 世界書 v1：一份只進 GM 上下文的 world.md（NewPlan §7.0）
function WorldEditor({ world, onBack }: { world: string; onBack: () => void }) {
  const [text, setText] = useState<string | null>(null);
  const [savedText, setSavedText] = useState("");
  const [message, setMessage] = useState("");
  const [entries, setEntries] = useState<WorldbookEntry[]>([]);
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  const [worldbookMessage, setWorldbookMessage] = useState("");
  const [draft, setDraft] = useState<WorldbookDraft | null>(null);
  // 條目表單開啟當下的快照，用來判斷「有沒有改過」（未儲存提示）
  const [draftOrigin, setDraftOrigin] = useState("");
  const importInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setMessage("");
    setWorldbookMessage("");
    setText(null);
    setEntries([]);
    setCharacters([]);
    setDraft(null);
    invoke<string>("read_world_md", { world })
      .then((value) => {
        setText(value);
        setSavedText(value);
      })
      .catch((reason) => setMessage(String(reason)));
    invoke<WorldbookEntry[]>("read_worldbook", { world })
      .then(setEntries)
      .catch((reason) => setWorldbookMessage(String(reason)));
    invoke<CharacterMeta[]>("list_characters", { world })
      .then((cast) => setCharacters(cast.filter((character) => !character.archived)))
      .catch((reason) => setWorldbookMessage(String(reason)));
  }, [world]);

  if (text === null) return message ? <p role="alert">{message}</p> : null;

  const unsavedCount =
    (text !== savedText ? 1 : 0) + (draft && JSON.stringify(draft) !== draftOrigin ? 1 : 0);

  async function saveWorldSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    try {
      await invoke("write_world_md", { world, content: text });
      setSavedText(text ?? "");
      setMessage(t("saved"));
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function handleBack() {
    if (unsavedCount > 0) {
      const accepted = await confirm(t("unsavedLeaveConfirm", { n: unsavedCount }), {
        title: t("unsavedLeaveTitle"),
        kind: "warning",
      });
      if (!accepted) return;
    }
    onBack();
  }

  async function refreshWorldbook() {
    setEntries(await invoke<WorldbookEntry[]>("read_worldbook", { world }));
  }

  function openDraft(next: WorldbookDraft) {
    setWorldbookMessage("");
    setDraft(next);
    setDraftOrigin(JSON.stringify(next));
  }

  function addEntry() {
    openDraft({
      uid: null,
      title: "",
      keys: "",
      content: "",
      constant: false,
      enabled: true,
      order: 100,
      visibility: "gm",
      characters: [],
    });
  }

  function editEntry(entry: WorldbookEntry) {
    openDraft({
      uid: entry.uid,
      title: entry.title,
      keys: entry.keys.join("、"),
      content: entry.content,
      constant: entry.constant,
      enabled: !entry.disabled,
      order: entry.order,
      visibility: entry.visibility.type,
      characters: entry.visibility.type === "characters" ? entry.visibility.characters : [],
    });
  }

  async function saveEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft) return;
    setWorldbookMessage("");
    const visibility: Visibility =
      draft.visibility === "characters"
        ? {
            type: "characters",
            characters: draft.characters.filter((name) =>
              characters.some((character) => character.name === name),
            ),
          }
        : { type: draft.visibility };
    const entry: WorldbookEntry = {
      uid: draft.uid ?? Number.MAX_SAFE_INTEGER,
      title: draft.title.trim(),
      keys: draft.keys
        .split(/[,、]/)
        .map((key) => key.trim())
        .filter(Boolean),
      content: draft.content,
      constant: draft.constant,
      order: draft.order,
      disabled: !draft.enabled,
      visibility,
    };
    try {
      await invoke<number>("upsert_worldbook_entry", { world, entry });
      await refreshWorldbook();
      setDraft(null);
      setWorldbookMessage(t("worldbookEntrySaved"));
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  async function deleteEntry(entry: WorldbookEntry) {
    setWorldbookMessage("");
    try {
      const accepted = await confirm(
        t("worldbookDeleteConfirm", { title: entry.title || String(entry.uid) }),
        { title: t("worldbookDeleteTitle"), kind: "warning" },
      );
      if (!accepted) return;
      await invoke("delete_worldbook_entry", { world, uid: entry.uid });
      await refreshWorldbook();
      if (draft?.uid === entry.uid) setDraft(null);
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  async function moveEntry(entry: WorldbookEntry, up: boolean) {
    setWorldbookMessage("");
    try {
      await invoke("move_worldbook_entry", { world, uid: entry.uid, up });
      await refreshWorldbook();
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  async function importWorldbook(file: File) {
    setWorldbookMessage("");
    try {
      const jsonText = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () =>
          typeof reader.result === "string" ? resolve(reader.result) : reject(t("worldbookReadError"));
        reader.onerror = () => reject(reader.error ?? new Error(t("worldbookReadError")));
        reader.readAsText(file);
      });
      const count = await invoke<number>("import_worldbook", { world, jsonText });
      await refreshWorldbook();
      setWorldbookMessage(t("worldbookImported", { n: count }));
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  async function exportWorldbook() {
    setWorldbookMessage("");
    try {
      const path = await save({
        defaultPath: "worldbook.json",
        filters: [{ name: t("worldbookJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("export_worldbook", { world, path });
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  return (
    <>
      <form onSubmit={saveWorldSettings} className="settings-form">
        {/* 按鈕列放文字框上方：長文編輯時儲存／返回固定在最顯眼處（2026-07-24 使用者回饋） */}
        <div className="row">
          <button type="submit">{t("saveWorld")}</button>
          <button type="button" onClick={() => void handleBack()}>
            {t("backToNow")}
          </button>
          {message && <span>{message}</span>}
          {unsavedCount > 0 && (
            <span className="unsaved-hint" role="status">
              {t("unsavedChanges", { n: unsavedCount })}
            </span>
          )}
        </div>
        <textarea
          rows={6}
          aria-label={t("worldAria")}
          value={text}
          onChange={(e) => setText(e.currentTarget.value)}
        />
      </form>

      <section className="worldbook-section" aria-labelledby="worldbook-title">
        <h3 id="worldbook-title">{t("worldbookTitle")}</h3>
        <div className="worldbook-actions">
          <button type="button" onClick={addEntry}>
            {t("worldbookAddEntry")}
          </button>
          <button type="button" onClick={() => importInputRef.current?.click()}>
            {t("worldbookImport")}
          </button>
          <input
            ref={importInputRef}
            type="file"
            accept=".json,application/json"
            hidden
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              event.currentTarget.value = "";
              if (file) void importWorldbook(file);
            }}
          />
          <button type="button" onClick={() => void exportWorldbook()}>
            {t("worldbookExport")}
          </button>
        </div>

        {draft && (
          <form className="settings-form worldbook-form" onSubmit={saveEntry}>
            <label>
              {t("worldbookEntryTitle")}
              <input
                value={draft.title}
                onChange={(event) =>
                  setDraft({ ...draft, title: event.currentTarget.value })
                }
              />
            </label>
            <label>
              {t("worldbookKeys")}
              <input
                value={draft.keys}
                placeholder={t("worldbookKeysHint")}
                onChange={(event) => setDraft({ ...draft, keys: event.currentTarget.value })}
              />
            </label>
            <label>
              {t("worldbookContent")}
              <textarea
                rows={7}
                value={draft.content}
                onChange={(event) =>
                  setDraft({ ...draft, content: event.currentTarget.value })
                }
              />
            </label>
            <label className="inline">
              <input
                type="checkbox"
                checked={draft.constant}
                onChange={(event) =>
                  setDraft({ ...draft, constant: event.currentTarget.checked })
                }
              />
              {t("worldbookConstantLabel")}
            </label>
            <label className="inline">
              <input
                type="checkbox"
                checked={draft.enabled}
                onChange={(event) =>
                  setDraft({ ...draft, enabled: event.currentTarget.checked })
                }
              />
              {t("worldbookEnabled")}
            </label>
            <fieldset className="worldbook-visibility">
              <legend>{t("worldbookVisibility")}</legend>
              {(["gm", "public", "characters"] as const).map((visibility) => (
                <label className="inline" key={visibility}>
                  <input
                    type="radio"
                    name="worldbook-visibility"
                    value={visibility}
                    checked={draft.visibility === visibility}
                    onChange={() => {
                      setDraft({ ...draft, visibility });
                      // 點「指定角色」當下重抓在場角色：畫面開著時可能剛從隱藏區還原角色
                      if (visibility === "characters") {
                        void invoke<CharacterMeta[]>("list_characters", { world }).then((cast) =>
                          setCharacters(cast.filter((character) => !character.archived)),
                        );
                      }
                    }}
                  />
                  {visibility === "gm"
                    ? t("worldbookVisibilityGm")
                    : visibility === "public"
                      ? t("worldbookVisibilityPublic")
                      : t("worldbookVisibilityCharacters")}
                </label>
              ))}
            </fieldset>
            {draft.visibility === "characters" && (
              <fieldset className="worldbook-characters">
                <legend>{t("worldbookChooseCharacters")}</legend>
                {characters.length === 0 ? (
                  <span>{t("worldbookNoCharacters")}</span>
                ) : (
                  characters.map((character) => (
                    <label className="inline" key={character.name}>
                      <input
                        type="checkbox"
                        checked={draft.characters.includes(character.name)}
                        onChange={(event) =>
                          setDraft({
                            ...draft,
                            characters: event.currentTarget.checked
                              ? [...draft.characters, character.name]
                              : draft.characters.filter((name) => name !== character.name),
                          })
                        }
                      />
                      {character.name}
                    </label>
                  ))
                )}
              </fieldset>
            )}
            <div className="row">
              <button type="submit">{t("worldbookSaveEntry")}</button>
              <button type="button" onClick={() => setDraft(null)}>
                {t("worldbookCancel")}
              </button>
            </div>
          </form>
        )}

        {entries.length === 0 ? (
          <p className="worldbook-empty">{t("worldbookEmpty")}</p>
        ) : (
          <div className="worldbook-list">
            {entries.map((entry, index) => (
              <div
                className={`worldbook-row${entry.disabled ? " worldbook-row-disabled" : ""}`}
                key={entry.uid}
              >
                <div className="worldbook-summary">
                  <strong>{entry.title || entry.uid}</strong>
                  <span>{entry.keys.join("、") || t("worldbookNoKeys")}</span>
                  <div className="worldbook-badges">
                    {entry.constant && (
                      <span className="worldbook-badge">{t("worldbookConstant")}</span>
                    )}
                    {/* 可見範圍＝資訊邊界：全 app 統一的虛線琥珀機密記號 */}
                    <span className="worldbook-badge worldbook-badge-visibility">
                      {entry.visibility.type === "gm"
                        ? t("worldbookVisibilityGm")
                        : entry.visibility.type === "public"
                          ? t("worldbookVisibilityPublic")
                          : t("worldbookCharacterCount", {
                              n: entry.visibility.characters.length,
                            })}
                    </span>
                    {entry.disabled && (
                      <span className="worldbook-badge">{t("worldbookDisabled")}</span>
                    )}
                  </div>
                </div>
                <div className="worldbook-row-actions">
                  <button
                    type="button"
                    aria-label={t("worldbookMoveUp")}
                    disabled={index === 0}
                    onClick={() => void moveEntry(entry, true)}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    aria-label={t("worldbookMoveDown")}
                    disabled={index === entries.length - 1}
                    onClick={() => void moveEntry(entry, false)}
                  >
                    ↓
                  </button>
                  <button type="button" onClick={() => editEntry(entry)}>
                    {t("editBtn")}
                  </button>
                  <button type="button" onClick={() => void deleteEntry(entry)}>
                    {t("worldbookDelete")}
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
        {worldbookMessage && <p role="status">{worldbookMessage}</p>}
      </section>
    </>
  );
}

function CropDialog({
  title,
  src,
  aspect,
  cropShape,
  onConfirm,
  onCancel,
}: {
  title: string;
  src: string;
  aspect: number;
  cropShape: "rect" | "round";
  onConfirm: (bytes: number[]) => Promise<void>;
  onCancel: () => void;
}) {
  const [crop, setCrop] = useState({ x: 0, y: 0 });
  const [zoom, setZoom] = useState(1);
  const [croppedAreaPixels, setCroppedAreaPixels] = useState<Area | null>(null);
  const [message, setMessage] = useState("");

  async function confirmCrop() {
    if (!croppedAreaPixels) return;
    setMessage("");
    try {
      const image = new Image();
      await new Promise<void>((resolve, reject) => {
        image.onload = () => resolve();
        image.onerror = () => reject(new Error("Unable to load image"));
        image.src = src;
      });
      const size = cropShape === "round" ? 256 : Math.min(Math.round(croppedAreaPixels.width), 1024);
      const height =
        cropShape === "round"
          ? 256
          : Math.max(1, Math.round((croppedAreaPixels.height / croppedAreaPixels.width) * size));
      const canvas = document.createElement("canvas");
      canvas.width = size;
      canvas.height = height;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("Unable to create image canvas");
      // 頭像存正方形原樣，圓形與黑框由 CSS 畫（拍板規格），canvas 不做圓形裁切
      context.drawImage(
        image,
        croppedAreaPixels.x,
        croppedAreaPixels.y,
        croppedAreaPixels.width,
        croppedAreaPixels.height,
        0,
        0,
        size,
        height,
      );
      const blob = await new Promise<Blob>((resolve, reject) => {
        canvas.toBlob((result) => (result ? resolve(result) : reject(new Error("Unable to crop image"))), "image/png");
      });
      await onConfirm(Array.from(new Uint8Array(await blob.arrayBuffer())));
      onCancel();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" role="dialog" aria-modal="true" aria-label={title} onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <strong>{title}</strong>
          <button type="button" className="modal-close" aria-label={t("closeBtn")} onClick={onCancel}>×</button>
        </div>
        <div className="crop-area">
          <Cropper
            image={src}
            crop={crop}
            zoom={zoom}
            aspect={aspect}
            cropShape={cropShape}
            onCropChange={setCrop}
            onZoomChange={setZoom}
            onCropComplete={(_, area) => setCroppedAreaPixels(area)}
          />
        </div>
        <label className="crop-zoom">
          {t("zoomLabel")}
          <input type="range" min={1} max={4} step={0.05} value={zoom} onChange={(event) => setZoom(Number(event.currentTarget.value))} />
        </label>
        <div className="row">
          <button type="button" onClick={() => void confirmCrop()}>{t("cropConfirm")}</button>
          <button type="button" onClick={onCancel}>{t("cropCancel")}</button>
          {message && <span role="alert">{message}</span>}
        </div>
      </div>
    </div>
  );
}

function CardEditor({
  world,
  name,
  imageDataUrl,
  avatarImgUrl,
  onImagesChanged,
  onSaved,
  onArchived,
  onBack,
}: {
  world: string;
  name: string;
  imageDataUrl?: string;
  avatarImgUrl?: string;
  onImagesChanged: () => Promise<void>;
  onBack: () => void;
  onSaved: () => void;
  onArchived: () => Promise<void>;
}) {
  const [card, setCard] = useState<CharacterCard | null>(null);
  const [message, setMessage] = useState("");
  const [pendingImage, setPendingImage] = useState<string | null>(null);
  const [croppingAvatar, setCroppingAvatar] = useState(false);
  const [lightboxOpen, setLightboxOpen] = useState(false);

  useEffect(() => {
    setMessage("");
    invoke<CharacterCard>("read_character", { world, name })
      .then(setCard)
      .catch((reason) => setMessage(String(reason)));
  }, [world, name]);

  if (!card) return message ? <p role="alert">{message}</p> : null;

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    try {
      await invoke("write_character", { world, card });
      setMessage(t("saved"));
      onSaved();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function archive() {
    setMessage("");
    try {
      await invoke("set_character_archived", { world, name, archived: true });
      await onArchived();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  function chooseImage(file: File) {
    const reader = new FileReader();
    reader.onload = () => setPendingImage(typeof reader.result === "string" ? reader.result : null);
    reader.onerror = () => setMessage(String(reader.error));
    reader.readAsDataURL(file);
  }

  async function removeImage() {
    setMessage("");
    try {
      await invoke("delete_character_image", { world, name });
      await onImagesChanged();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function removeAvatar() {
    setMessage("");
    try {
      await invoke("delete_character_avatar", { world, name });
      await onImagesChanged();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <form onSubmit={save} className="settings-form">
      {/* 按鈕列統一放頂部，與世界設定畫面同款（2026-07-24 使用者拍板） */}
      <div className="row">
        <button type="submit">{t("saveCard")}</button>
        <button type="button" onClick={onBack}>
          {t("backToNow")}
        </button>
        <button type="button" className="archive-button" onClick={archive}>
          {t("archiveCharacter")}
        </button>
        {message && <span>{message}</span>}
      </div>
      <div className="card-editor-avatar">
        {imageDataUrl ? (
          <button
            type="button"
            className="card-editor-image-zoom"
            aria-label={t("viewImageLabel")}
            title={t("viewImageLabel")}
            onClick={() => setLightboxOpen(true)}
          >
            <img className="card-editor-image" src={imageDataUrl} alt="" />
          </button>
        ) : avatarImgUrl ? (
          <img className="avatar-round card-editor-avatar-round" src={avatarImgUrl} alt="" />
        ) : (
          <span className="card-editor-avatar-emoji" style={{ ["--ring" as string]: card.color }}>
            {card.avatar}
          </span>
        )}
      </div>
      <div className="row">
        <button type="button" onClick={() => document.getElementById(`character-image-${name}`)?.click()}>
          {t(imageDataUrl ? "replaceImageBtn" : "addImageBtn")}
        </button>
        {imageDataUrl && (
          <>
            <button type="button" onClick={() => void removeImage()}>{t("removeImageBtn")}</button>
            <button type="button" onClick={() => setCroppingAvatar(true)}>{t("makeAvatarBtn")}</button>
          </>
        )}
        {avatarImgUrl && <button type="button" onClick={() => void removeAvatar()}>{t("removeAvatarBtn")}</button>}
        <input
          id={`character-image-${name}`}
          type="file"
          accept="image/png,image/jpeg,image/webp"
          hidden
          onChange={(event) => {
            const file = event.currentTarget.files?.[0];
            event.currentTarget.value = "";
            if (file) chooseImage(file);
          }}
        />
      </div>
      <label>
        {t("publicLabel")}
        <textarea
          rows={4}
          value={card.public_md}
          onChange={(e) => setCard({ ...card, public_md: e.currentTarget.value })}
        />
      </label>
      <label>
        {t("privateLabel")}
        <textarea
          rows={4}
          value={card.private_md}
          onChange={(e) => setCard({ ...card, private_md: e.currentTarget.value })}
        />
      </label>
      {imageDataUrl && (
        <label className="inline">
          <input
            type="checkbox"
            checked={card.show_image}
            onChange={(e) => setCard({ ...card, show_image: e.currentTarget.checked })}
          />
          {t("showImageLabel")}
        </label>
      )}
      <label>
        {t("tierLabel")}
        <select
          value={card.tier}
          onChange={(e) => setCard({ ...card, tier: e.currentTarget.value as Tier })}
        >
          {(["default", "best", "balanced", "fast"] as const).map((tier) => (
            <option key={tier} value={tier}>
              {tierLabel(tier)}
            </option>
          ))}
        </select>
      </label>
      {pendingImage && (
        <CropDialog
          title={t("cropImageTitle")}
          src={pendingImage}
          aspect={2 / 3}
          cropShape="rect"
          onConfirm={async (data) => {
            await invoke("save_character_image", { world, name, data });
            await onImagesChanged();
          }}
          onCancel={() => setPendingImage(null)}
        />
      )}
      {lightboxOpen && imageDataUrl && (
        <div
          className="modal-overlay"
          role="dialog"
          aria-modal="true"
          aria-label={t("viewImageLabel")}
          onClick={() => setLightboxOpen(false)}
        >
          <img className="lightbox-image" src={imageDataUrl} alt="" />
        </div>
      )}
      {croppingAvatar && imageDataUrl && (
        <CropDialog
          title={t("cropAvatarTitle")}
          src={imageDataUrl}
          aspect={1}
          cropShape="round"
          onConfirm={async (data) => {
            await invoke("save_character_avatar", { world, name, data });
            await onImagesChanged();
          }}
          onCancel={() => setCroppingAvatar(false)}
        />
      )}
    </form>
  );
}

// 卡片／世界設定編輯共用整面外框：與單幕閱讀同款（頂部標題，下方內容填滿），不是 modal——
// 使用者拍板：主欄下半部（messages＋composer）整面取代，composer 不渲染＝編輯中無法發言。
// 「返回」不在這裡：使用者拍板放在表單的儲存鈕旁邊，由 CardEditor／WorldEditor 自己渲染
function EditPane({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <>
      <div className="act-reader-header">
        <strong>{title}</strong>
      </div>
      <div className="edit-pane-body">{children}</div>
    </>
  );
}

// 單幕閱讀：整面取代對話畫面（不是 modal），頂部一行標題＋匯出＋返回，下方唯讀事件列表填滿到底
function ActReader({
  world,
  scene,
  label,
  onBack,
}: {
  world: string;
  scene: number;
  label: string;
  onBack: () => void;
}) {
  const [events, setEvents] = useState<TranscriptEvent[] | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    setEvents(null);
    setError("");
    invoke<TranscriptEvent[]>("read_transcript", { world, scene })
      .then(setEvents)
      .catch((reason) => setError(String(reason)));
  }, [world, scene]);

  async function exportScene() {
    setError("");
    try {
      const now = new Date();
      const pad = (n: number) => String(n).padStart(2, "0");
      const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}${pad(now.getMinutes())}`;
      const path = await save({
        defaultPath: `${t("sceneExportFileName", { table: world, n: scene + 1, stamp })}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await invoke("export_scene", { world, scene, path });
      await revealItemInDir(path);
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <>
      <div className="act-reader-header">
        <strong>{label}</strong>
        <button type="button" onClick={exportScene}>
          {t("exportScene")}
        </button>
        <button type="button" onClick={onBack}>
          {t("backToNow")}
        </button>
      </div>
      <section className="messages" aria-label={label}>
        {events === null ? (
          error && <p role="alert">{error}</p>
        ) : (
          events.map((event, index) => (
            <div key={index} className={`scene-event scene-event-${event.kind}`}>
              {(event.kind === "dialogue" || event.kind === "player") && (
                <span className="speaker">{event.speaker}</span>
              )}
              <span className="text">{event.text}</span>
            </div>
          ))
        )}
      </section>
      {error && events !== null && <p role="alert">{error}</p>}
    </>
  );
}

// 首開先選語言再建範例桌（sample-world-i18n 拍板）：預選跟系統語系走，選單即選即換介面語言
function FirstRun({ onStart }: { onStart: (lang: Lang) => void }) {
  const [choice, setChoice] = useState<Lang>(() =>
    navigator.language.toLowerCase().startsWith("zh") ? "zh-TW" : "en",
  );
  setLang(choice);

  return (
    <main className="container first-run">
      <h1>{t("firstRunTitle")}</h1>
      <p>{t("firstRunIntro")}</p>
      <select
        aria-label={t("languageLabel")}
        value={choice}
        onChange={(e) => setChoice(normalizeLang(e.currentTarget.value))}
      >
        {LANGUAGE_OPTIONS.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      <button onClick={() => onStart(choice)}>{t("firstRunStart")}</button>
    </main>
  );
}

function App() {
  const [worlds, setWorlds] = useState<string[]>([]);
  const [table, setTable] = useState("");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  const activeCharacters = characters.filter((character) => !character.archived);
  const archivedCharacters = characters.filter((character) => character.archived);
  // 角色圖快取：name → data URL（來源是匯入時存下的原 PNG，後端 read_character_image）
  const [characterImages, setCharacterImages] = useState<Record<string, string>>({});
  const [characterAvatars, setCharacterAvatars] = useState<Record<string, string>>({});
  const [characterName, setCharacterName] = useState("");
  const [speaker, setSpeaker] = useState("");
  const [scene, setScene] = useState(0);
  const [sceneTitles, setSceneTitles] = useState<Record<string, string>>({});
  const [events, setEvents] = useState<TranscriptEvent[]>([]);
  const [input, setInput] = useState("");
  // 逐角色打字指示：狀態帶「是誰在生成、以哪種形式」，不做全域單一指示燈（NewPlan §9.2）
  const [generating, setGenerating] = useState<{
    name: string;
    kind: "dialogue" | "narration";
  } | null>(null);
  const [streamText, setStreamText] = useState("");
  const [editingName, setEditingName] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  // 主欄下半部（messages＋composer）三選一整面取代：單幕閱讀／角色卡編輯／GM 世界設定編輯
  // （使用者拍板改版：需求 4 不用 modal，與需求 3 單幕閱讀同一套「整面取代」模式）
  const [mainView, setMainView] = useState<
    { kind: "scene"; n: number } | { kind: "character"; name: string } | { kind: "world" } | null
  >(null);
  // 前幕清單浮層：只是開關狀態，不佔版面高度（NewPlan §9.4 主欄閱讀優先改造）
  const [actsOpen, setActsOpen] = useState(false);
  const [firstRun, setFirstRun] = useState(false);
  const [error, setError] = useState("");
  const [sidebarWidth, setSidebarWidth] = useState(
    () => Number(localStorage.getItem(SIDEBAR_WIDTH_KEY)) || SIDEBAR_DEFAULT_WIDTH,
  );
  const [tableListOpen, setTableListOpen] = useState(
    () => localStorage.getItem(TABLE_LIST_OPEN_KEY) !== "false",
  );
  const bottomRef = useRef<HTMLDivElement>(null);
  const importInputRef = useRef<HTMLInputElement>(null);

  async function loadCharacterImages(world: string, cast: CharacterMeta[]) {
    const entries = await Promise.all(
      cast.map(async (c) => {
        const [image, avatar] = await Promise.all([
          invoke<string | null>("read_character_image", { world, name: c.name }).catch(() => null),
          invoke<string | null>("read_character_avatar", { world, name: c.name }).catch(() => null),
        ]);
        return [c.name, image, avatar] as const;
      }),
    );
    setCharacterImages(
      Object.fromEntries(
        entries.filter(([, image]) => image !== null).map(([name, image]) => [name, `data:image/png;base64,${image}`]),
      ),
    );
    setCharacterAvatars(
      Object.fromEntries(
        entries.filter(([, , avatar]) => avatar !== null).map(([name, , avatar]) => [name, `data:image/png;base64,${avatar}`]),
      ),
    );
  }

  // 語系跟著 config 走；render 前同步進 i18n 模組，之後子樹的 t() 都拿到正確語言
  const language = normalizeLang(config?.preferences["language"]);
  setLang(language);

  // 外觀類偏好（語言、文字大小）：改了立即生效並寫回 config，不設儲存鈕
  async function changePreference(key: string, value: unknown) {
    if (!config) return;
    const updated = { ...config, preferences: { ...config.preferences, [key]: value } };
    setConfig(updated);
    try {
      await invoke("write_config", { config: updated });
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 串流期間 config 可能已被設定頁改寫，走 ref 取最新值，避免舊閉包蓋掉剛存的設定
  const chatConfigRef = useRef(config);
  chatConfigRef.current = config;
  async function markCliConnectedFromChat() {
    const current = chatConfigRef.current;
    if (!current) return;
    const transport = current.preferences["transport"];
    if (!CLI_IDS.includes(transport as (typeof CLI_IDS)[number]) || current.preferences[cliConnectedKey(String(transport))] === true) {
      return;
    }
    const updated = {
      ...current,
      preferences: { ...current.preferences, [cliConnectedKey(String(transport))]: true },
    };
    try {
      await invoke("write_config", { config: updated });
      setConfig(updated);
    } catch (reason) {
      setError(String(reason));
    }
  }

  const textSize = String(config?.preferences["text_size"] ?? TEXT_SIZE_DEFAULT);
  useEffect(() => {
    document.documentElement.style.fontSize =
      TEXT_SIZE_PX[textSize] ?? TEXT_SIZE_PX[TEXT_SIZE_DEFAULT];
  }, [textSize]);

  // 主題不跟系統走（2026-07-25 使用者拍板）：config.preferences.theme 寫在 <html data-theme>，預設深色
  const theme = String(config?.preferences["theme"] ?? "dark");
  useEffect(() => {
    document.documentElement.dataset.theme = theme === "light" ? "light" : "dark";
  }, [theme]);

  // 開 App 直接回上次那桌；一桌都沒有就默默開一桌，零精靈（NewPlan §9.3）
  useEffect(() => {
    (async () => {
      try {
        const [names, loaded] = await Promise.all([
          invoke<string[]>("list_worlds"),
          invoke<AppConfig>("read_config"),
        ]);
        setConfig(loaded);
        if (names.length === 0) {
          // 首開（沒有任何桌也沒選過語言）：先讓使用者選語言，選完才建範例桌
          if (loaded.preferences["language"] === undefined) {
            setFirstRun(true);
            return;
          }
          const name = await invoke<string>("create_sample_world", {
            lang: normalizeLang(loaded.preferences["language"]),
          });
          setWorlds([name]);
          await enterTable(name, loaded);
          return;
        }
        setWorlds(names);
        const last = String(loaded.preferences["last_world"] ?? "");
        await enterTable(names.includes(last) ? last : names[0], loaded);
      } catch (reason) {
        setError(String(reason));
      }
    })();
  }, []);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [events, generating, streamText]);

  async function enterTable(name: string, loaded: AppConfig) {
    const state = await invoke<WorldState>("read_state", { world: name });
    const transcript = await invoke<TranscriptEvent[]>("read_transcript", {
      world: name,
      scene: state.current_scene,
    });
    const cast = await invoke<CharacterMeta[]>("list_characters", { world: name });
    setTable(name);
    setScene(state.current_scene);
    setSceneTitles(state.scene_titles ?? {});
    setEvents(transcript);
    setCharacters(cast);
    await loadCharacterImages(name, cast);
    setSpeaker(cast.find((character) => !character.archived)?.name ?? "");
    setEditingName(null);
    // 切桌就離開單幕閱讀／編輯畫面與前幕浮層，避免殘留上一桌的狀態
    setMainView(null);
    setActsOpen(false);
    if (loaded.preferences["last_world"] !== name) {
      const next = { ...loaded, preferences: { ...loaded.preferences, last_world: name } };
      await invoke("write_config", { config: next });
      setConfig(next);
    }
  }

  async function switchTable(name: string) {
    if (!config || name === table || generating !== null) return;
    setError("");
    try {
      const previous = table;
      await enterTable(name, config);
      // 空桌（零訊息、零角色、無設定）離開時自動回收（NewPlan §9.3）
      if (previous) await invoke("reclaim_world_if_empty", { world: previous });
      setWorlds(await invoke<string[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function newTable() {
    if (!config || generating !== null) return;
    setError("");
    try {
      const existing = await invoke<string[]>("list_worlds");
      const base = t("newTableName");
      let name = base;
      for (let n = 2; existing.includes(name); n += 1) name = `${base} ${n}`;
      await invoke("create_world", { name });
      const previous = table;
      await enterTable(name, config);
      if (previous) await invoke("reclaim_world_if_empty", { world: previous });
      setWorlds(await invoke<string[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function renameTable(raw: string) {
    const name = raw.trim();
    setEditingName(null);
    if (!config || !name || name === table) return;
    setError("");
    try {
      await invoke("rename_world", { world: table, newName: name });
      setTable(name);
      const next = { ...config, preferences: { ...config.preferences, last_world: name } };
      await invoke("write_config", { config: next });
      setConfig(next);
      setWorlds(await invoke<string[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 換場：把目前場景公開紀錄壓成一則前情提要，寫進新場景開頭，current_scene +1
  async function advanceScene() {
    setError("");
    setGenerating({ name: "GM", kind: "narration" });
    setStreamText("");
    try {
      await invoke<number>("advance_scene", { world: table });
      await enterTable(table, config!);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }

  // 存哪裡由使用者決定：跳原生「另存新檔」對話框，取消就什麼都不做
  async function exportTranscript() {
    setError("");
    try {
      const now = new Date();
      const pad = (n: number) => String(n).padStart(2, "0");
      const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}${pad(now.getMinutes())}`;
      const path = await save({
        defaultPath: `${t("exportFileName", { table, stamp })}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await invoke("export_transcript", { world: table, path });
      await revealItemInDir(path);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function createCharacter(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError("");
    const name = characterName.trim();
    if (name === "GM" || name === "玩家") {
      setError(t("reservedNameError"));
      return;
    }
    const card: CharacterCard = {
      name,
      color: PALETTE[characters.length % PALETTE.length],
      avatar: "🎭",
      tier: "default",
      show_image: true,
      archived: false,
      public_md: "",
      private_md: "",
    };
    try {
      await invoke("write_character", { world: table, card });
      setCharacters(await invoke<CharacterMeta[]>("list_characters", { world: table }));
      setSpeaker(card.name);
      setCharacterName("");
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 匯入 SillyTavern 角色卡（V2 PNG 或 JSON）：讀 bytes 交後端解析，顏色沿用建卡輪選
  async function importCharacter(file: File) {
    setError("");
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const meta = await invoke<CharacterMeta>("import_character", {
        world: table,
        data: Array.from(bytes),
        color: PALETTE[characters.length % PALETTE.length],
      });
      const cast = await invoke<CharacterMeta[]>("list_characters", { world: table });
      setCharacters(cast);
      await loadCharacterImages(table, cast);
      setSpeaker(meta.name);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function refreshCharacters() {
    const cast = await invoke<CharacterMeta[]>("list_characters", { world: table });
    setCharacters(cast);
    await loadCharacterImages(table, cast);
    return cast;
  }

  async function finishArchiving(name: string) {
    const cast = await refreshCharacters();
    if (speaker === name) {
      setSpeaker(cast.find((character) => !character.archived)?.name ?? "");
    }
    setMainView(null);
  }

  async function restoreCharacter(name: string) {
    setError("");
    try {
      await invoke("set_character_archived", { world: table, name, archived: false });
      await refreshCharacters();
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function deleteArchivedCharacter(name: string) {
    setError("");
    try {
      const accepted = await confirm(t("deleteCharacterConfirm", { name }), {
        title: t("deleteCharacterTitle"),
        kind: "warning",
      });
      if (!accepted) return;
      await invoke("delete_character", { world: table, name });
      await refreshCharacters();
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function appendEvent(event: TranscriptEvent) {
    await invoke("append_transcript", { world: table, scene, event });
    setEvents((previous) => [...previous, event]);
  }

  // 單次角色接話（不含 busy 防護），供手動點名與 GM 推進共用；失敗往外拋由呼叫端收尾
  async function replyOnce(character: string) {
    setGenerating({ name: character, kind: "dialogue" });
    setStreamText("");
    const onDelta = new Channel<string>();
    onDelta.onmessage = (delta) => setStreamText((previous) => previous + delta);
    const full = await invoke<string>("chat_with_character", {
      world: table,
      character,
      onDelta,
    });
    await appendEvent({ ts: nowTs(), speaker: character, kind: "dialogue", text: full });
    await markCliConnectedFromChat();
  }

  // 點名指定角色接話；也是「請 X 發言」按鈕的入口（NewPlan §9、MVP 第 8 項）
  async function requestReply(character: string) {
    if (!character || generating !== null) return;
    setError("");
    try {
      await replyOnce(character);
      setWorlds(await invoke<string[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }

  // 簡易導演：GM 插入旁白（NewPlan §6.1、MVP 第 9 項）
  async function gmNarrate() {
    if (generating !== null) return;
    setError("");
    setGenerating({ name: "GM", kind: "narration" });
    setStreamText("");
    try {
      const onDelta = new Channel<string>();
      onDelta.onmessage = (delta) => setStreamText((previous) => previous + delta);
      const full = await invoke<string>("gm_narrate", { world: table, onDelta });
      await appendEvent({ ts: nowTs(), speaker: "GM", kind: "narration", text: full });
      await markCliConnectedFromChat();
      setWorlds(await invoke<string[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }

  // 簡易導演：GM 點名→角色接話的接力，至「玩家」哨兵或每回合上限停下（NewPlan §6.1）
  async function gmAdvance() {
    if (!config || generating !== null || activeCharacters.length === 0) return;
    setError("");
    const max = Math.max(1, Number(config.preferences["max_round_speakers"]) || 3);
    try {
      for (let turn = 0; turn < max; turn += 1) {
        setGenerating({ name: "GM", kind: "narration" });
        setStreamText("");
        const name = await invoke<string>("gm_suggest_speaker", { world: table });
        if (name === "玩家") break;
        await appendEvent({ ts: nowTs(), speaker: "GM", kind: "system", text: t("gmCallOn", { name }) });
        await replyOnce(name);
      }
      setWorlds(await invoke<string[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }

  async function send(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const text = input.trim();
    if (!text || !speaker || generating !== null) return;
    setError("");
    setInput("");
    try {
      await appendEvent({ ts: nowTs(), speaker: "玩家", kind: "player", text });
    } catch (reason) {
      setError(String(reason));
      return;
    }
    await requestReply(speaker);
  }

  const metaOf = (name: string) => characters.find((c) => c.name === name);

  // 幕的顯示標籤：有取到幕名就「第 n 幕：幕名」，沒有就沿用「第 n 幕」；n 從 1 起算，內部場號 0 起算
  const sceneDisplayLabel = (n: number) => {
    const title = sceneTitles[String(n)];
    return title ? t("sceneWithTitle", { n: n + 1, title }) : t("sceneLabel", { n: n + 1 });
  };
  const generatingMeta = generating !== null ? metaOf(generating.name) : undefined;

  async function startFirstRun(lang: Lang) {
    if (!config) return;
    setError("");
    try {
      const updated = { ...config, preferences: { ...config.preferences, language: lang } };
      await invoke("write_config", { config: updated });
      setConfig(updated);
      const name = await invoke<string>("create_sample_world", { lang });
      setWorlds([name]);
      await enterTable(name, updated);
      setFirstRun(false);
    } catch (reason) {
      setError(String(reason));
    }
  }

  if (firstRun && config) {
    return (
      <>
        <FirstRun onStart={(lang) => void startFirstRun(lang)} />
        {error && <p role="alert">{error}</p>}
      </>
    );
  }

  if (!config || !table) {
    return <main className="container">{error && <p role="alert">{error}</p>}</main>;
  }

  // 換場提醒：粗估目前場景累計字元數，超過門檻就在換場鈕旁小字提醒（不擋操作）
  const sceneTooLong =
    events.reduce((sum, event) => sum + event.text.length, 0) > SCENE_LENGTH_HINT_CHARS;

  // 拖曳分隔線調側欄寬度：上限由 CSS max-width 夾住，這裡只擋下限
  function resizeSidebar(next: number) {
    const clamped = Math.min(Math.max(next, SIDEBAR_MIN_WIDTH), window.innerWidth / 2);
    setSidebarWidth(clamped);
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(Math.round(clamped)));
  }

  function startSidebarResize(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const onMove = (moveEvent: PointerEvent) => resizeSidebar(moveEvent.clientX);
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" style={{ width: sidebarWidth }}>
        <details
          className="table-section"
          open={tableListOpen}
          onToggle={(event) => {
            const next = event.currentTarget.open;
            setTableListOpen(next);
            localStorage.setItem(TABLE_LIST_OPEN_KEY, String(next));
          }}
        >
          <summary>{t("tableListAria")}</summary>
          <div className="table-section-content">
            <button className="new-table" onClick={newTable} disabled={generating !== null}>
              {t("newTable")}
            </button>
            <nav className="table-list" aria-label={t("tableListAria")}>
              {worlds.map((name) => (
                <button
                  key={name}
                  className={`table-item ${name === table ? "table-item-active" : ""}`}
                  onClick={() => switchTable(name)}
                >
                  {name}
                </button>
              ))}
            </nav>
          </div>
        </details>
        <section className="character-panel" aria-label={t("castAria")}>
          <div className="character-list">
            {/* GM 列：系統機件壓成一行（ui-overhaul 拍板）；整列點擊開世界設定＋世界書，不可選為發言對象 */}
            <div
              role="button"
              tabIndex={0}
              className="gm-row"
              title={t("worldSummary")}
              onClick={() => setMainView({ kind: "world" })}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  setMainView({ kind: "world" });
                }
              }}
            >
              <span aria-hidden="true">🎲</span>
              <b>GM</b>
              <span className="gm-cfg" aria-hidden="true">
                ⚙
              </span>
            </div>
            {/* 角色卡＝桌遊組件卡：圖窗＋名字 wedge＋檔位寶石（tier 是既有欄位；「跟隨預設」不掛寶石） */}
            {activeCharacters.map((c) => (
              <div
                key={c.name}
                role="button"
                tabIndex={0}
                className={`tcard ${speaker === c.name ? "tcard-selected" : ""}`}
                style={{ ["--fac" as string]: c.color }}
                onClick={() => setSpeaker(c.name)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    setSpeaker(c.name);
                  }
                }}
                title={t("castHint", { name: c.name })}
              >
                <span className="tcard-art">
                  {c.show_image && characterImages[c.name] ? (
                    <img className="tcard-image" src={characterImages[c.name]} alt="" />
                  ) : characterAvatars[c.name] ? (
                    <img className="avatar-round tcard-avatar" src={characterAvatars[c.name]} alt="" />
                  ) : (
                    <span aria-hidden="true">{c.avatar}</span>
                  )}
                </span>
                <span className="tcard-body">
                  <span className="tcard-name-row">
                    <span className="tcard-plate">{c.name}</span>
                    {c.tier !== "default" && <span className="tcard-gem">{tierLabel(c.tier)}</span>}
                  </span>
                </span>
                <button
                  type="button"
                  className="character-card-edit"
                  aria-label={t("editCardSummary", { name: c.name })}
                  title={t("editCardSummary", { name: c.name })}
                  onClick={(event) => {
                    event.stopPropagation();
                    setMainView({ kind: "character", name: c.name });
                  }}
                >
                  {t("editBtn")}
                </button>
              </div>
            ))}
          </div>
          {archivedCharacters.length > 0 && (
            <details className="archive-section">
              <summary>{t("archiveSectionTitle")}</summary>
              <div className="archive-list">
                {archivedCharacters.map((character) => (
                  <div className="archive-row" key={character.name}>
                    <span>{character.name}</span>
                    <button type="button" onClick={() => void restoreCharacter(character.name)}>
                      {t("restoreCharacter")}
                    </button>
                    <button
                      type="button"
                      className="delete-character"
                      onClick={() => void deleteArchivedCharacter(character.name)}
                    >
                      {t("deleteCharacter")}
                    </button>
                  </div>
                ))}
              </div>
            </details>
          )}
          <form className="character-create" onSubmit={createCharacter}>
            <input
              aria-label={t("newCharacterAria")}
              value={characterName}
              onChange={(e) => setCharacterName(e.currentTarget.value)}
              placeholder={t("newCharacterPlaceholder")}
            />
            <button type="submit">{t("createCard")}</button>
            <button
              type="button"
              title={t("importCardHint")}
              onClick={() => importInputRef.current?.click()}
            >
              {t("importCard")}
            </button>
            <input
              ref={importInputRef}
              type="file"
              accept=".png,.json,image/png,application/json"
              hidden
              onChange={(e) => {
                const file = e.currentTarget.files?.[0];
                e.currentTarget.value = "";
                if (file) void importCharacter(file);
              }}
            />
          </form>
        </section>
        <div className="sidebar-footer">
          <button className="settings-open" onClick={() => setSettingsOpen(true)}>
            ⚙️ {t("settingsBtn")}
          </button>
        </div>
      </aside>

      {settingsOpen && (
        <SettingsWindow
          config={config}
          onSaved={setConfig}
          onPreference={(key, value) => void changePreference(key, value)}
          onClose={() => setSettingsOpen(false)}
        />
      )}

      <div
        className="sidebar-resizer"
        role="separator"
        aria-orientation="vertical"
        aria-label={t("sidebarResizerAria")}
        aria-valuenow={Math.round(sidebarWidth)}
        tabIndex={0}
        onPointerDown={startSidebarResize}
        onKeyDown={(e) => {
          if (e.key === "ArrowLeft") resizeSidebar(sidebarWidth - SIDEBAR_KEY_STEP);
          if (e.key === "ArrowRight") resizeSidebar(sidebarWidth + SIDEBAR_KEY_STEP);
        }}
        onDoubleClick={() => resizeSidebar(SIDEBAR_DEFAULT_WIDTH)}
      />

      <main className="chat-main">
        <header className="chat-header">
          {editingName === null ? (
            <button
              className="table-title"
              title={t("renameHint")}
              onClick={() => setEditingName(table)}
            >
              {table}
            </button>
          ) : (
            <input
              className="table-title-input"
              autoFocus
              value={editingName}
              aria-label={t("tableNameAria")}
              onChange={(e) => setEditingName(e.currentTarget.value)}
              onBlur={() => renameTable(editingName)}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
                if (e.key === "Escape") setEditingName(null);
              }}
            />
          )}
          <div className="chat-header-actions">
            {sceneTooLong && <span className="scene-length-hint">{t("sceneTooLongHint")}</span>}
            <button
              type="button"
              title={t("sceneAdvanceHint")}
              aria-label={t("sceneAdvance")}
              disabled={generating !== null || events.length === 0}
              onClick={advanceScene}
            >
              {t("sceneAdvance")}
            </button>
            <button
              type="button"
              title={t("exportTranscriptHint")}
              aria-label={t("exportTranscript")}
              onClick={exportTranscript}
            >
              {t("exportTranscript")}
            </button>
            {scene > 0 && (
              <button type="button" onClick={() => setActsOpen((open) => !open)}>
                {t("pastScenes", { count: scene })}
              </button>
            )}
          </div>
        </header>

        <div className="chat-body">
        {actsOpen && scene > 0 && (
          <div className="acts-flyout">
            <button type="button" className="acts-flyout-hide" onClick={() => setActsOpen(false)}>
              {t("hideActs")}
            </button>
            <div className="acts-flyout-list">
              {Array.from({ length: scene }, (_, n) => n).map((n) => (
                <button
                  key={n}
                  type="button"
                  onClick={() => {
                    setMainView({ kind: "scene", n });
                    setActsOpen(false);
                  }}
                >
                  {sceneDisplayLabel(n)}
                </button>
              ))}
            </div>
          </div>
        )}
        {mainView?.kind === "scene" ? (
          <ActReader
            world={table}
            scene={mainView.n}
            label={sceneDisplayLabel(mainView.n)}
            onBack={() => setMainView(null)}
          />
        ) : mainView?.kind === "character" ? (
          <EditPane title={t("editCardSummary", { name: mainView.name })}>
            <CardEditor
              world={table}
              name={mainView.name}
              imageDataUrl={characterImages[mainView.name]}
              avatarImgUrl={characterAvatars[mainView.name]}
              onImagesChanged={() => loadCharacterImages(table, characters)}
              onSaved={() =>
                invoke<CharacterMeta[]>("list_characters", { world: table }).then(setCharacters)
              }
              onArchived={() => finishArchiving(mainView.name)}
              onBack={() => setMainView(null)}
            />
          </EditPane>
        ) : mainView?.kind === "world" ? (
          <EditPane title={t("worldSummary")}>
            <WorldEditor world={table} onBack={() => setMainView(null)} />
          </EditPane>
        ) : (
          <>
            <Onboarding config={config} onSaved={setConfig} />

            <section className="messages" aria-label={t("messagesAria")}>
              {/* 幕書籤：目前這一幕的既有系統標籤（換幕／前幕／單幕匯出同一套資料） */}
              <div className="act-divider">
                <span className="act-tag">{sceneDisplayLabel(scene)}</span>
              </div>
              {events.map((event, index) => {
                if (event.kind === "dialogue" || event.kind === "player") {
                  const meta = metaOf(event.speaker);
                  const isPlayer = event.kind === "player";
                  return (
                    <div
                      key={index}
                      className={`message message-${event.kind}`}
                      style={
                        isPlayer ? undefined : { ["--fac" as string]: meta?.color ?? "#888888" }
                      }
                    >
                      <div className="pb-name">
                        <span className="pb-plate">
                          {isPlayer ? t("playerLabel") : event.speaker}
                        </span>
                      </div>
                      <span className="text">{event.text}</span>
                    </div>
                  );
                }
                return (
                  <div key={index} className={`message message-${event.kind}`}>
                    <span className="text">{event.text}</span>
                  </div>
                );
              })}
              {generating !== null && generating.kind === "dialogue" && (
                <div
                  className="message message-dialogue"
                  style={{ ["--fac" as string]: generatingMeta?.color ?? "#888888" }}
                >
                  <div className="pb-name">
                    <span className="pb-plate">{generating.name}</span>
                  </div>
                  {streamText ? (
                    <span className="text">{streamText}</span>
                  ) : (
                    <span className="typing" aria-label={t("typing", { name: generating.name })}>
                      <i />
                      <i />
                      <i />
                    </span>
                  )}
                </div>
              )}
              {generating !== null && generating.kind === "narration" && (
                <div className="message message-narration">
                  {streamText ? (
                    <span className="text">{streamText}</span>
                  ) : (
                    <span className="typing" aria-label={t("typing", { name: "GM" })}>
                      <i />
                      <i />
                      <i />
                    </span>
                  )}
                </div>
              )}
              <div ref={bottomRef} />
            </section>

            {/* Composer 改整寬書寫面（ui-overhaul 拍板）：目標晶片只是把「點側欄選發言對象」既有狀態可見化 */}
            <form className="composer" onSubmit={send}>
              {speaker && (
                <div className="composer-opts">
                  <span
                    className="opt-target"
                    title={t("castHint", { name: speaker })}
                    style={{ ["--fac" as string]: metaOf(speaker)?.color ?? "#888888" }}
                  >
                    {characterAvatars[speaker] ? (
                      <img className="avatar-round opt-avatar" src={characterAvatars[speaker]} alt="" />
                    ) : (
                      <span aria-hidden="true">{metaOf(speaker)?.avatar ?? "🎭"}</span>
                    )}
                    {speaker}
                  </span>
                </div>
              )}
              <input
                className="writebox"
                aria-label={t("composerAria")}
                value={input}
                onChange={(e) => setInput(e.currentTarget.value)}
                placeholder={
                  speaker ? t("composerPlaceholder", { name: speaker }) : t("composerNoCharacter")
                }
                disabled={!speaker || generating !== null}
              />
              <div className="composer-send">
                <button
                  type="button"
                  onClick={() => requestReply(speaker)}
                  disabled={!speaker || generating !== null}
                  title={t("requestReplyHint")}
                >
                  {t("requestReplyBtn", { name: speaker || t("characterFallback") })}
                </button>
                <button
                  type="button"
                  onClick={gmNarrate}
                  disabled={generating !== null}
                  title={t("gmNarrateHint")}
                >
                  {t("gmNarrate")}
                </button>
                <button
                  type="button"
                  onClick={gmAdvance}
                  disabled={generating !== null || activeCharacters.length === 0}
                  title={t("gmAdvanceHint")}
                >
                  {t("gmAdvance")}
                </button>
                <span className="spacer" />
                <button type="submit" disabled={!speaker || generating !== null}>
                  {t("send")} ➤
                </button>
              </div>
            </form>
          </>
        )}
        </div>
        {error && <p role="alert">{error}</p>}
      </main>
    </div>
  );
}

export default App;
