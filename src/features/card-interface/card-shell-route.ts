// 卡片介面殼的選路：這桌現在該顯示哪一份殼。純函式，controller 只負責接線。
import { findShell, type CardInterface } from "./interface-card";
import { fillSkeletonPlaceholders, type StateNode } from "../refactor/refactor-shell";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";

export function pickCardShell(input: {
  /** 桌面玩法標記；undefined＝還不知道 */
  tableMode: string | null | undefined;
  /** AI 重構產的介面骨架（interface-shell.html）；null＝沒有 */
  refactorShell: string | null;
  events: TranscriptEvent[];
  tableTree: Record<string, StateNode>;
  cardInterfaces: CardInterface[];
}): string | null {
  const { tableMode, events, tableTree, cardInterfaces } = input;
  // 角色優先桌：介面產物一律不建不顯示（refactor-mode-split 拍板）——重構骨架、卡片自帶殼、
  // 掃 raw 的 fallback 整組短路。標記還沒讀回（undefined）也先不顯示，未知就放行會在角色桌
  // 切桌瞬間閃出介面。
  if (tableMode === undefined || tableMode === "characters") return null;
  const refactorShell =
    input.refactorShell !== null && input.refactorShell.trim() !== "" ? input.refactorShell : null;
  // 重構判定不接管介面（沒產殼）的 interface 桌不給面板，狀態看頂部狀態欄〔作者裁決 2026-10-02〕：
  // 原卡 HTML 靠 TavernHelper 讀訊息，app 沒墊，退回去只會停在載入中。direct-first、掃 raw、
  // 開場白三條退路都在這裡一起關；沒重構過的桌（null）照舊走卡片自帶殼。
  if (tableMode === "interface" && refactorShell === null) return null;
  if (refactorShell !== null) {
    const latestGm = [...events].reverse().find((event) => event.kind !== "player");
    if (latestGm !== undefined) {
      // 先照直玩語意讓卡腳本試原文：開場（選角）這類訊息卡自己就畫得出來，
      // 硬塞進骨架反而讓兩支腳本互咬（選角殼插進主介面模板中間，抽殼變碎片）
      const direct = findShell(cardInterfaces, [latestGm.raw ?? latestGm.text]);
      if (direct !== null) return direct;
      // 骨架照搬卡的每回合輸出格式，填值後過卡自己的顯示腳本；`{{本回合.正文}}` 吃最新一則 GM 正文
      const filled = fillSkeletonPlaceholders(refactorShell, {
        ...tableTree,
        本回合: { 正文: latestGm.text },
      });
      const fromSkeleton = findShell(cardInterfaces, [filled]);
      if (fromSkeleton !== null) return fromSkeleton;
    }
    // 剛開桌還沒有 GM 回合，或骨架沒過卡的顯示腳本：退回既有路徑（開場白選角殼等）
  }
  const recent = events
    .slice(-10)
    .filter((event) => event.kind !== "player")
    .reverse()
    .map((event) => event.raw ?? event.text);
  // 空桌退回卡片自己的開場白：這類卡的開場就是一整頁選角畫面，玩家得先在那裡選了才有第一句話
  const openings = events.length === 0 ? cardInterfaces.map((card) => card.opening) : [];
  return findShell(cardInterfaces, [...recent, ...openings]);
}
