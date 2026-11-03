// 角色私設欄的劇透遮罩：「不看就重骰」只重生這個角色的私設與他持有的秘密；新私設交回編輯器寫進草稿與已存基線，
// 免得舊草稿一存檔就把新私設蓋回去。
import { invoke } from "@tauri-apps/api/core";
import type { ReactNode } from "react";
import type { CharacterCard } from "../../characters/card-model";
import { SpoilerMask } from "./SpoilerMask";
import { useBible } from "./useBible";

export function PrivateMask({
  worldId,
  characterId,
  onRerolled,
  children,
}: {
  worldId: string;
  characterId: string;
  onRerolled: (privateMd: string) => void;
  children: ReactNode;
}) {
  const { rerollSecrets } = useBible(worldId);
  async function reroll() {
    await rerollSecrets(characterId);
    const card = await invoke<CharacterCard>("read_character", { worldId, characterId });
    onRerolled(card.private_md);
  }
  return (
    <SpoilerMask worldId={worldId} onReroll={reroll}>
      {children}
    </SpoilerMask>
  );
}
