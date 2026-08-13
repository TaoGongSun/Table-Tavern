// 一句話開桌：把玩家的一句話（＋類型標籤）交給 AI 產出世界觀綱要與角色名單，
// 就地改完再展開成一張真的桌。整組 state 與三支生成流程都在這支元件裡；
// 關閉只是不畫（open=false 時回 null，元件不卸載），草稿留著，跟拆分前逐字等價。
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

const GENRE_KEYS = [
  "genGenreFantasy",
  "genGenreScifi",
  "genGenreUrban",
  "genGenreWuxia",
  "genGenreSchool",
  "genGenreApocalypse",
] as const;

interface GeneratedOutline {
  title: string;
  world: string;
  characters: { name: string; tagline: string }[];
}

interface GenerateOutlineResult {
  parsed: GeneratedOutline | null;
  raw: string;
}

interface GenerateExpandResult {
  worldId: string | null;
  raw: string;
}

interface GenerateCharacterResult {
  parsed: { name: string; tagline: string } | null;
  raw: string;
}

function serializeGeneratedOutline(outline: GeneratedOutline): string {
  const sections = [`## WORLD: ${outline.title.trim()}\n${outline.world.trim()}`];
  for (const character of outline.characters) {
    const name = character.name.trim();
    if (name) sections.push(`## CHARACTER: ${name}\n${character.tagline.trim()}`);
  }
  return sections.join("\n\n");
}

function resizeGeneratedCharacterTagline(target: HTMLTextAreaElement) {
  target.style.height = "auto";
  target.style.height = `${target.scrollHeight}px`;
}

export function GenerateTableDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  /** 桌生出來了：桌次清單重讀並進去那張新桌 */
  onCreated: (worldId: string) => Promise<void>;
}) {
  const [genInput, setGenInput] = useState("");
  const [genGenres, setGenGenres] = useState<string[]>([]);
  const [genOutline, setGenOutline] = useState<GeneratedOutline | null>(null);
  const [genOutlineRaw, setGenOutlineRaw] = useState<string | null>(null);
  const [genResultRaw, setGenResultRaw] = useState<string | null>(null);
  const [genResultMessage, setGenResultMessage] = useState<"outline" | "character">("outline");
  const [genError, setGenError] = useState("");
  const [genBusy, setGenBusy] = useState<"outline" | "character" | "expand" | null>(null);
  const [genCharacterHint, setGenCharacterHint] = useState("");

  async function generateTableOutline() {
    const input = genInput.trim();
    if (!input && genGenres.length === 0) return;
    setGenBusy("outline");
    setGenOutline(null);
    setGenOutlineRaw(null);
    setGenResultRaw(null);
    setGenResultMessage("outline");
    setGenError("");
    try {
      const result = await invoke<GenerateOutlineResult>("generate_table_outline", {
        input,
        genres: genGenres.map((key) => t(key as typeof GENRE_KEYS[number])),
      });
      setGenOutlineRaw(result.raw);
      if (result.parsed) {
        setGenOutline(result.parsed);
      } else {
        setGenResultRaw(result.raw);
      }
    } catch (reason) {
      setGenError(String(reason));
    } finally {
      setGenBusy(null);
    }
  }

  async function generateTableCharacter() {
    if (!genOutline) return;
    setGenBusy("character");
    setGenResultRaw(null);
    setGenResultMessage("character");
    setGenError("");
    try {
      const result = await invoke<GenerateCharacterResult>("generate_table_character", {
        input: genInput.trim(),
        genres: genGenres.map((key) => t(key as typeof GENRE_KEYS[number])),
        outlineRaw: serializeGeneratedOutline(genOutline),
        hint: genCharacterHint,
      });
      if (result.parsed) {
        const character = result.parsed;
        setGenOutline((current) => current && {
          ...current,
          characters: [...current.characters, character],
        });
        setGenCharacterHint("");
      } else {
        setGenResultRaw(result.raw);
      }
    } catch (reason) {
      setGenError(String(reason));
    } finally {
      setGenBusy(null);
    }
  }

  async function createGeneratedTable() {
    if (!genOutline || !genOutline.title.trim() || !genOutline.world.trim()) return;
    const input = genInput.trim();
    setGenBusy("expand");
    setGenError("");
    setGenResultRaw(null);
    setGenResultMessage("outline");
    try {
      const result = await invoke<GenerateExpandResult>("generate_table_expand", {
        input,
        genres: genGenres.map((key) => t(key as typeof GENRE_KEYS[number])),
        outlineRaw: serializeGeneratedOutline(genOutline),
      });
      if (!result.worldId) {
        setGenResultRaw(result.raw);
        return;
      }
      await onCreated(result.worldId);
      onClose();
      setGenOutline(null);
      setGenOutlineRaw(null);
      setGenResultRaw(null);
    } catch (reason) {
      setGenError(String(reason));
    } finally {
      setGenBusy(null);
    }
  }

  if (!open) return null;

  return (
    <div className="modal-overlay">
      <div className="modal gen-table-modal" role="dialog" aria-modal="true" aria-label={t("genTitle")} onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <strong>{t("genTitle")}</strong>
          <button type="button" className="modal-close" aria-label={t("closeBtn")} disabled={genBusy !== null} onClick={onClose}>×</button>
        </div>
        <textarea
          rows={4}
          value={genInput}
          placeholder={t("genInputPlaceholder")}
          aria-label={t("genInputPlaceholder")}
          disabled={genBusy !== null}
          onChange={(event) => setGenInput(event.currentTarget.value)}
        />
        <div className="gen-genres">
          {GENRE_KEYS.map((key) => {
            const selected = genGenres.includes(key);
            return (
              <button
                key={key}
                type="button"
                className={`gen-genre${selected ? " gen-genre-selected" : ""}`}
                aria-pressed={selected}
                disabled={genBusy !== null}
                onClick={() => setGenGenres((current) => selected ? current.filter((genre) => genre !== key) : [...current, key])}
              >
                {t(key)}
              </button>
            );
          })}
        </div>
        <div className="gen-submit-row">
          <button type="button" className="gen-submit" disabled={genBusy !== null || (!genInput.trim() && genGenres.length === 0)} onClick={() => void generateTableOutline()}>
            {genBusy === "outline" ? t("genGenerating") : t("genGenerateBtn")}
          </button>
          <small>{t("genQuotaNote")}</small>
        </div>
        {genOutline && (
          <section className="gen-outline-preview">
            <input
              value={genOutline.title}
              disabled={genBusy !== null}
              onChange={(event) => setGenOutline((current) => current && { ...current, title: event.currentTarget.value })}
            />
            <textarea
              rows={6}
              value={genOutline.world}
              disabled={genBusy !== null}
              onChange={(event) => setGenOutline((current) => current && { ...current, world: event.currentTarget.value })}
            />
            <h3>{t("genCharListTitle")}</h3>
            <div className="gen-character-list">
              {genOutline.characters.map((character, index) => (
                <div className="gen-character-row" key={index}>
                  <input
                    className="gen-character-name"
                    value={character.name}
                    disabled={genBusy !== null}
                    onChange={(event) => setGenOutline((current) => current && {
                      ...current,
                      characters: current.characters.map((item, itemIndex) => itemIndex === index ? { ...item, name: event.currentTarget.value } : item),
                    })}
                  />
                  <textarea
                    rows={2}
                    value={character.tagline}
                    disabled={genBusy !== null}
                    ref={(element) => {
                      if (element) resizeGeneratedCharacterTagline(element);
                    }}
                    onInput={(event) => resizeGeneratedCharacterTagline(event.currentTarget)}
                    onChange={(event) => setGenOutline((current) => current && {
                      ...current,
                      characters: current.characters.map((item, itemIndex) => itemIndex === index ? { ...item, tagline: event.currentTarget.value } : item),
                    })}
                  />
                  <button
                    type="button"
                    className="gen-remove-character"
                    aria-label={t("genRemoveCharacter")}
                    disabled={genBusy !== null}
                    onClick={() => setGenOutline((current) => current && {
                      ...current,
                      characters: current.characters.filter((_, itemIndex) => itemIndex !== index),
                    })}
                  >
                    ×
                  </button>
                </div>
              ))}
            </div>
            <button
              type="button"
              className="gen-add-character"
              disabled={genBusy !== null}
              onClick={() => setGenOutline((current) => current && {
                ...current,
                characters: [...current.characters, { name: "", tagline: "" }],
              })}
            >
              ＋ {t("genAddCharacter")}
            </button>
            <div className="gen-add-character-ai">
              <input
                value={genCharacterHint}
                placeholder={t("genCharHintPlaceholder")}
                aria-label={t("genCharHintPlaceholder")}
                disabled={genBusy !== null}
                onChange={(event) => setGenCharacterHint(event.currentTarget.value)}
              />
              <button
                type="button"
                disabled={genBusy !== null}
                onClick={() => void generateTableCharacter()}
              >
                {genBusy === "character" ? t("genCharGenerating") : t("genAddCharacterAI")}
              </button>
            </div>
          </section>
        )}
        {(genResultRaw !== null || genError) && (
          <section className="gen-result-error" role="alert">
            <p>{genError || t(genResultMessage === "character" ? "genCharParseFail" : "genParseFail")}</p>
            <pre>{genError || genResultRaw}</pre>
          </section>
        )}
        {genOutlineRaw !== null && (
          <div className="gen-result-actions">
            <button type="button" disabled={genBusy !== null} onClick={() => void generateTableOutline()}>{t("genRerollBtn")}</button>
            <button type="button" className="gen-submit" disabled={genBusy !== null || !genOutline || !genOutline.title.trim() || !genOutline.world.trim()} onClick={() => void createGeneratedTable()}>
              {genBusy === "expand" ? t("genExpanding") : t("genCreateBtn")}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
