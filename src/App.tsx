import { FormEvent, useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
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

function nowTs() {
  return new Date().toISOString();
}

function Settings({ config, onSaved }: { config: AppConfig; onSaved: (c: AppConfig) => void }) {
  const [apiKey, setApiKey] = useState(config.api_keys["openrouter"] ?? "");
  const [tierModels, setTierModels] = useState<Record<string, string>>({
    ...SUGGESTED_TIER_MODELS,
    ...config.tier_models,
  });
  const [baseUrl, setBaseUrl] = useState(String(config.preferences["base_url"] ?? ""));
  const [message, setMessage] = useState("");

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    const next: AppConfig = {
      ...config,
      api_keys: { ...config.api_keys, openrouter: apiKey.trim() },
      tier_models: tierModels,
      preferences: { ...config.preferences, base_url: baseUrl.trim() },
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
      <summary>設定（API key／檔位模型）</summary>
      <form onSubmit={save} className="settings-form">
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

function App() {
  const [worlds, setWorlds] = useState<string[]>([]);
  const [worldName, setWorldName] = useState("");
  const [world, setWorld] = useState("");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  const [characterName, setCharacterName] = useState("");
  const [speaker, setSpeaker] = useState("");
  const [scene, setScene] = useState(0);
  const [events, setEvents] = useState<TranscriptEvent[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState<string | null>(null);
  const [error, setError] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    Promise.all([
      invoke<string[]>("list_worlds").then(setWorlds),
      invoke<AppConfig>("read_config").then(setConfig),
    ]).catch((reason) => setError(String(reason)));
  }, []);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [events, streaming]);

  async function openWorld(name: string) {
    setError("");
    try {
      const state = await invoke<WorldState>("read_state", { world: name });
      const transcript = await invoke<TranscriptEvent[]>("read_transcript", {
        world: name,
        scene: state.current_scene,
      });
      const cast = await invoke<CharacterMeta[]>("list_characters", { world: name });
      setWorld(name);
      setScene(state.current_scene);
      setEvents(transcript);
      setCharacters(cast);
      setSpeaker(cast[0]?.name ?? "");
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function createWorld(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError("");
    try {
      await invoke("create_world", { name: worldName });
      setWorlds(await invoke<string[]>("list_worlds"));
      await openWorld(worldName);
      setWorldName("");
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function createCharacter(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError("");
    const card: CharacterCard = {
      name: characterName.trim(),
      color: PALETTE[characters.length % PALETTE.length],
      avatar: "🎭",
      tier: "default",
      public_md: "",
      private_md: "",
    };
    try {
      await invoke("write_character", { world, card });
      const cast = await invoke<CharacterMeta[]>("list_characters", { world });
      setCharacters(cast);
      setSpeaker(card.name);
      setCharacterName("");
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function appendEvent(event: TranscriptEvent) {
    await invoke("append_transcript", { world, scene, event });
    setEvents((previous) => [...previous, event]);
  }

  async function send(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const text = input.trim();
    if (!text || !speaker || streaming !== null) return;
    setError("");
    setInput("");
    try {
      await appendEvent({ ts: nowTs(), speaker: "玩家", kind: "player", text });
      setStreaming("");
      const onDelta = new Channel<string>();
      onDelta.onmessage = (delta) => setStreaming((prev) => (prev ?? "") + delta);
      const full = await invoke<string>("chat_with_character", {
        world,
        character: speaker,
        onDelta,
      });
      await appendEvent({ ts: nowTs(), speaker, kind: "dialogue", text: full });
    } catch (reason) {
      setError(String(reason));
    } finally {
      setStreaming(null);
    }
  }

  const colorOf = (name: string) =>
    characters.find((c) => c.name === name)?.color ?? "#888888";

  if (!config) return <main className="container">{error && <p role="alert">{error}</p>}</main>;

  if (!world) {
    return (
      <main className="container">
        <h1>桌面酒館</h1>
        <Settings config={config} onSaved={setConfig} />
        <section aria-labelledby="world-list-title">
          <h2 id="world-list-title">世界</h2>
          {worlds.length === 0 ? (
            <p>尚無世界</p>
          ) : (
            <ul>
              {worlds.map((name) => (
                <li key={name}>
                  <button className="link" onClick={() => openWorld(name)}>
                    {name}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
        <form className="row" onSubmit={createWorld}>
          <input
            aria-label="世界名稱"
            value={worldName}
            onChange={(e) => setWorldName(e.currentTarget.value)}
            placeholder="輸入世界名稱"
          />
          <button type="submit">建立世界</button>
        </form>
        {error && <p role="alert">{error}</p>}
      </main>
    );
  }

  return (
    <main className="container chat-container">
      <header className="row chat-header">
        <button className="link" onClick={() => setWorld("")}>
          ← 世界列表
        </button>
        <h1>{world}</h1>
      </header>

      <Settings config={config} onSaved={setConfig} />

      <section className="row cast-row" aria-label="角色">
        {characters.map((c) => (
          <button
            key={c.name}
            className={`cast ${speaker === c.name ? "cast-active" : ""}`}
            style={{ borderColor: c.color }}
            onClick={() => setSpeaker(c.name)}
          >
            {c.avatar} {c.name}
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

      {speaker && <CardEditor world={world} name={speaker} onSaved={() => openWorld(world)} />}

      <section className="messages" aria-label="對話">
        {events.map((event, index) => (
          <div key={index} className={`message message-${event.kind}`}>
            {event.kind === "dialogue" && (
              <span className="speaker" style={{ color: colorOf(event.speaker) }}>
                {event.speaker}
              </span>
            )}
            <span className="text">{event.text}</span>
          </div>
        ))}
        {streaming !== null && (
          <div className="message message-dialogue">
            <span className="speaker" style={{ color: colorOf(speaker) }}>
              {speaker}
            </span>
            <span className="text">{streaming || "…"}</span>
          </div>
        )}
        <div ref={bottomRef} />
      </section>

      <form className="row" onSubmit={send}>
        <input
          aria-label="玩家輸入"
          value={input}
          onChange={(e) => setInput(e.currentTarget.value)}
          placeholder={speaker ? `對「${speaker}」說…` : "先建立一個角色"}
          disabled={!speaker || streaming !== null}
        />
        <button type="submit" disabled={!speaker || streaming !== null}>
          送出
        </button>
      </form>
      {error && <p role="alert">{error}</p>}
    </main>
  );
}

export default App;
