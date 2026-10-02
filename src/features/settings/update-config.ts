import { invoke } from "@tauri-apps/api/core";
import { AppConfig } from "../../shared/contracts/backend-contracts";

/**
 * 只送這次改動的鍵。後端以磁碟上的 JSON 為底合併：
 * 沒出現的鍵不動，null 刪鍵，空字串是合法值要留就送 ""。
 *
 * 全部呼叫端共用這一條鏈。前一筆 settle（成功或失敗）後才 invoke 下一筆，
 * 回傳也依呼叫順序 resolve。呼叫端照這個順序 setConfig，最後留下的是最新磁碟快照。
 */
let chain: Promise<unknown> = Promise.resolve();

export function updateConfig(patch: Record<string, unknown>): Promise<AppConfig> {
  const run = chain.then(() => invoke<AppConfig>("update_config", { patch }));
  chain = run.then(
    () => undefined,
    () => undefined,
  );
  return run;
}
