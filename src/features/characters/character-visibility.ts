// 側欄卡片主區／隱藏區分流：手動封存（archived）與登場自動隱藏（auto_hidden）是兩條正交的隱藏理由，
// 這裡把兩者收斂成一個判準給 App.tsx 用。純函式、零 UI／invoke 依賴——判斷邏輯在這裡單獨測。

/** 分流只用得到這三個欄位，不吃整個 CharacterMeta（也免得這裡要 import App.tsx 的型別）。 */
export interface CharacterVisibilityFlags {
  id: string;
  archived: boolean;
  auto_hidden: boolean;
}

/**
 * true＝這張卡目前該落在隱藏區。手動封存一律隱藏；auto_hidden 卡本幕若已出場
 * （sceneAppearances 含其 id，來源是載入時的 scene_appearances 與 gm_narrate 回傳的
 * arrived_characters）就當主區卡——劇情已經帶它上場，不必等下一次換幕才把它放出來。
 */
export function isCharacterHidden(
  character: CharacterVisibilityFlags,
  sceneAppearances: ReadonlySet<string>,
): boolean {
  if (character.archived) return true;
  return character.auto_hidden && !sceneAppearances.has(character.id);
}
