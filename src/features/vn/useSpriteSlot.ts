// 一位角色的立繪狀態與動作（精靈 ③ 與立繪分頁共用）：定稿候選的生成、挑選、上傳。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { t, type MsgKey } from "../../i18n";
import type { SpriteState } from "../../shared/contracts/backend-contracts";
import { backendText } from "../../shared/ui/backend-text";

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
  // 這次掛載裝成定稿的那張候選：同一張不必再挑（「挑定這張」變灰）
  const [pickedFile, setPickedFile] = useState<string | null>(null);
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
    setSelected((current) =>
      current && next.candidates.includes(current) ? current : (next.candidates[0] ?? null),
    );
  }, [worldId, characterId]);

  useEffect(() => {
    reload().catch((reason) => setError(spriteError(reason)));
  }, [reload]);

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
    pickedFile,
    busy,
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
        setPickedFile(selected);
      }),
    upload: (file: File, expression: string | null = null, variant: string | null = null) =>
      run("upload", async () => {
        await invoke("vn_sprite_upload", { ...ids, variant, expression, data: await fileBytes(file) });
      }),
  };
}

export type SpriteSlot = ReturnType<typeof useSpriteSlot>;
