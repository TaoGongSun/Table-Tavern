import { FormEvent, PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from "react";
import Cropper, { Area } from "react-easy-crop";
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { confirm, save } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { Lang, LANGUAGE_OPTIONS, normalizeLang, setLang, t } from "./i18n";
import taoIcon from "./assets/tao-icon.png";
import gmBook from "./assets/gm-book.png";
import "./App.css";

const KOFI_URL = "https://ko-fi.com/taogongsun";
const GALLERY_PAGE_SIZE = 12;
// 主題清單：free 兩套隨點隨存；sponsor 五套未解鎖只能試看（關設定視窗即復原）
const FREE_THEMES = ["dark", "light"] as const;
const SPONSOR_THEMES = ["parchment", "herbal", "candlelight", "port", "seamist"] as const;
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
const ALL_THEMES = [...FREE_THEMES, ...SPONSOR_THEMES] as const;
type ThemeId = (typeof ALL_THEMES)[number];

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
  gen_prompt: string;
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

// 主題不跟系統走（2026-07-25 使用者拍板）：config.preferences.theme 寫在 <html data-theme>，預設深色
function resolveTheme(config: AppConfig | null | undefined): ThemeId {
  const theme = String(config?.preferences["theme"] ?? "dark");
  if (!ALL_THEMES.includes(theme as ThemeId)) return "dark";
  if (
    (SPONSOR_THEMES as readonly string[]).includes(theme) &&
    config?.preferences["sponsor_unlocked"] !== true
  ) {
    return "dark";
  }
  return theme as ThemeId;
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

// 裁切完成的圖：bytes 給後端存檔、url 給畫面預覽（按儲存前只活在記憶體裡）
type DraftImage = { bytes: number[]; url: string };

// 角色圖示快捷選項；輸入框沒限制在這幾個，系統 emoji 鍵盤打什麼都行
const AVATAR_EMOJIS = ["🎭", "🧙", "🗡️", "🏹", "🛡️", "🐺", "🦊", "🐉", "👑", "💀", "🌙", "🕯️"];
const DEFAULT_AVATAR = "🎭";
const AVATAR_MAX_CHARS = 4;

// 以「看得到的字元」為單位截斷：input 的 maxLength 算的是 UTF-16 單元，
// 一顆 🗡️ 就佔 3 個，拿來限長會讓 emoji 只打得下一顆。
function clampChars(value: string, max: number) {
  const chars =
    typeof Intl.Segmenter === "function"
      ? Array.from(new Intl.Segmenter().segment(value), (unit) => unit.segment)
      : Array.from(value);
  return chars.slice(0, max).join("");
}

// 角色名檢查（建卡／改名共用）：規則對齊後端 validate_name（data.rs:197），
// 前端先擋才不會讓使用者看到 `invalid name: ""` 這種內部訊息。空名由送出鈕 disabled 處理。
// taken 傳完整名單（含收起的卡），同名會直接覆寫既有卡片。
function characterNameError(name: string, taken: string[]): string | null {
  if (name === "GM" || name === "玩家") return t("reservedNameError");
  if (
    name.startsWith(".") ||
    name.includes("..") ||
    name.includes("/") ||
    name.includes("\\") ||
    /[\u0000-\u001f\u007f]/.test(name)
  ) {
    return t("invalidNameError");
  }
  if (taken.includes(name)) return t("duplicateNameError");
  return null;
}

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

// 偵測結果跨畫面／跨設定頁開關快取：null＝本次啟動還沒偵測過。
// 設定頁重開時先吃上次結果，不再讓使用者重看一次「偵測中」。
let cliCache: CliInfo[] | null = null;

async function detectClis(): Promise<CliInfo[]> {
  const detected = await invoke<CliInfo[]>("detect_clis");
  cliCache = detected;
  return detected;
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
  const [imageModel, setImageModel] = useState(String(config.preferences["image_model"] ?? ""));
  const [claudeCompatBaseUrl, setClaudeCompatBaseUrl] = useState(
    String(config.preferences["claude_base_url"] ?? ""),
  );
  const [claudeCompatKey, setClaudeCompatKey] = useState(config.api_keys["claude_compat"] ?? "");
  const [gmTier, setGmTier] = useState(String(config.preferences["gm_tier"] ?? "best"));
  const [maxRound, setMaxRound] = useState(String(config.preferences["max_round_speakers"] ?? 3));
  const [transport, setTransport] = useState(String(config.preferences["transport"] ?? "api"));
  const [riskAccepted, setRiskAccepted] = useState(config.preferences["cli_risk_accepted"] === true);
  const [clis, setClis] = useState<CliInfo[] | null>(cliCache);
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
    detectClis().then(setClis).catch(() => setClis([]));
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
          void detectClis().then(setClis).catch(() => {});
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
      detectClis()
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
    imageModel.trim() !== String(config.preferences["image_model"] ?? ""),
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
        image_model: imageModel.trim(),
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
            // clis === null＝偵測還沒回來，與「偵測不到」是兩回事：此時不給按鈕，避免誤按一鍵安裝
            const detecting = clis === null;
            const found = clis?.find((c) => c.id === id);
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
                {detecting ? (
                  <span className="cli-version" role="status">
                    {t("cliDetecting")}
                  </span>
                ) : found ? (
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
            <label>
              {t("imageModelLabel")}
              <input value={imageModel} onChange={(e) => setImageModel(e.currentTarget.value)} />
            </label>
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
  initialTab = "appearance",
}: {
  config: AppConfig;
  onSaved: (c: AppConfig) => void;
  onPreference: (key: string, value: unknown) => void;
  onClose: () => void;
  initialTab?: "appearance" | "ai" | "author";
}) {
  const [tab, setTab] = useState<"appearance" | "ai" | "author">(initialTab);
  const [previewTheme, setPreviewTheme] = useState<ThemeId | null>(null);
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

  async function switchTab(target: "appearance" | "author") {
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
  const sponsorUnlocked = config.preferences["sponsor_unlocked"] === true;
  const selectedTheme = previewTheme ?? resolveTheme(config);

  useEffect(() => {
    document.documentElement.dataset.theme = previewTheme ?? resolveTheme(config);
    return () => {
      document.documentElement.dataset.theme = resolveTheme(config);
    };
  }, [previewTheme, config]);

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
        ) : tab === "author" ? (
          <div className="author-page">
            <img src={taoIcon} alt="TaoGongSun" className="avatar-round author-avatar" />
            <strong>TaoGongSun</strong>
            <p className="author-blurb">{t("authorBlurb")}</p>
            <button type="button" onClick={() => void openUrl(KOFI_URL)}>
              {t("sponsorBtn")}
            </button>
          </div>
        ) : (
          <Settings config={config} onSaved={onSaved} onDirty={setDirtyCount} />
        )}
      </div>
    </div>
  );
}

// 拖曳排序：按住移動超過門檻才算拖曳，門檻內放開仍是單純點擊（角色卡的點擊＝選發言者）
const DRAG_THRESHOLD_PX = 5;

function useDragReorder<T>(
  items: T[],
  keyOf: (item: T) => string,
  onReorder: (ordered: T[]) => void,
) {
  const [preview, setPreview] = useState<T[] | null>(null);
  const [draggingKey, setDraggingKey] = useState<string | null>(null);
  const rows = useRef(new Map<string, HTMLElement>());
  const dragged = useRef(false);

  // 一次只跟相鄰那列交換：越過鄰居中線就換，換完中線落到指標另一側，高度不一也不會來回抖
  function neighbourStep(y: number, order: T[], from: number): number {
    const midpoint = (index: number) => {
      const item = order[index];
      const row = item === undefined ? undefined : rows.current.get(keyOf(item));
      if (!row) return null;
      const rect = row.getBoundingClientRect();
      return rect.top + rect.height / 2;
    };
    const above = midpoint(from - 1);
    if (above !== null && y < above) return from - 1;
    const below = midpoint(from + 1);
    if (below !== null && y > below) return from + 1;
    return from;
  }

  function startDrag(event: ReactPointerEvent, item: T) {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("button, a, input, textarea, select")) return;
    const key = keyOf(item);
    const startY = event.clientY;
    let order = items;
    let started = false;

    const move = (moveEvent: globalThis.PointerEvent) => {
      if (!started) {
        if (Math.abs(moveEvent.clientY - startY) < DRAG_THRESHOLD_PX) return;
        started = true;
        dragged.current = true;
        setDraggingKey(key);
      }
      const from = order.findIndex((candidate) => keyOf(candidate) === key);
      const target = neighbourStep(moveEvent.clientY, order, from);
      if (target === from) return;
      const next = order.slice();
      const [moved] = next.splice(from, 1);
      next.splice(target, 0, moved);
      order = next;
      setPreview(next);
    };
    const finish = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      if (started) {
        onReorder(order);
        // 放開後瀏覽器才補送 click，等它送完再解旗標
        setTimeout(() => (dragged.current = false), 0);
      }
      setDraggingKey(null);
      setPreview(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
  }

  return {
    order: preview ?? items,
    draggingKey,
    justDragged: () => dragged.current,
    rowProps: (item: T) => ({
      onPointerDown: (event: ReactPointerEvent) => startDrag(event, item),
      ref: (element: HTMLElement | null) => {
        if (element) rows.current.set(keyOf(item), element);
        else rows.current.delete(keyOf(item));
      },
    }),
  };
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
  const entryDrag = useDragReorder(
    entries,
    (entry) => String(entry.uid),
    (ordered) => void reorderEntries(ordered),
  );

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

  async function reorderEntries(ordered: WorldbookEntry[]) {
    setWorldbookMessage("");
    const previous = entries;
    setEntries(ordered);
    try {
      await invoke("reorder_worldbook_entries", {
        world,
        uids: ordered.map((entry) => entry.uid),
      });
    } catch (reason) {
      setEntries(previous);
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
            {entryDrag.order.map((entry) => (
              <div
                className={`worldbook-row${entry.disabled ? " worldbook-row-disabled" : ""}${
                  entryDrag.draggingKey === String(entry.uid) ? " row-dragging" : ""
                }`}
                key={entry.uid}
                title={t("dragToReorder")}
                {...entryDrag.rowProps(entry)}
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
  onConfirm: (image: DraftImage) => Promise<void>;
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
      // bytes 給存檔用、url 給暫存預覽用（圖像按儲存才落地）
      await onConfirm({
        bytes: Array.from(new Uint8Array(await blob.arrayBuffer())),
        url: canvas.toDataURL("image/png"),
      });
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
  takenNames,
  newCardColor,
  imageDataUrl,
  avatarImgUrl,
  onImagesChanged,
  onSaved,
  onArchived,
  onDeleted,
  onBack,
  config,
  onPreference,
  onOpenAiSettings,
}: {
  world: string;
  /** null＝建新卡的空白草稿（名字在表單裡填，按儲存才落地） */
  name: string | null;
  takenNames: string[];
  newCardColor: string;
  imageDataUrl?: string;
  avatarImgUrl?: string;
  onImagesChanged: () => Promise<void>;
  onBack: () => void;
  onSaved: (name: string) => void;
  onArchived: () => Promise<void>;
  onDeleted: () => Promise<void>;
  config: AppConfig;
  onPreference: (key: string, value: unknown) => Promise<void>;
  onOpenAiSettings: () => void;
}) {
  const [card, setCard] = useState<CharacterCard | null>(null);
  const [savedCardJson, setSavedCardJson] = useState("");
  // 圖像操作一律暫存，按儲存才落地（2026-07-27 使用者拍板）：
  // undefined＝沒動過（沿用 props 的已存檔圖）、null＝已標記移除、物件＝待存的新圖
  const [draftImage, setDraftImage] = useState<DraftImage | null | undefined>(undefined);
  const [draftAvatar, setDraftAvatar] = useState<DraftImage | null | undefined>(undefined);
  const [message, setMessage] = useState("");
  const [pendingImage, setPendingImage] = useState<string | null>(null);
  const [croppingAvatar, setCroppingAvatar] = useState(false);
  const [lightboxOpen, setLightboxOpen] = useState(false);
  const [aiGenOpen, setAiGenOpen] = useState(false);
  const [aiGenLockedOpen, setAiGenLockedOpen] = useState(false);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiSource, setAiSource] = useState("api");
  const [aiClis, setAiClis] = useState<CliInfo[]>([]);
  const [aiGenerating, setAiGenerating] = useState(false);
  const [aiGenError, setAiGenError] = useState("");
  const [galleryFiles, setGalleryFiles] = useState<string[]>([]);
  const [galleryImages, setGalleryImages] = useState<Record<string, string>>({});
  const [galleryLoaded, setGalleryLoaded] = useState(0);

  useEffect(() => {
    setMessage("");
    setDraftImage(undefined);
    setDraftAvatar(undefined);
    if (name === null) {
      const blank: CharacterCard = {
        name: "",
        color: newCardColor,
        avatar: DEFAULT_AVATAR,
        tier: "default",
        show_image: true,
        archived: false,
        public_md: "",
        private_md: "",
        gen_prompt: "",
      };
      setCard(blank);
      setSavedCardJson(JSON.stringify(blank));
      return;
    }
    invoke<CharacterCard>("read_character", { world, name })
      .then((loaded) => {
        setCard(loaded);
        setSavedCardJson(JSON.stringify(loaded));
      })
      .catch((reason) => setMessage(String(reason)));
  }, [world, name, newCardColor]);

  // 生圖與圖庫的檔案身分：既有卡片一律用已存檔的名字（改名未存檔時仍指向舊資料夾，
  // 存檔時才由 rename_character 整包搬走）；新卡草稿用當下填的名字
  const galleryName = name ?? card?.name.trim() ?? "";
  const sponsorUnlocked = config.preferences["sponsor_unlocked"] === true;
  const trialsUsed = Number(config.preferences["ai_image_trials_used"] ?? 0);
  const sourceOptions = ["api", ...aiClis.map((cli) => cli.id)];

  async function loadGalleryPage(files: string[], start: number) {
    const page = files.slice(start, start + GALLERY_PAGE_SIZE);
    const images = await Promise.all(page.map(async (file) => [file, await invoke<string>("read_gallery_image", { world, name: galleryName, file })] as const));
    setGalleryImages((current) => ({ ...current, ...Object.fromEntries(images) }));
    setGalleryLoaded(Math.min(start + page.length, files.length));
  }

  async function refreshGallery() {
    const files = await invoke<string[]>("list_gallery_images", { world, name: galleryName });
    setGalleryFiles(files);
    setGalleryImages({});
    setGalleryLoaded(0);
    await loadGalleryPage(files, 0);
  }

  function openAiGenerator() {
    if (!sponsorUnlocked && trialsUsed >= 3) {
      setAiGenLockedOpen(true);
      return;
    }
    const savedSource = String(config.preferences["image_source"] ?? "");
    const transport = String(config.preferences["transport"] ?? "api");
    void detectClis()
      .then((detected) => {
        setAiClis(detected);
        const detectedSources = ["api", ...detected.map((cli) => cli.id)];
        setAiSource(detectedSources.includes(savedSource) ? savedSource : transport);
      })
      .catch(() => {
        setAiClis([]);
        setAiSource(savedSource === "api" ? savedSource : transport);
      });
    setAiPrompt(card?.gen_prompt ?? "");
    setAiGenError("");
    setAiGenOpen(true);
    void refreshGallery().catch(() => {
      setGalleryFiles([]);
      setGalleryImages({});
      setGalleryLoaded(0);
    });
  }

  async function generateImage() {
    setAiGenerating(true);
    setAiGenError("");
    try {
      await invoke<string>("generate_character_image", {
        world,
        name: galleryName,
        description: card?.public_md ?? "",
        extraPrompt: aiPrompt,
        source: aiSource,
      });
      // 追加描寫記進草稿，跟其他欄位一起等按儲存才落地
      setCard((current) => (current ? { ...current, gen_prompt: aiPrompt } : current));
      await refreshGallery();
      await onPreference("image_source", aiSource);
      if (!sponsorUnlocked) await onPreference("ai_image_trials_used", trialsUsed + 1);
    } catch (reason) {
      setAiGenError(String(reason));
    } finally {
      setAiGenerating(false);
    }
  }

  async function deleteGalleryImage(file: string) {
    const accepted = await confirm(t("aiGalleryDeleteConfirm"), { title: t("aiGalleryDeleteTitle"), kind: "warning" });
    if (!accepted) return;
    await invoke("delete_gallery_image", { world, name: galleryName, file });
    setGalleryFiles((current) => current.filter((item) => item !== file));
    setGalleryImages((current) => {
      const { [file]: _, ...remaining } = current;
      return remaining;
    });
    setGalleryLoaded((current) => Math.max(0, current - (galleryImages[file] ? 1 : 0)));
  }

  if (!card) return message ? <p role="alert">{message}</p> : null;

  const shownImage = draftImage === undefined ? imageDataUrl : draftImage?.url;
  const shownAvatar = draftAvatar === undefined ? avatarImgUrl : draftAvatar?.url;
  const aiGenBlocked = !card.name.trim() || !card.public_md.trim();
  const unsavedCount =
    (JSON.stringify(card) !== savedCardJson ? 1 : 0) +
    (draftImage !== undefined ? 1 : 0) +
    (draftAvatar !== undefined ? 1 : 0);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    if (!card) return;
    const target = card.name.trim();
    if (!target) {
      setMessage(t("nameRequiredError"));
      return;
    }
    const nameError = characterNameError(
      target,
      takenNames.filter((taken) => taken !== name),
    );
    if (nameError) {
      setMessage(nameError);
      return;
    }
    // 圖示清空就回預設，免得沒圖也沒 emoji 的空白角色
    const saved: CharacterCard = {
      ...card,
      name: target,
      avatar: card.avatar.trim() || DEFAULT_AVATAR,
    };
    try {
      // 改名要先搬檔＋回填引用，再寫卡片內容；建新卡直接寫
      if (name !== null && target !== name) {
        await invoke("rename_character", { world, from: name, to: target });
      }
      await invoke("write_character", { world, card: saved });
      if (draftImage === null) await invoke("delete_character_image", { world, name: target });
      else if (draftImage) await invoke("save_character_image", { world, name: target, data: draftImage.bytes });
      if (draftAvatar === null) await invoke("delete_character_avatar", { world, name: target });
      else if (draftAvatar) await invoke("save_character_avatar", { world, name: target, data: draftAvatar.bytes });
      setDraftImage(undefined);
      setDraftAvatar(undefined);
      await onImagesChanged();
      setCard(saved);
      setSavedCardJson(JSON.stringify(saved));
      setMessage(t("saved"));
      onSaved(target);
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

  // 移除頭像會退回 emoji 圖示，先問一聲（2026-07-27 使用者回饋）
  async function removeAvatar() {
    const accepted = await confirm(t("removeAvatarConfirm"), {
      title: t("removeAvatarTitle"),
      kind: "warning",
    });
    if (accepted) setDraftAvatar(null);
  }

  return (
    <form onSubmit={save} className="settings-form">
      {/* 按鈕列統一放頂部，與世界設定畫面同款（2026-07-24 使用者拍板） */}
      <div className="row">
        <button type="submit">{t("saveCard")}</button>
        <button type="button" onClick={() => void handleBack()}>
          {t("backToNow")}
        </button>
        {name !== null && (
          <>
            <button type="button" className="archive-button" onClick={archive}>
              {t("archiveCharacter")}
            </button>
            <button type="button" className="delete-character" onClick={() => void onDeleted()}>
              {t("deleteCharacter")}
            </button>
          </>
        )}
        {message && <span>{message}</span>}
        {unsavedCount > 0 && (
          <span className="unsaved-hint" role="status">
            {t("unsavedChanges", { n: unsavedCount })}
          </span>
        )}
      </div>
      <div className="card-editor-avatar">
        {shownImage ? (
          <button
            type="button"
            className="card-editor-image-zoom"
            aria-label={t("viewImageLabel")}
            title={t("viewImageLabel")}
            onClick={() => setLightboxOpen(true)}
          >
            <img className="card-editor-image" src={shownImage} alt="" />
          </button>
        ) : shownAvatar ? (
          <img className="avatar-round card-editor-avatar-round" src={shownAvatar} alt="" />
        ) : (
          <span className="card-editor-avatar-emoji" style={{ ["--ring" as string]: card.color }}>
            {card.avatar}
          </span>
        )}
      </div>
      <div className="row">
        <button type="button" onClick={() => document.getElementById(`character-image-${name}`)?.click()}>
          {t(shownImage ? "replaceImageBtn" : "addImageBtn")}
        </button>
        {/* 名字給圖庫資料夾用、公開設定進提示詞；欄位沒填就生不出像樣的圖，故先鎖住。
            提示掛在外層 span：disabled 的按鈕不收滑鼠事件，title 掛上去不會浮出來 */}
        <span className="hint-wrap" data-hint={aiGenBlocked ? t("aiGenNeedsContent") : undefined}>
          <button
            type="button"
            className="ai-gen-btn"
            disabled={aiGenBlocked}
            onClick={openAiGenerator}
          >
            ✨ {t("aiGenBtn")}
          </button>
        </span>
        {shownImage && (
          <>
            <button type="button" onClick={() => setDraftImage(null)}>{t("removeImageBtn")}</button>
            <button type="button" onClick={() => setCroppingAvatar(true)}>{t("makeAvatarBtn")}</button>
          </>
        )}
        {shownAvatar && <button type="button" onClick={() => void removeAvatar()}>{t("removeAvatarBtn")}</button>}
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
        {t("nameLabel")}
        <input
          value={card.name}
          placeholder={t("newCharacterPlaceholder")}
          onChange={(e) => setCard({ ...card, name: e.currentTarget.value })}
        />
      </label>
      {/* 改名時才提示：機械取代劇情正文會誤傷同名詞句，故舊名留在原處（2026-07-27 拍板） */}
      {name !== null && card.name.trim() !== name && (
        <p className="field-note" role="note">
          {t("renameNote")}
        </p>
      )}
      <label>
        {t("avatarEmojiLabel")}
        <div className="emoji-row">
          <input
            className="emoji-input"
            value={card.avatar}
            onChange={(e) =>
              setCard({
                ...card,
                avatar: clampChars(e.currentTarget.value.replace(/\s/g, ""), AVATAR_MAX_CHARS),
              })
            }
          />
          {AVATAR_EMOJIS.map((emoji) => (
            <button
              key={emoji}
              type="button"
              className="emoji-preset"
              aria-pressed={card.avatar === emoji}
              onClick={() => setCard({ ...card, avatar: emoji })}
            >
              {emoji}
            </button>
          ))}
        </div>
      </label>
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
      {shownImage && (
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
          onConfirm={async (image) => setDraftImage(image)}
          onCancel={() => setPendingImage(null)}
        />
      )}
      {aiGenOpen && (
        <div className="modal-overlay" onClick={() => !aiGenerating && setAiGenOpen(false)}>
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("aiGenTitle")} onClick={(event) => event.stopPropagation()}>
            <h2>{t("aiGenTitle")}</h2>
            <label>{t("aiGenPromptLabel")}<textarea rows={3} value={aiPrompt} placeholder={t("aiGenPromptPlaceholder")} onChange={(event) => setAiPrompt(event.currentTarget.value)} /></label>
            <label>{t("aiGenSourceLabel")}
              <div className="row">
                <select value={aiSource} onChange={(event) => setAiSource(event.currentTarget.value)} disabled={aiGenerating}>
                  {sourceOptions.map((source) => <option key={source} value={source}>{source === "api" ? t("aiGenSourceApi") : `${source[0].toUpperCase()}${source.slice(1)}`}</option>)}
                  {!sourceOptions.includes(aiSource) && <option value={aiSource}>{`${aiSource[0].toUpperCase()}${aiSource.slice(1)}`}</option>}
                </select>
                <button type="button" disabled={aiGenerating} onClick={onOpenAiSettings}>⚙ {t("aiTab")}</button>
              </div>
            </label>
            {!sponsorUnlocked && <p role="note">{t("aiGenTrialNote", { n: Math.max(0, 3 - trialsUsed) })}</p>}
            {aiGenError && <div className="ai-gen-error" role="alert"><div>{t("aiGenFailed")}</div><small>{aiGenError}</small></div>}
            {galleryFiles.length > 0 && (
              <section aria-label={t("aiGalleryTitle")}>
                <h3>{t("aiGalleryTitle")}</h3>
                <div className="ai-gallery">
                  {galleryFiles.slice(0, galleryLoaded).map((file) => galleryImages[file] && (
                    <div className="ai-gallery-thumb" key={file}>
                      <button
                        type="button"
                        className="ai-gallery-pick"
                        title={t("aiGalleryPick")}
                        onClick={() => { setAiGenOpen(false); setPendingImage(galleryImages[file]); }}
                      >
                        <img src={galleryImages[file]} alt="" />
                      </button>
                      <button
                        type="button"
                        className="ai-gallery-delete"
                        aria-label={t("aiGalleryDeleteTitle")}
                        onClick={() => void deleteGalleryImage(file).catch((reason) => setAiGenError(String(reason)))}
                      >×</button>
                    </div>
                  ))}
                </div>
                {galleryFiles.length > galleryLoaded && <button type="button" onClick={() => void loadGalleryPage(galleryFiles, galleryLoaded)}>{t("aiGalleryLoadMore", { n: galleryFiles.length - galleryLoaded })}</button>}
              </section>
            )}
            {/* 主要動作放右下（2026-07-27 使用者拍板：此對話框例外，不置頂） */}
            <div className="ai-gen-footer">
              <button type="button" disabled={aiGenerating} onClick={() => setAiGenOpen(false)}>{t("cropCancel")}</button>
              <button type="button" className="ai-gen-submit" disabled={aiGenerating} onClick={() => void generateImage()}>
                {aiGenerating ? t("aiGenerating") : `✨ ${t("aiGenBtn")}`}
              </button>
            </div>
          </div>
        </div>
      )}
      {aiGenLockedOpen && (
        <div className="modal-overlay" onClick={() => setAiGenLockedOpen(false)}>
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("aiGenLockedTitle")} onClick={(event) => event.stopPropagation()}>
            <div className="row"><button type="button" onClick={() => void openUrl(KOFI_URL)}>{t("sponsorBtn")}</button><button type="button" onClick={() => setAiGenLockedOpen(false)}>{t("closeBtn")}</button></div>
            <h2>{t("aiGenLockedTitle")}</h2><p>{t("aiGenLockedBody")}</p>
          </div>
        </div>
      )}
      {lightboxOpen && shownImage && (
        <div
          className="modal-overlay"
          role="dialog"
          aria-modal="true"
          aria-label={t("viewImageLabel")}
          onClick={() => setLightboxOpen(false)}
        >
          <img className="lightbox-image" src={shownImage} alt="" />
        </div>
      )}
      {croppingAvatar && shownImage && (
        <CropDialog
          title={t("cropAvatarTitle")}
          src={shownImage}
          aspect={1}
          cropShape="round"
          onConfirm={async (image) => setDraftAvatar(image)}
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
  const castDrag = useDragReorder(
    activeCharacters,
    (character) => character.name,
    (ordered) => void reorderCast(ordered),
  );
  // 角色圖快取：name → data URL（來源是匯入時存下的原 PNG，後端 read_character_image）
  const [characterImages, setCharacterImages] = useState<Record<string, string>>({});
  const [characterAvatars, setCharacterAvatars] = useState<Record<string, string>>({});
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
  // false＝關閉；字串＝開啟並落在該分頁（生圖對話框的「AI 連線設定」鈕直開 ai 分頁）
  const [settingsOpen, setSettingsOpen] = useState<false | "appearance" | "ai">(false);
  // 主欄下半部（messages＋composer）三選一整面取代：單幕閱讀／角色卡編輯／GM 世界設定編輯
  // （使用者拍板改版：需求 4 不用 modal，與需求 3 單幕閱讀同一套「整面取代」模式）
  const [mainView, setMainView] = useState<
    | { kind: "scene"; n: number }
    | { kind: "character"; name: string }
    | { kind: "new-character" }
    | { kind: "world" }
    | null
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
    const current = chatConfigRef.current;
    if (!current) return;
    const updated = { ...current, preferences: { ...current.preferences, [key]: value } };
    chatConfigRef.current = updated;
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

  useEffect(() => {
    document.documentElement.dataset.theme = resolveTheme(config);
  }, [config]);

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

  // 刪桌：整桌的角色、紀錄、世界設定一起沒，故確認框把後果講白。
  // 刪掉最後一桌就補一張範例桌——App 不留「沒有桌」的空狀態（NewPlan §9.3 零精靈）
  async function deleteTable(name: string) {
    if (!config || generating !== null) return;
    const accepted = await confirm(t("deleteTableConfirm", { name }), {
      title: t("deleteTableTitle"),
      kind: "warning",
    });
    if (!accepted) return;
    setError("");
    try {
      await invoke("delete_world", { world: name });
      let names = await invoke<string[]>("list_worlds");
      if (names.length === 0) {
        names = [
          await invoke<string>("create_sample_world", {
            lang: normalizeLang(config.preferences["language"]),
          }),
        ];
      }
      setWorlds(names);
      if (name === table) await enterTable(names[0], config);
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

  // 建卡或改名存檔後：名單與圖片重載，畫面停在存檔後的那張卡（草稿轉正、改名換 key）
  async function finishCardSaved(saved: string) {
    const previous = mainView?.kind === "character" ? mainView.name : null;
    await refreshCharacters();
    if (previous !== saved) setMainView({ kind: "character", name: saved });
    if (previous === null || speaker === previous) setSpeaker(saved);
  }

  // 角色被隱藏或刪除後的善後：名單重載、發言對象改人、關掉編輯面板
  async function finishRemoval(name: string) {
    const cast = await refreshCharacters();
    if (speaker === name) {
      setSpeaker(cast.find((character) => !character.archived)?.name ?? "");
    }
    setMainView(null);
  }

  // 側欄拖曳排序：先樂觀套用，寫檔失敗才回捲
  async function reorderCast(ordered: CharacterMeta[]) {
    setError("");
    const previous = characters;
    setCharacters([...ordered, ...characters.filter((character) => character.archived)]);
    try {
      await invoke("reorder_characters", {
        world: table,
        names: ordered.map((character) => character.name),
      });
    } catch (reason) {
      setCharacters(previous);
      setError(String(reason));
    }
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

  // 隱藏區與角色卡編輯畫面共用同一條刪除路徑（確認框＋善後）
  async function deleteCharacter(name: string) {
    setError("");
    try {
      const accepted = await confirm(t("deleteCharacterConfirm", { name }), {
        title: t("deleteCharacterTitle"),
        kind: "warning",
      });
      if (!accepted) return;
      await invoke("delete_character", { world: table, name });
      await finishRemoval(name);
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
                <div className="table-row" key={name}>
                  <button
                    className={`table-item ${name === table ? "table-item-active" : ""}`}
                    onClick={() => switchTable(name)}
                  >
                    {name}
                  </button>
                  <button
                    type="button"
                    className="table-delete"
                    aria-label={t("deleteTableTitle")}
                    title={t("deleteTableTitle")}
                    disabled={generating !== null}
                    onClick={() => void deleteTable(name)}
                  >
                    ✕
                  </button>
                </div>
              ))}
            </nav>
          </div>
        </details>
        <section className="character-panel" aria-label={t("castAria")}>
          <div className="character-list">
            {/* GM 卡：與角色卡同款同尺寸（GM 是桌上最重要的一位），但不可選為發言對象；整張點擊開世界設定＋世界書 */}
            <div
              role="button"
              tabIndex={0}
              className="tcard tcard-gm"
              title={t("worldSummary")}
              onClick={() => setMainView({ kind: "world" })}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  setMainView({ kind: "world" });
                }
              }}
            >
              <span className="tcard-art">
                <img className="gm-book" src={gmBook} alt="" />
              </span>
              <span className="tcard-body">
                <span className="tcard-name-row">
                  <span className="tcard-plate">GM</span>
                </span>
              </span>
              <span className="gm-cfg" aria-hidden="true">
                ⚙
              </span>
            </div>
            {/* 角色卡＝桌遊組件卡：圖窗＋名字 wedge＋檔位寶石（tier 是既有欄位；「跟隨預設」不掛寶石） */}
            {castDrag.order.map((c) => (
              <div
                key={c.name}
                role="button"
                tabIndex={0}
                className={`tcard ${speaker === c.name ? "tcard-selected" : ""}${
                  castDrag.draggingKey === c.name ? " row-dragging" : ""
                }`}
                style={{ ["--fac" as string]: c.color }}
                onClick={() => {
                  if (castDrag.justDragged()) return;
                  setSpeaker(c.name);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    setSpeaker(c.name);
                  }
                }}
                title={`${t("castHint", { name: c.name })}｜${t("dragToReorder")}`}
                {...castDrag.rowProps(c)}
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
                      onClick={() => void deleteCharacter(character.name)}
                    >
                      {t("deleteCharacter")}
                    </button>
                  </div>
                ))}
              </div>
            </details>
          )}
          {/* 建卡＝直接開空白角色卡編輯器，名字與內容都在那邊填（2026-07-27 使用者拍板） */}
          <div className="character-create">
            <button type="button" onClick={() => setMainView({ kind: "new-character" })}>
              {t("createCard")}
            </button>
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
          </div>
        </section>
        <div className="sidebar-footer">
          <button className="settings-open" onClick={() => setSettingsOpen("appearance")}>
            ⚙️ {t("settingsBtn")}
          </button>
        </div>
      </aside>

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
        ) : mainView?.kind === "character" || mainView?.kind === "new-character" ? (
          <EditPane
            title={
              mainView.kind === "new-character"
                ? t("newCardTitle")
                : t("editCardSummary", { name: mainView.name })
            }
          >
            <CardEditor
              world={table}
              name={mainView.kind === "new-character" ? null : mainView.name}
              takenNames={characters.map((character) => character.name)}
              newCardColor={PALETTE[characters.length % PALETTE.length]}
              imageDataUrl={mainView.kind === "character" ? characterImages[mainView.name] : undefined}
              avatarImgUrl={mainView.kind === "character" ? characterAvatars[mainView.name] : undefined}
              onImagesChanged={() => loadCharacterImages(table, characters)}
              onSaved={(saved) => void finishCardSaved(saved)}
              onArchived={
                mainView.kind === "character"
                  ? () => finishRemoval(mainView.name)
                  : async () => setMainView(null)
              }
              onDeleted={
                mainView.kind === "character"
                  ? () => deleteCharacter(mainView.name)
                  : async () => setMainView(null)
              }
              onBack={() => setMainView(null)}
              config={config}
              onPreference={changePreference}
              onOpenAiSettings={() => setSettingsOpen("ai")}
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

      {/* 放整個版面最後：設定視窗永遠疊在其他 modal（含生圖對話框）之上 */}
      {settingsOpen !== false && (
        <SettingsWindow
          config={config}
          onSaved={setConfig}
          onPreference={(key, value) => void changePreference(key, value)}
          onClose={() => setSettingsOpen(false)}
          initialTab={settingsOpen}
        />
      )}
    </div>
  );
}

export default App;
