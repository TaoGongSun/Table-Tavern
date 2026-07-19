import { FormEvent, useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";

type Tier = "best" | "balanced" | "fast" | "default";

interface CharacterMeta {
  name: string;
  color: string;
  avatar: string;
  tier: Tier;
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

const PALETTE = ["#e07a5f", "#3d84a8", "#81b29a", "#f2a541", "#9b5de5", "#e56399"];

const DEFAULT_TABLE_NAME = "新的一桌";

interface CliInfo {
  id: string;
  path: string;
  version: string;
}

const CLI_LABELS: Record<string, string> = {
  claude: "Claude Code CLI",
  codex: "Codex CLI",
};

const CLI_RISKS = [
  "供應商條款禁止第三方工具使用訂閱憑證；Google 已對同類工具的使用者執行帳號停權且申訴無果；Anthropic 保留不經通知執法的權利。",
  "多角色扮演的用量形狀與條款所述「一般個人使用」有可見差距，可能觸發限流或審查。",
  "在訂閱模式下生成違反該供應商內容政策的內容，風險疊加。",
  "後果由你自己的帳號承擔。",
];

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
        <strong>還差最後一步：貼上 API key 就能開玩</strong>
        <p>本 App 不代管模型——你自備一把 OpenRouter key，一把通吃多家模型（角色與 GM 可以用不同檔位）。</p>
        <ol>
          <li>
            註冊 OpenRouter
            <button type="button" onClick={() => void openUrl("https://openrouter.ai/")}>
              開啟註冊頁
            </button>
          </li>
          <li>儲值小額（$5 起，用多少扣多少，不會自動扣款）</li>
          <li>
            建立一把 API key 並貼到下方
            <button
              type="button"
              onClick={() => void openUrl("https://openrouter.ai/settings/keys")}
            >
              開啟 API key 頁
            </button>
          </li>
        </ol>
        <p>費用有多低？以平衡檔粗估，$5 約可玩 5 個晚上；改用「快速省額度」檔更便宜。</p>
        <div className="row">
          <input
            type="password"
            aria-label="OpenRouter API key"
            value={apiKey}
            onChange={(event) => setApiKey(event.currentTarget.value)}
            placeholder="sk-or-..."
          />
          <button type="submit">儲存並開玩</button>
        </div>
        {message && <span role="alert">{message}</span>}
        <small>已自行安裝並登入官方 CLI 的進階使用者，也可以改到下方「AI 設定」啟用 CLI 訂閱模式。</small>
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
  const [message, setMessage] = useState("");

  useEffect(() => {
    invoke<CliInfo[]>("detect_clis").then(setClis).catch(() => setClis([]));
  }, []);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    if (transport !== "api" && !riskAccepted) {
      setMessage("啟用 CLI 訂閱模式前，請先勾選風險告知確認");
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
      setMessage("已儲存");
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <details className="settings">
      <summary>AI 設定（自備 key／CLI 模式）</summary>
      <form onSubmit={save} className="settings-form">
        <fieldset className="transport-choice">
          <legend>連線方式</legend>
          <label className="inline">
            <input
              type="radio"
              name="transport"
              checked={transport === "api"}
              onChange={() => setTransport("api")}
            />
            API 直連（OpenRouter，標準）
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
                {CLI_LABELS[id]}（訂閱模式，進階）
                {found ? (
                  <span className="cli-version">已偵測：{found.version}</span>
                ) : (
                  <span className="cli-version">未偵測到；請自行安裝並登入官方 CLI，App 不代辦</span>
                )}
              </label>
            );
          })}
        </fieldset>
        {transport !== "api" && (
          <div className="risk-box" role="note">
            <strong>啟用前請了解具體風險：</strong>
            <ul>
              {CLI_RISKS.map((risk, index) => (
                <li key={index}>{risk}</li>
              ))}
            </ul>
            <label className="inline">
              <input
                type="checkbox"
                checked={riskAccepted}
                onChange={(e) => setRiskAccepted(e.currentTarget.checked)}
              />
              我已了解上述風險，仍要以自己的帳號啟用
            </label>
          </div>
        )}
        <label>
          OpenRouter API key
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.currentTarget.value)}
            placeholder="sk-or-..."
          />
        </label>
        {(["best", "balanced", "fast"] as const).map((tier) => (
          <label key={tier}>
            {tier} 檔位模型
            <input
              value={tierModels[tier] ?? ""}
              onChange={(e) =>
                setTierModels({ ...tierModels, [tier]: e.currentTarget.value })
              }
            />
          </label>
        ))}
        <label>
          GM 檔位（導演與旁白用，建議較強模型）
          <select value={gmTier} onChange={(e) => setGmTier(e.currentTarget.value)}>
            {["best", "balanced", "fast", "default"].map((tier) => (
              <option key={tier} value={tier}>
                {tier}
              </option>
            ))}
          </select>
        </label>
        <label>
          GM 推進每回合最大發言數
          <input
            type="number"
            min={1}
            max={10}
            value={maxRound}
            onChange={(e) => setMaxRound(e.currentTarget.value)}
          />
        </label>
        <label>
          自訂 base URL（進階，留空用 OpenRouter）
          <input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.currentTarget.value)}
            placeholder="https://openrouter.ai/api/v1"
          />
        </label>
        <div className="row">
          <button type="submit">儲存設定</button>
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
      setMessage("已儲存");
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <details className="settings">
      <summary>世界設定 world.md（只進 GM 上下文，角色只知道 GM 說出口的內容）</summary>
      <form onSubmit={save} className="settings-form">
        <textarea
          rows={6}
          aria-label="世界設定"
          value={text}
          onChange={(e) => setText(e.currentTarget.value)}
        />
        <div className="row">
          <button type="submit">儲存世界設定</button>
          {message && <span>{message}</span>}
        </div>
      </form>
    </details>
  );
}

function CardEditor({ world, name, onSaved }: { world: string; name: string; onSaved: () => void }) {
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
      setMessage("已儲存");
      onSaved();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <details className="settings">
      <summary>編輯「{card.name}」角色卡</summary>
      <form onSubmit={save} className="settings-form">
        <label>
          公開設定（所有人認識的它）
          <textarea
            rows={4}
            value={card.public_md}
            onChange={(e) => setCard({ ...card, public_md: e.currentTarget.value })}
          />
        </label>
        <label>
          私有設定（只進本角色與 GM 的上下文）
          <textarea
            rows={4}
            value={card.private_md}
            onChange={(e) => setCard({ ...card, private_md: e.currentTarget.value })}
          />
        </label>
        <label>
          檔位
          <select
            value={card.tier}
            onChange={(e) => setCard({ ...card, tier: e.currentTarget.value as Tier })}
          >
            {["default", "best", "balanced", "fast"].map((tier) => (
              <option key={tier} value={tier}>
                {tier}
              </option>
            ))}
          </select>
        </label>
        <div className="row">
          <button type="submit">儲存角色卡</button>
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
  const bottomRef = useRef<HTMLDivElement>(null);

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
      let name = DEFAULT_TABLE_NAME;
      for (let n = 2; existing.includes(name); n += 1) name = `${DEFAULT_TABLE_NAME} ${n}`;
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

  async function createCharacter(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError("");
    const name = characterName.trim();
    if (name === "GM" || name === "玩家") {
      setError("「GM」與「玩家」是保留名稱，請換一個角色名");
      return;
    }
    const card: CharacterCard = {
      name,
      color: PALETTE[characters.length % PALETTE.length],
      avatar: "🎭",
      tier: "default",
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
        await appendEvent({ ts: nowTs(), speaker: "GM", kind: "system", text: `GM 請「${name}」發言` });
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

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <button className="new-table" onClick={newTable} disabled={generating !== null}>
          ＋ 開新的一桌
        </button>
        <nav className="table-list" aria-label="桌列表">
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
        <div className="sidebar-footer">
          <Settings config={config} onSaved={setConfig} />
        </div>
      </aside>

      <main className="chat-main">
        <header className="chat-header">
          {editingName === null ? (
            <button
              className="table-title"
              title="點一下改名"
              onClick={() => setEditingName(table)}
            >
              {table}
            </button>
          ) : (
            <input
              className="table-title-input"
              autoFocus
              value={editingName}
              aria-label="桌名"
              onChange={(e) => setEditingName(e.currentTarget.value)}
              onBlur={() => renameTable(editingName)}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
                if (e.key === "Escape") setEditingName(null);
              }}
            />
          )}
        </header>

        <Onboarding config={config} onSaved={setConfig} />

        <section className="row cast-row" aria-label="角色">
          {characters.map((c) => (
            <button
              key={c.name}
              className={`cast ${speaker === c.name ? "cast-active" : ""}`}
              style={{ ["--ring" as string]: c.color }}
              onClick={() => setSpeaker(c.name)}
              title={`點名「${c.name}」接話`}
            >
              <Avatar meta={c} /> {c.name}
            </button>
          ))}
          <form className="row" onSubmit={createCharacter}>
            <input
              aria-label="角色名稱"
              value={characterName}
              onChange={(e) => setCharacterName(e.currentTarget.value)}
              placeholder="新角色名稱"
            />
            <button type="submit">建卡</button>
          </form>
        </section>

        <WorldEditor world={table} />

        {speaker && (
          <CardEditor
            world={table}
            name={speaker}
            onSaved={() =>
              invoke<CharacterMeta[]>("list_characters", { world: table }).then(setCharacters)
            }
          />
        )}

        <section className="messages" aria-label="對話">
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
                  <span className="typing" aria-label={`${generating.name} 正在打字`}>
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
                <span className="typing" aria-label="GM 正在打字">
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
            aria-label="玩家輸入"
            value={input}
            onChange={(e) => setInput(e.currentTarget.value)}
            placeholder={speaker ? `以玩家身分發言，「${speaker}」會接話…` : "先建立一個角色"}
            disabled={!speaker || generating !== null}
          />
          <button type="submit" disabled={!speaker || generating !== null}>
            送出
          </button>
          <button
            type="button"
            onClick={() => requestReply(speaker)}
            disabled={!speaker || generating !== null}
            title="不輸入玩家發言，直接請被點名的角色接話"
          >
            請{speaker || "角色"}發言
          </button>
          <button
            type="button"
            onClick={gmNarrate}
            disabled={generating !== null}
            title="請 GM 插入一段場景旁白（GM 讀得到世界設定與全部角色卡）"
          >
            GM 旁白
          </button>
          <button
            type="button"
            onClick={gmAdvance}
            disabled={generating !== null || characters.length === 0}
            title="GM 點名下一位角色接話並接力推進，遇「輪到玩家」或達每回合上限即停"
          >
            GM 推進
          </button>
        </form>
        {error && <p role="alert">{error}</p>}
      </main>
    </div>
  );
}

export default App;
