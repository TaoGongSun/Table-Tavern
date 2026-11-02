// 玩家選的供應商、手選模型、各家金鑰都存在這個瀏覽器（localStorage）；存不了就只在這次有效。
// 沒選過供應商才看預設：已有 OpenRouter 金鑰的舊玩家照用 OpenRouter，否則簡中預設魔搭、其他語系 OpenRouter
// （作者裁決 2026-11-02）。選過就以存的為準，換語系不改。
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

export function initialProvider(lang: Lang, storage = browserStorage("local")): ProviderId {
  const stored = read(PROVIDER, storage);
  if (stored && (PROVIDER_IDS as string[]).includes(stored)) return stored as ProviderId;
  if (loadKey(storage)) return "openrouter";
  return lang === "zh-CN" ? "modelscope" : "openrouter";
}

export const rememberProvider = (id: ProviderId) => write(PROVIDER, id);
export const loadModelChoice = (id: CnProviderId) => read(modelKey(id));
export const rememberModelChoice = (id: CnProviderId, model: string | null) => write(modelKey(id), model);
