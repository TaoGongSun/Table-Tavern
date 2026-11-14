// 一句話開桌的草稿與三支生成流程（大綱、AI 加一位、展開建桌）：聊天桌的 GenerateTableDialog 與 VN 精靈共用。
// 草稿活在呼叫端的元件裡：對話窗關掉只是不畫、VN 精靈換步驟不卸載，草稿都留著。
import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";
import type { PlayerStyle, TableKind, VnPreferences } from "../../shared/contracts/backend-contracts";

export interface OutlineCharacter {
  name: string;
  tagline: string;
}

export interface GeneratedOutline {
  title: string;
  world: string;
  characters: OutlineCharacter[];
  /** VN 桌的主角（AI 提出、精靈 ② 可改）；聊天桌沒有 */
  player?: OutlineCharacter | null;
}

interface Outcome<T> {
  parsed: T | null;
  raw: string;
}

export type GenesisBusy = "outline" | "character" | "expand" | null;

function serializeOutline(outline: GeneratedOutline): string {
  const sections = [`## WORLD: ${outline.title.trim()}\n${outline.world.trim()}`];
  const player = outline.player;
  if (player && (player.name.trim() || player.tagline.trim())) {
    sections.push(`## PLAYER: ${player.name.trim()}\n${player.tagline.trim()}`);
  }
  for (const character of outline.characters) {
    const name = character.name.trim();
    if (name) sections.push(`## CHARACTER: ${name}\n${character.tagline.trim()}`);
  }
  return sections.join("\n\n");
}

/** 開房生成才帶；`cast`＝AI 先生好可入座的角色。聊天桌與 VN 傳 null。 */
export interface RoomGenesis {
  cast: boolean;
}

/** `preferences` 有值＝VN 精靈：後端收空白點子、偏好進提示詞；genres 是介面語言的題材文字 */
export function useTableGenesis(tableKind: TableKind, room: RoomGenesis | null = null) {
  const roomRef = useRef(room);
  roomRef.current = room;
  const [input, setInput] = useState("");
  const [genres, setGenres] = useState<string[]>([]);
  const [outline, setOutline] = useState<GeneratedOutline | null>(null);
  const [outlineRaw, setOutlineRaw] = useState<string | null>(null);
  const [resultRaw, setResultRaw] = useState<string | null>(null);
  const [resultMessage, setResultMessage] = useState<"outline" | "character">("outline");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState<GenesisBusy>(null);
  const [hint, setHint] = useState("");

  /** 回有沒有拿到可改的大綱 */
  async function generateOutline(genreLabels: string[], preferences: VnPreferences | null = null) {
    const idea = input.trim();
    if (!preferences && !idea && genreLabels.length === 0) return false;
    setBusy("outline");
    setOutline(null);
    setOutlineRaw(null);
    setResultRaw(null);
    setResultMessage("outline");
    setError("");
    try {
      const result = await invoke<Outcome<GeneratedOutline>>("generate_table_outline", {
        input: idea,
        genres: genreLabels,
        preferences,
        room: roomRef.current,
      });
      setOutlineRaw(result.raw);
      if (result.parsed) setOutline(result.parsed);
      else setResultRaw(result.raw);
      return result.parsed !== null;
    } catch (reason) {
      setError(String(reason));
      return false;
    } finally {
      setBusy(null);
    }
  }

  async function addCharacter(genreLabels: string[]) {
    if (!outline) return;
    setBusy("character");
    setResultRaw(null);
    setResultMessage("character");
    setError("");
    try {
      const result = await invoke<Outcome<OutlineCharacter>>("generate_table_character", {
        input: input.trim(),
        genres: genreLabels,
        outlineRaw: serializeOutline(outline),
        hint,
        room: roomRef.current,
      });
      if (result.parsed) {
        const character = result.parsed;
        setOutline((current) => current && { ...current, characters: [...current.characters, character] });
        setHint("");
      } else {
        setResultRaw(result.raw);
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(null);
    }
  }

  /** 換模式時舊大綱作廢。點子與題材留著，玩家重按生成即可。 */
  function discardOutline() {
    setOutline(null);
    setOutlineRaw(null);
    setResultRaw(null);
  }

  const canCreate = Boolean(outline && outline.title.trim() && outline.world.trim());

  /** 展開成桌；成功回新桌 id（草稿清掉），失敗或解析不出回 null（錯誤留在畫面上） */
  async function createTable(
    genreLabels: string[],
    vn: { preferences: VnPreferences; spoilerLock: boolean; artStyle?: PlayerStyle } | null = null,
  ): Promise<string | null> {
    if (!outline || !canCreate) return null;
    setBusy("expand");
    setError("");
    setResultRaw(null);
    setResultMessage("outline");
    try {
      const roomNow = roomRef.current;
      const draft = roomNow?.cast === false ? { ...outline, characters: [] } : outline;
      const result = await invoke<{ worldId: string | null; raw: string }>("generate_table_expand", {
        input: input.trim(),
        genres: genreLabels,
        title: outline.title.trim(),
        outlineRaw: serializeOutline(draft),
        tableKind,
        preferences: vn?.preferences ?? null,
        spoilerLock: vn?.spoilerLock ?? null,
        artStyle: vn?.artStyle ?? null,
        room: roomNow,
      });
      if (!result.worldId) {
        setResultRaw(result.raw);
        return null;
      }
      setOutline(null);
      setOutlineRaw(null);
      setResultRaw(null);
      return result.worldId;
    } catch (reason) {
      setError(String(reason));
      return null;
    } finally {
      setBusy(null);
    }
  }

  return {
    tableKind,
    input,
    setInput,
    genres,
    setGenres,
    outline,
    setOutline,
    hasOutline: outlineRaw !== null,
    resultRaw,
    resultMessage,
    error,
    busy,
    hint,
    setHint,
    canCreate,
    discardOutline,
    generateOutline,
    addCharacter,
    createTable,
  };
}

export type TableGenesis = ReturnType<typeof useTableGenesis>;
