// 「去哪裡找卡」的站點清單（D6：只放連結，不打站方 API）。爬別站、另存別站卡的站不收。
// 查證見計畫 4.2；站名是專有名詞不翻，說明走文案鍵。
import type { MsgKey } from "../../i18n";

export interface CardSite {
  id: string;
  name: string;
  url: string;
  blurb: MsgKey;
}

export const CARD_SITES: CardSite[] = [
  { id: "risu-realm", name: "RisuRealm", url: "https://realm.risuai.net/", blurb: "site_risuRealm" },
  { id: "chub", name: "Chub", url: "https://chub.ai/", blurb: "site_chub" },
  { id: "ai-character-cards", name: "AICharacterCards", url: "https://aicharactercards.com/", blurb: "site_aiCharacterCards" },
  { id: "character-tavern", name: "Character Tavern", url: "https://character-tavern.com/", blurb: "site_characterTavern" },
  { id: "pygmalion", name: "Pygmalion", url: "https://pygmalion.chat/", blurb: "site_pygmalion" },
];
