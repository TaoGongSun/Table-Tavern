// 一桌的世界書：這張卡會觸發哪些條目（各配穩定 ID）、從存檔接回的觸發狀態，以及匯出時的 world_info。
// 只有 ST 會觸發的卡內角色書（character_book）才算；條目的 key 指向匯入身分那條路會匯入的書（契約三）：
// 角色卡路＝books.character；世界書路只有書來自 character_book 時才是同一本，其餘（頂層 entries、人設轉成的
// 常駐條目）ST 不會當世界書觸發，不配 ID。
import type { WebSaveWorldInfo } from "@desktop/shared/contracts/web-save/web-save";
import { cardData } from "../cards/card-file";
import type { PlayCard } from "../cards/play-card";
import { assignEntryIds, worldInfoEntries, type WiEntry } from "../sillytavern/world-info-book";
import type { WiTimed } from "../sillytavern/world-info-scan";
import type { WorldInfoState } from "./st-text";

export interface TableWorldInfo {
  /** 存檔的 world_info.entries（穩定 ID ↔ 卡片契約的 key） */
  ids: { id: string; key: string }[];
  entries: WiEntry[];
  state: WorldInfoState;
}

function bookKeys(card: PlayCard): string[] {
  if (card.route === "character") return card.view.books.character?.entries.map((entry) => entry.key) ?? [];
  const book = card.view.books.worldbook;
  return book.source === "character_book" ? book.entries.map((entry) => entry.key) : [];
}

/** 存檔的 timed（契約檢查過）→ sticky／cooldown 兩張表。 */
export const timedFrom = (raw: WebSaveWorldInfo["timed"]): WiTimed => ({ sticky: { ...raw.sticky }, cooldown: { ...raw.cooldown } });

export function tableWorldInfo(card: PlayCard, saved: WebSaveWorldInfo): TableWorldInfo {
  const ids = assignEntryIds(bookKeys(card), saved.entries);
  return {
    ids,
    entries: worldInfoEntries(cardData(card.shell), ids),
    state: { timed: timedFrom(saved.timed), lastMessageId: saved.last_message_id, outlets: {} },
  };
}

/** 匯出的 world_info：計時算到的那則被刪掉了就當作還沒算過（契約要求指向存在的訊息）。 */
export function worldInfoForSave(table: TableWorldInfo, saved: WebSaveWorldInfo, messageIds: Set<string>): WebSaveWorldInfo {
  const last = table.state.lastMessageId;
  return {
    entries: table.ids,
    timed: { sticky: table.state.timed.sticky, cooldown: table.state.timed.cooldown },
    last_message_id: last !== null && messageIds.has(last) ? last : null,
    message_effects: saved.message_effects,
  };
}
