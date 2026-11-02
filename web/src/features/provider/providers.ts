// 模型供應商設定表。中國供應商的一鍵名單寫死（作者裁決 2026-11-02：沒有分數排行可依），挑選規則與理由見
// .ai/plans/cn-web-provider.md「預設模型排序」。名單只放不會扣錢的模型：不得有 Pro/、LoRA/ 前綴或 `:` 後綴。
import type { MsgKey } from "../../i18n";
import { MODELSCOPE_API, SILICONFLOW_API } from "../../shared/endpoints/origins";

export type ProviderId = "openrouter" | "modelscope" | "siliconflow";
export type CnProviderId = Exclude<ProviderId, "openrouter">;
export const PROVIDER_IDS: ProviderId[] = ["openrouter", "modelscope", "siliconflow"];

export interface LineupModel {
  id: string;
  /** 寫進提示預算的上下文長度（保守值，見方案） */
  contextLength: number;
}

export interface CnProvider {
  id: CnProviderId;
  apiBase: string;
  /** 貼令牌面板的步驟：文案鍵＋要開的官方頁面 */
  steps: { text: MsgKey; url?: string }[];
  /** 面板底下的一行補充說明 */
  note: MsgKey | null;
  lineup: LineupModel[];
  /** 存金鑰前打 /models 驗證（魔搭的 /models 不驗令牌，驗不了） */
  validateOnSave: boolean;
  /** 401 的說明 */
  authError: MsgKey;
  /** 403、429、忙線時補一句「也可能是額度不足」 */
  quotaHint: MsgKey;
}

/** 手選模型給的保守上下文（名單外的模型沒有目錄可查） */
export const UNKNOWN_CONTEXT = 32_768;

export const CN_PROVIDERS: Record<CnProviderId, CnProvider> = {
  modelscope: {
    id: "modelscope",
    apiBase: MODELSCOPE_API,
    steps: [
      { text: "msStep1", url: "https://modelscope.cn/" },
      { text: "msStep2", url: "https://www.modelscope.cn/docs/accounts/aliyun-binding-and-authorization" },
      { text: "msStep3", url: "https://modelscope.cn/my/settings/account" },
      { text: "msStep4", url: "https://modelscope.cn/my/myaccesstoken" },
    ],
    note: "msQuotaNote",
    lineup: [
      { id: "deepseek-ai/DeepSeek-V4.1-Flash", contextLength: 131_072 },
      { id: "Qwen/Qwen3.5-397B-A17B", contextLength: 131_072 },
      { id: "ZhipuAI/GLM-5.2", contextLength: 131_072 },
      { id: "MiniMax/MiniMax-M3", contextLength: 131_072 },
    ],
    validateOnSave: false,
    authError: "errAuthModelscope",
    quotaHint: "errMagicubeMaybe",
  },
  siliconflow: {
    id: "siliconflow",
    apiBase: SILICONFLOW_API,
    steps: [
      { text: "sfStep1", url: "https://cloud.siliconflow.cn/" },
      { text: "sfStep2", url: "https://cloud.siliconflow.cn/account/ak" },
    ],
    note: null,
    lineup: [
      { id: "XingChenAGI/Xing4.0-29B", contextLength: 32_768 },
      { id: "Qwen/Qwen3-8B", contextLength: 32_768 },
      { id: "THUDM/GLM-4-9B-0414", contextLength: 32_768 },
    ],
    validateOnSave: true,
    authError: "errAuthSiliconflow",
    quotaHint: "errQuotaMaybe",
  },
};

export const isCn = (id: ProviderId): id is CnProviderId => id !== "openrouter";

/** 確定走付費路徑的模型 ID：矽基流動的 Pro/、LoRA/ 收費版，魔搭帶 `:提供方` 後綴的外部計費 */
export const isPaidId = (id: string) => /^(Pro|LoRA)\//.test(id) || id.includes(":");

/**
 * 手選清單的標示：魔搭 API-Inference 只扣魔粒、不扣錢（帶 `:` 的不列），不標；矽基流動的 API 不給價格，
 * 名單內標免費、確定收費的標收費、其他一律標可能收費。
 */
export function priceTag(provider: CnProviderId, id: string): "free" | "paid" | "maybe-paid" | null {
  if (provider === "modelscope") return null;
  if (isPaidId(id)) return "paid";
  return CN_PROVIDERS[provider].lineup.some((model) => model.id === id) ? "free" : "maybe-paid";
}
