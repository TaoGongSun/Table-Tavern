// 側欄卡片主區／隱藏區分流：手動封存（archived）與登場自動隱藏（auto_hidden）是兩條正交的隱藏理由，
// 這裡把兩者收斂成一個判準給 App.tsx 用。純函式、零 UI／invoke 依賴——判斷邏輯在這裡單獨測。

/** 分流只用得到這三個欄位，不吃整個 CharacterMeta（也免得這裡要 import App.tsx 的型別）。 */
export interface CharacterVisibilityFlags {
  id: string;
  archived: boolean;
  auto_hidden: boolean;
  /** 舞台卡：不收進自動隱藏區，也不能當發言對象 */
  stage?: boolean;
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
  if (character.stage) return false;
  return character.auto_hidden && !sceneAppearances.has(character.id);
}

/**
 * 有效發言對象：選的對象是舞台卡（剛切成舞台、重構套用）時一律當沒有對象。導覽 controller 對外只給這一份，
 * 輸入框的名字／頭像／提示、側欄選取、送出與卡片介面的 /send、/trigger 共用，玩家句不會先落檔再被後端拒收。
 * GM 代號與空字串原樣。
 */
export function effectiveSpeaker(
  speaker: string,
  metaOf: (id: string) => { stage?: boolean } | undefined,
): string {
  return metaOf(speaker)?.stage === true ? "" : speaker;
}

/** 預設發言對象：主區第一張不是舞台卡的角色；沒有就 undefined（呼叫端退回 GM 或空）。 */
export function firstSpeakerCandidate<T extends CharacterVisibilityFlags>(
  cast: readonly T[],
  sceneAppearances: ReadonlySet<string>,
): T | undefined {
  return cast.find((character) => !character.stage && !isCharacterHidden(character, sceneAppearances));
}
