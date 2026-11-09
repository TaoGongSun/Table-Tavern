// 一位角色的立繪狀態與動作（精靈 ③ 與立繪分頁共用）：定稿候選的生成、挑選、上傳；
// 聽後端自動定稿的事件（plans/vn-setup-wizard.md §2 ③）重讀並顯示「生成中」。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { t, type MsgKey } from "../../i18n";
import type { SpriteSlotStatus, SpriteState } from "../../shared/contracts/backend-contracts";
import { explainAiError } from "../../shared/ui/ai-error";
import { backendText } from "../../shared/ui/backend-text";
import { onSpriteStatus } from "./useSprites";

/** 自動定稿事件的 expression 欄（後端 `prepare::BASE_EXPRESSION`） */
const BASE_EVENT = "base";

const SPRITE_ERRORS: Record<string, MsgKey> = {
  MISALIGNED: "vnSpriteMisaligned",
  UPLOAD_ASPECT: "vnSpriteUploadAspect",
  CHARACTER_GONE: "vnSpriteCharacterGone",
  VARIANT: "vnSpriteVariantInvalid",
  MATTING_DOWNLOAD: "vnSpriteMattingDownload",
  MATTING_MODEL_INVALID: "vnSpriteMattingDownload",
};

/** 後端 `VN_SPRITE_<碼>` 換成人話，其餘照通用錯誤 */
export function spriteError(raw: unknown): string {
  const explained = explainAiError(String(raw));
  if (explained) return t(explained);
  const code = /VN_SPRITE_([A-Z_]+)/.exec(String(raw))?.[1];
  return code && SPRITE_ERRORS[code] ? t(SPRITE_ERRORS[code]) : backendText(raw);
}

export const fileBytes = async (file: File) => Array.from(new Uint8Array(await file.arrayBuffer()));

export type SlotBusy = "generate" | "pick" | "upload" | "work" | null;

/** fill＝動作成功後排入缺的臉層（立繪分頁；精靈 ③ 不排，表情等 ④ 開演才補） */
export function useSpriteSlot(worldId: string, characterId: string, fill = false) {
  const [state, setState] = useState<SpriteState | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<SlotBusy>(null);
  const [error, setError] = useState<string | null>(null);
  /** 後端自動定稿：排隊中或生成中（失敗進 error） */
  const [auto, setAuto] = useState<"queued" | "running" | null>(null);
  const loaded = useRef<Record<string, string>>({});
  const ids = { worldId, characterId };

  const reload = useCallback(async () => {
    const next = await invoke<SpriteState>("vn_sprite_read", { worldId, characterId });
    // 只留後端還在的候選（後端限 6 張），被淘汰的預覽一起丟掉
    const kept: Record<string, string> = {};
    for (const file of next.candidates) {
      kept[file] =
        loaded.current[file] ??
        (await invoke<string>("vn_sprite_candidate", { worldId, characterId, file }));
    }
    loaded.current = kept;
    setPreviews(kept);
    setState(next);
    // 沿用目前選的；沒選或已淘汰：有定稿就選裝上去那張（上傳的不在候選裡＝不選，顯示定稿），沒定稿選最新候選
    const installed = next.sprite ? (next.sprite.meta.candidate ?? null) : undefined;
    setSelected((current) =>
      current && next.candidates.includes(current)
        ? current
        : installed === undefined
          ? (next.candidates[0] ?? null)
          : installed && next.candidates.includes(installed)
            ? installed
            : null,
    );
  }, [worldId, characterId]);

  useEffect(() => {
    reload().catch((reason) => setError(spriteError(reason)));
    // 掛上前就開始的自動定稿：事件已錯過，補讀一次
    const apply = (status: SpriteSlotStatus) => {
      if (status.characterId !== characterId || status.expression !== BASE_EVENT) return;
      setAuto(status.state === "queued" || status.state === "running" ? status.state : null);
      if (status.state === "failed") setError(spriteError(status.reason));
    };
    invoke<SpriteSlotStatus[]>("vn_sprites_status", { worldId })
      .then((all) => (all ?? []).forEach(apply))
      .catch(() => {});
    return onSpriteStatus(worldId, (status) => {
      apply(status);
      if (status.characterId === characterId && status.expression === BASE_EVENT && status.state === "done")
        reload().catch((reason) => setError(spriteError(reason)));
    });
  }, [reload, worldId, characterId]);

  async function run(kind: Exclude<SlotBusy, null>, work: () => Promise<void>) {
    if (busy) return;
    setBusy(kind);
    setError(null);
    try {
      await work();
      if (fill) await invoke("vn_sprites_fill", { worldId });
      await reload();
    } catch (reason) {
      setError(spriteError(reason));
    } finally {
      setBusy(null);
    }
  }

  return {
    worldId,
    characterId,
    run,
    state,
    selected,
    setSelected,
    previews,
    /** 目前定稿是哪張候選裝的（上傳的、舊桌＝null） */
    installed: state?.sprite?.meta.candidate ?? null,
    busy,
    auto,
    error,
    reload,
    generate: () =>
      run("generate", async () => {
        setSelected(await invoke<string>("vn_sprite_generate", ids));
      }),
    pick: () =>
      run("pick", async () => {
        if (!selected) return;
        await invoke("vn_sprite_pick", { ...ids, file: selected });
      }),
    upload: (file: File, expression: string | null = null, variant: string | null = null) =>
      run("upload", async () => {
        await invoke("vn_sprite_upload", { ...ids, variant, expression, data: await fileBytes(file) });
        // 換定稿：改顯示上傳的那張（不再指著先前選的候選）
        if (expression === null) setSelected(null);
      }),
  };
}

export type SpriteSlot = ReturnType<typeof useSpriteSlot>;
