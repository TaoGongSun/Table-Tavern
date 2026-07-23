import { FormEvent, useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
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

const CLI_LABELS: Record<string, string> = {
  claude: "Claude Code CLI",
  codex: "Codex CLI",
};

const CLI_RISK_KEYS = ["risk1", "risk2", "risk3", "risk4"] as const;

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

function Settings({ config, onSaved }: { config: AppConfig; onSaved: (c: AppConfig) => void }) {
  const [apiKey, setApiKey] = useState(config.api_keys["openrouter"] ?? "");
  const [tierModels, setTierModels] = useState<Record<string, string>>({
    ...SUGGESTED_TIER_MODELS,
    ...config.tier_models,
  });
  const [baseUrl, setBaseUrl] = useState(String(config.preferences["base_url"] ?? ""));
  const [gmTier, setGmTier] = useState(String(config.preferences["gm_tier"] ?? "best"));
  const [maxRound, setMaxRound] = useState(String(config.preferences["max_round_speakers"] ?? 3));
  const [transport, setTransport] = useState(String(config.preferences["transport"] ?? "api"));
  const [riskAccepted, setRiskAccepted] = useState(config.preferences["cli_risk_accepted"] === true);
  const [clis, setClis] = useState<CliInfo[]>([]);
  const [models, setModels] = useState<{ id: string; name: string }[]>([]);
  const [cliCatalogs, setCliCatalogs] = useState<Record<string, { id: string; label: string }[]>>({});
  const [customTiers, setCustomTiers] = useState<Record<string, boolean>>({});
  const [message, setMessage] = useState("");

  useEffect(() => {
    invoke<CliInfo[]>("detect_clis").then(setClis).catch(() => setClis([]));
    // CLI 模型下拉目錄：讀各 CLI 本機快取（後端 list_cli_models）
    for (const id of ["claude", "codex"]) {
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
  }, []);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    if (transport !== "api" && !riskAccepted) {
      setMessage(t("riskRequired"));
      return;
    }
    const next: AppConfig = {
      ...config,
      api_keys: { ...config.api_keys, openrouter: apiKey.trim() },
      tier_models: tierModels,
      preferences: {
        ...config.preferences,
        base_url: baseUrl.trim(),
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
    <details className="settings">
      <summary>{t("settingsSummary")}</summary>
      <form onSubmit={save} className="settings-form">
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
          {(["claude", "codex"] as const).map((id) => {
            const found = clis.find((c) => c.id === id);
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
                  <span className="cli-version">{t("cliDetected", { version: found.version })}</span>
                ) : (
                  <span className="cli-version">{t("cliNotDetected")}</span>
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
        <label>
          {t("apiKeyLabel")}
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.currentTarget.value)}
            placeholder="sk-or-..."
          />
        </label>
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
              {transport === "claude" ? t("cliCatalogClaude") : t("cliCatalogCodex")}
            </p>
          </>
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
        <label>
          {t("baseUrlLabel")}
          <input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.currentTarget.value)}
            placeholder="https://openrouter.ai/api/v1"
          />
        </label>
        <div className="row">
          <button type="submit">{t("saveSettings")}</button>
          {message && <span>{message}</span>}
        </div>
      </form>
    </details>
  );
}

// 世界書 v1：一份只進 GM 上下文的 world.md（NewPlan §7.0）
function WorldEditor({ world }: { world: string }) {
  const [text, setText] = useState<string | null>(null);
  const [message, setMessage] = useState("");

  useEffect(() => {
    setMessage("");
    setText(null);
    invoke<string>("read_world_md", { world })
      .then(setText)
      .catch((reason) => setMessage(String(reason)));
  }, [world]);

  if (text === null) return message ? <p role="alert">{message}</p> : null;

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    try {
      await invoke("write_world_md", { world, content: text });
      setMessage(t("saved"));
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <details className="settings">
      <summary>{t("worldSummary")}</summary>
      <form onSubmit={save} className="settings-form">
        <textarea
          rows={6}
          aria-label={t("worldAria")}
          value={text}
          onChange={(e) => setText(e.currentTarget.value)}
        />
        <div className="row">
          <button type="submit">{t("saveWorld")}</button>
          {message && <span>{message}</span>}
        </div>
      </form>
    </details>
  );
}

function CardEditor({
  world,
  name,
  hasImage,
  onSaved,
}: {
  world: string;
  name: string;
  hasImage: boolean;
  onSaved: () => void;
}) {
  const [card, setCard] = useState<CharacterCard | null>(null);
  const [message, setMessage] = useState("");

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

  return (
    <details className="settings">
      <summary>{t("editCardSummary", { name: card.name })}</summary>
      <form onSubmit={save} className="settings-form">
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
        {hasImage && (
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
        <div className="row">
          <button type="submit">{t("saveCard")}</button>
          {message && <span>{message}</span>}
        </div>
      </form>
    </details>
  );
}

// 頭像光圈：角色色上在光圈與左邊框，不整顆填色（NewPlan §9.2）
function Avatar({ meta }: { meta?: CharacterMeta }) {
  return (
    <span className="avatar" style={{ ["--ring" as string]: meta?.color ?? "#888888" }}>
      {meta?.avatar ?? "🎭"}
    </span>
  );
}

function App() {
  const [worlds, setWorlds] = useState<string[]>([]);
  const [table, setTable] = useState("");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  // 角色圖快取：name → data URL（來源是匯入時存下的原 PNG，後端 read_character_image）
  const [characterImages, setCharacterImages] = useState<Record<string, string>>({});
  const [characterName, setCharacterName] = useState("");
  const [speaker, setSpeaker] = useState("");
  const [scene, setScene] = useState(0);
  const [events, setEvents] = useState<TranscriptEvent[]>([]);
  const [input, setInput] = useState("");
  // 逐角色打字指示：狀態帶「是誰在生成、以哪種形式」，不做全域單一指示燈（NewPlan §9.2）
  const [generating, setGenerating] = useState<{
    name: string;
    kind: "dialogue" | "narration";
  } | null>(null);
  const [streamText, setStreamText] = useState("");
  const [editingName, setEditingName] = useState<string | null>(null);
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
        const encoded = await invoke<string | null>("read_character_image", {
          world,
          name: c.name,
        }).catch(() => null);
        return [c.name, encoded] as const;
      }),
    );
    setCharacterImages(
      Object.fromEntries(
        entries
          .filter(([, encoded]) => encoded !== null)
          .map(([name, encoded]) => [name, `data:image/png;base64,${encoded}`]),
      ),
    );
  }

  // 語系跟著 config 走；render 前同步進 i18n 模組，之後子樹的 t() 都拿到正確語言
  const language = normalizeLang(config?.preferences["language"]);
  setLang(language);

  async function changeLanguage(next: Lang) {
    if (!config) return;
    const updated = { ...config, preferences: { ...config.preferences, language: next } };
    setConfig(updated);
    try {
      await invoke("write_config", { config: updated });
    } catch (reason) {
      setError(String(reason));
    }
  }

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
          const name = await invoke<string>("create_sample_world");
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
    setEvents(transcript);
    setCharacters(cast);
    await loadCharacterImages(name, cast);
    setSpeaker(cast[0]?.name ?? "");
    setEditingName(null);
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

  async function exportTranscript() {
    setError("");
    try {
      const path = await invoke<string>("export_transcript", { world: table });
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
    if (!config || generating !== null || characters.length === 0) return;
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
  const generatingMeta = generating !== null ? metaOf(generating.name) : undefined;

  if (!config || !table) {
    return <main className="container">{error && <p role="alert">{error}</p>}</main>;
  }

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
            {characters.map((c) => (
              <button
                key={c.name}
                className={`character-card ${speaker === c.name ? "character-card-active" : ""}`}
                style={{ ["--ring" as string]: c.color }}
                onClick={() => setSpeaker(c.name)}
                title={t("castHint", { name: c.name })}
              >
                <span className="character-card-avatar">
                  {c.show_image && characterImages[c.name] ? (
                    <img
                      className="character-card-image"
                      src={characterImages[c.name]}
                      alt=""
                    />
                  ) : (
                    <Avatar meta={c} />
                  )}
                </span>
                <span className="character-card-name">{c.name}</span>
              </button>
            ))}
          </div>
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
          <label className="language-picker">
            {t("languageLabel")}
            <select
              value={language}
              onChange={(e) => void changeLanguage(normalizeLang(e.currentTarget.value))}
            >
              {LANGUAGE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <Settings config={config} onSaved={setConfig} />
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
          <button
            type="button"
            title={t("exportTranscriptHint")}
            aria-label={t("exportTranscript")}
            onClick={exportTranscript}
          >
            {t("exportTranscript")}
          </button>
        </header>

        <Onboarding config={config} onSaved={setConfig} />

        <WorldEditor world={table} />

        {speaker && (
          <CardEditor
            world={table}
            name={speaker}
            hasImage={speaker in characterImages}
            onSaved={() =>
              invoke<CharacterMeta[]>("list_characters", { world: table }).then(setCharacters)
            }
          />
        )}

        <section className="messages" aria-label={t("messagesAria")}>
          {events.map((event, index) => {
            if (event.kind === "dialogue") {
              const meta = metaOf(event.speaker);
              const color = meta?.color ?? "#888888";
              return (
                <div key={index} className="message message-dialogue">
                  <Avatar meta={meta} />
                  <div className="bubble" style={{ borderLeftColor: color }}>
                    <span className="speaker" style={{ color }}>
                      {event.speaker}
                    </span>
                    <span className="text">{event.text}</span>
                  </div>
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
            <div className="message message-dialogue">
              <Avatar meta={generatingMeta} />
              <div
                className="bubble"
                style={{ borderLeftColor: generatingMeta?.color ?? "#888888" }}
              >
                <span className="speaker" style={{ color: generatingMeta?.color ?? "#888888" }}>
                  {generating.name}
                </span>
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

        <form className="row composer" onSubmit={send}>
          <input
            aria-label={t("composerAria")}
            value={input}
            onChange={(e) => setInput(e.currentTarget.value)}
            placeholder={
              speaker ? t("composerPlaceholder", { name: speaker }) : t("composerNoCharacter")
            }
            disabled={!speaker || generating !== null}
          />
          <button type="submit" disabled={!speaker || generating !== null}>
            {t("send")}
          </button>
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
            disabled={generating !== null || characters.length === 0}
            title={t("gmAdvanceHint")}
          >
            {t("gmAdvance")}
          </button>
        </form>
        {error && <p role="alert">{error}</p>}
      </main>
    </div>
  );
}

export default App;
