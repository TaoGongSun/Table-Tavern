// 重構雙軌定向（refactor-mode-split）：本機三態偵測。
// 素材是後端 card_interfaces 回傳的清單——介面腳本存在與否本機判得準，人物一律不用本機猜。
import { type CardInterface } from "./interface-card";

export type RefactorTristate = "supported" | "unsupported" | "none";

/**
 * 三態：只要有一張卡帶可用顯示腳本＝supported（進二選一）；沒有可用腳本但有 DRM／雲端
 * 載入器卡＝unsupported（擋下，不跑重構）；兩者皆無＝none（免問，直跑角色線）。
 * 混合桌取「還有得救」優先：有一張可接管就值得問玩家。
 */
export function detectRefactorTristate(cards: CardInterface[]): RefactorTristate {
  if (cards.some((card) => card.unsupported === null && card.scripts.length > 0)) return "supported";
  if (cards.some((card) => card.unsupported !== null)) return "unsupported";
  return "none";
}

export type RefactorMode = "interface" | "characters";

/** 後端 refactor_recommend 的初判結果（對照 refactor_ai.rs RefactorRecommendOutcome）。 */
export interface RefactorRecommendOutcome {
  recommend: string;
  evidence: string;
  raw: string;
}
