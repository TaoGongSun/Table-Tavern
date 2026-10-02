// 卡片介面殼的選路：這桌現在該顯示哪一份殼、它是從哪一樓產生的。純函式，controller 只負責接線。
import { findShell, type CardInterface } from "./interface-card";
import { chatEvents, floorText, type CurrentFloor } from "./card-chat-shim";
import { fillSkeletonPlaceholders, type StateNode } from "../refactor/refactor-shell";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";

export interface PickedShell {
  shell: string;
  /** 產生這份殼的那一樓；讀訊息墊片的 getCurrentMessageId 回它 */
  current: CurrentFloor;
}

export function pickCardShell(input: {
  /** 桌面玩法標記；undefined＝還不知道 */
  tableMode: string | null | undefined;
  /** AI 重構產的介面骨架（interface-shell.html）；null＝沒有 */
  refactorShell: string | null;
  events: TranscriptEvent[];
  tableTree: Record<string, StateNode>;
  cardInterfaces: CardInterface[];
}): PickedShell | null {
  const { tableMode, tableTree, cardInterfaces } = input;
  // 角色優先桌：介面產物一律不建不顯示（refactor-mode-split 拍板）——重構骨架、卡片自帶殼、
  // 掃 raw 的 fallback 整組短路。標記還沒讀回（undefined）也先不顯示，未知就放行會在角色桌
  // 切桌瞬間閃出介面。
  if (tableMode === undefined || tableMode === "characters") return null;
  const refactorShell =
    input.refactorShell !== null && input.refactorShell.trim() !== "" ? input.refactorShell : null;
  // 樓號＝本場「樓」的位置（gm_only 事件不算一樓）；後面各條路都帶著原始樓號走，不拿候選序號當樓號。
  const floors = chatEvents(input.events).map((event, id) => ({ event, id }));
  const gmFloors = floors.filter(({ event }) => event.kind !== "player");
  const latestGm = gmFloors[gmFloors.length - 1];

  if (refactorShell !== null && latestGm !== undefined) {
    const raw = floorText(latestGm.event);
    const current = (text: string): CurrentFloor => ({
      id: latestGm.id,
      name: latestGm.event.speaker_name,
      text,
    });
    // 先照直玩語意讓卡腳本試原文：開場（選角）這類訊息卡自己就畫得出來，
    // 硬塞進骨架反而讓兩支腳本互咬（選角殼插進主介面模板中間，抽殼變碎片）
    const direct = findShell(cardInterfaces, [raw]);
    if (direct !== null) return { shell: direct.shell, current: current(raw) };
    // 骨架照搬卡的每回合輸出格式，填值後過卡自己的顯示腳本；`{{本回合.正文}}` 吃最新一則 GM 正文。
    // 這一樓交給卡片的是填值後的合成文字（唯一例外），讀本樓的殼才拿得到值。
    const filled = fillSkeletonPlaceholders(refactorShell, {
      ...tableTree,
      本回合: { 正文: latestGm.event.text },
    });
    const fromSkeleton = findShell(cardInterfaces, [filled]);
    if (fromSkeleton !== null) return { shell: fromSkeleton.shell, current: current(filled) };
    // 骨架沒過卡的顯示腳本：退回既有路徑
  }
  // 沒有骨架（沒重構過、判定不接管、或剛開桌）照原卡畫面：近 10 則掃原文，空桌退回卡片開場白
  // ——這類卡的開場就是一整頁選角畫面，玩家得先在那裡選了才有第一句話
  const recent = floors
    .slice(-10)
    .filter(({ event }) => event.kind !== "player")
    .reverse();
  const openings = floors.length === 0 ? cardInterfaces : [];
  const match = findShell(cardInterfaces, [
    ...recent.map(({ event }) => floorText(event)),
    ...openings.map((card) => card.opening),
  ]);
  if (match === null) return null;
  if (match.index < recent.length) {
    const { event, id } = recent[match.index];
    return { shell: match.shell, current: { id, name: event.speaker_name, text: floorText(event) } };
  }
  const card = openings[match.index - recent.length];
  return { shell: match.shell, current: { id: 0, name: card.character_name, text: card.opening ?? "" } };
}
