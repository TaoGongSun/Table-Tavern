// 玩家選的供應商、手選模型、各家金鑰都存在這個瀏覽器（localStorage）；存不了就只在這次有效。
// 供應商跟著目前語系分流：簡中列魔搭、矽基流動、OpenRouter，其他語系只有 OpenRouter（作者裁決 2026-11-03）。
// 選過且這個語系有列就以選的為準；否則已有 OpenRouter 金鑰的舊玩家照用 OpenRouter，簡中預設魔搭。
import type { Lang } from "../../i18n";
import { browserStorage, loadKey } from "../openrouter/key-store";
import { PROVIDER_IDS, type CnProviderId, type ProviderId } from "./providers";

const PROVIDER = "tt-web:provider";
const modelKey = (id: CnProviderId) => `tt-web:model:${id}`;
/** OpenRouter 沿用舊鍵名（舊玩家不用重登），其他家一家一格。 */
export const keyName = (id: CnProviderId) => `tt-web:key:${id}`;

export function read(name: string, storage = browserStorage("local")): string | null {
  try {
    return storage?.getItem(name) ?? null;
  } catch {
    return null;
  }
}

/** 回傳是否真的存進去了 */
export function write(name: string, value: string | null, storage = browserStorage("local")): boolean {
  try {
    if (value === null) storage?.removeItem(name);
    else storage?.setItem(name, value);
    return storage !== null;
  } catch {
    return false;
  }
}

/** 這個語系列出的供應商，依顯示順序 */
export const providersFor = (lang: Lang): ProviderId[] => (lang === "zh-CN" ? PROVIDER_IDS : ["openrouter"]);

export function storedProvider(storage = browserStorage("local")): ProviderId | null {
  const stored = read(PROVIDER, storage);
  return stored && (PROVIDER_IDS as string[]).includes(stored) ? (stored as ProviderId) : null;
}

export function effectiveProvider(lang: Lang, chosen: ProviderId | null, storage = browserStorage("local")): ProviderId {
  if (chosen && providersFor(lang).includes(chosen)) return chosen;
  if (lang === "zh-CN" && !loadKey(storage)) return "modelscope";
  return "openrouter";
}

export const rememberProvider = (id: ProviderId) => write(PROVIDER, id);
export const loadModelChoice = (id: CnProviderId) => read(modelKey(id));
export const rememberModelChoice = (id: CnProviderId, model: string | null) => write(modelKey(id), model);
