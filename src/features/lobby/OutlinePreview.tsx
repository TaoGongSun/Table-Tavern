// 一句話開桌的可改預覽（標題、簡介、角色名單、手加、AI 加一位）與生成失敗區：聊天桌對話窗與 VN 精靈 ② 共用。
// `player` 是 VN 精靈插在簡介與角色名單之間的主角列。
import type { ReactNode } from "react";
import { t } from "../../i18n";
import { AiErrorText } from "../../shared/ui/atoms";
import type { TableGenesis } from "./useTableGenesis";

function resizeTagline(target: HTMLTextAreaElement) {
  target.style.height = "auto";
  target.style.height = `${target.scrollHeight}px`;
}

export function OutlinePreview({
  genesis,
  genreLabels,
  player,
  showCharacters = true,
}: {
  genesis: TableGenesis;
  genreLabels: string[];
  player?: ReactNode;
  /** 開房且朋友自己建角色：不畫名單、手加、AI 再加一位 */
  showCharacters?: boolean;
}) {
  const { outline, setOutline, busy } = genesis;
  if (!outline) return null;
  const disabled = busy !== null;
  const setCharacter = (index: number, patch: Partial<{ name: string; tagline: string }>) =>
    setOutline((current) => current && {
      ...current,
      characters: current.characters.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item)),
    });
  return (
    <section className="gen-outline-preview">
      <input
        value={outline.title}
        disabled={disabled}
        onChange={(event) => {
          const title = event.currentTarget.value;
          setOutline((current) => current && { ...current, title });
        }}
      />
      <textarea
        rows={6}
        value={outline.world}
        disabled={disabled}
        onChange={(event) => {
          const world = event.currentTarget.value;
          setOutline((current) => current && { ...current, world });
        }}
      />
      {player}
      {showCharacters && (
      <>
      <h3>{t("genCharListTitle")}</h3>
      <div className="gen-character-list">
        {outline.characters.map((character, index) => (
          <div className="gen-character-row" key={index}>
            <input
              className="gen-character-name"
              value={character.name}
              disabled={disabled}
              onChange={(event) => setCharacter(index, { name: event.currentTarget.value })}
            />
            <textarea
              rows={2}
              value={character.tagline}
              disabled={disabled}
              ref={(element) => {
                if (element) resizeTagline(element);
              }}
              onInput={(event) => resizeTagline(event.currentTarget)}
              onChange={(event) => setCharacter(index, { tagline: event.currentTarget.value })}
            />
            <button
              type="button"
              className="gen-remove-character"
              aria-label={t("genRemoveCharacter")}
              disabled={disabled}
              onClick={() => setOutline((current) => current && {
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
        disabled={disabled}
        onClick={() => setOutline((current) => current && {
          ...current,
          characters: [...current.characters, { name: "", tagline: "" }],
        })}
      >
        ＋ {t("genAddCharacter")}
      </button>
      <div className="gen-add-character-ai">
        <input
          value={genesis.hint}
          placeholder={t("genCharHintPlaceholder")}
          aria-label={t("genCharHintPlaceholder")}
          disabled={disabled}
          onChange={(event) => genesis.setHint(event.currentTarget.value)}
        />
        <button type="button" disabled={disabled} onClick={() => void genesis.addCharacter(genreLabels)}>
          {busy === "character" ? t("genCharGenerating") : t("genAddCharacterAI")}
        </button>
      </div>
      </>
      )}
    </section>
  );
}

/** 生成失敗：AI 錯誤講人話；解析不出時聊天桌攤開原始輸出，VN 桌不攤（含私設與劇情，劇透鎖） */
export function GenesisError({ genesis, transport }: { genesis: TableGenesis; transport?: string }) {
  if (genesis.resultRaw === null && !genesis.error) return null;
  return (
    <section className="gen-result-error" role="alert">
      {genesis.error ? (
        <p>
          <AiErrorText text={genesis.error} transport={transport} />
        </p>
      ) : (
        <>
          {/* VN 不給看原文；VN 建桌少了開場白也走這裡（後端不建桌），大綱還在、按開桌就是重試 */}
          <p>{t(genesisFailKey(genesis))}</p>
          {genesis.tableKind !== "vn" && <pre>{genesis.resultRaw}</pre>}
        </>
      )}
    </section>
  );
}

function genesisFailKey(genesis: TableGenesis) {
  if (genesis.resultMessage === "character") return "genCharParseFail";
  return genesis.tableKind === "vn" && genesis.resultMessage === "expand" ? "vnExpandIncomplete" : "genParseFail";
}
