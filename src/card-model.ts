// 角色卡的資料契約：後端 character.rs 的欄位對應，畫面與 controller 共用。
// 只放跨檔案的型別與常數，元件私有的草稿型別跟著元件走。

export type Tier = "best" | "balanced" | "fast";

export interface CharacterMeta {
  id: string;
  name: string;
  color: string;
  avatar: string;
  tier: Tier;
  show_image: boolean;
  archived: boolean;
  // 換幕結算的自動隱藏（劇情帶出場就解除）；archived 是玩家手動封存，兩者正交
  auto_hidden: boolean;
}

export interface CharacterCard extends CharacterMeta {
  public_md: string;
  private_md: string;
  gen_prompt: string;
}

/** 新卡的陣營色輪播盤 */
export const PALETTE = ["#e07a5f", "#3d84a8", "#81b29a", "#f2a541", "#9b5de5", "#e56399"];

// 裁切完成的圖：bytes 給後端存檔、url 給畫面預覽（按儲存前只活在記憶體裡）
export type DraftImage = { bytes: number[]; url: string };
