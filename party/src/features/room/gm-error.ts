// GM 生成失敗的種類 → 一句人話（room-host-actions）。種類由執行端 `classify`（src-tauri/src/room/run.rs）分好，這裡不再認碼：
// 桌面版資訊列「最近錯誤」與重生時的換模型選單共用。模型本身用不了（下架、金鑰／登入）就該換模型；暫時性的原模型直接重試。
import type { ExecutorErrorKind } from "../../../server/core/types";
import { t, type MsgKey } from "../../i18n";

const TEXT: Record<ExecutorErrorKind, MsgKey> = {
  model_unavailable: "gmErrGone",
  not_ready: "gmErrAuth",
  quota: "gmErrQuota",
  provider_unreachable: "gmErrNetwork",
  refusal: "gmErrRefusal",
  scene_full: "gmErrSceneFull",
  tool_format: "gmErrTool",
  other: "gmErrOther",
};

/** 這個模型本身用不了：重生時灰掉它、預設換別的 */
export const modelBroken = (kind: ExecutorErrorKind) =>
  kind === "model_unavailable" || kind === "not_ready";
/** `kind` 為 null＝不是 GM 生成失敗（房間連線、名冊、種子、同步等），只講房間出了問題 */
export const gmErrorText = (kind: ExecutorErrorKind | null, code: string) =>
  t(kind ? TEXT[kind] : "roomErrOther", { code: code.slice(0, 40) });
