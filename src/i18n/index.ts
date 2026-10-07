// UI 語系字典入口：zh-TW 為正典（zh-TW.ts），其他語系逐鍵對應，缺鍵時 TypeScript 直接報錯。
// 語系存 config.preferences.language；App 每次 render 前呼叫 setLang 同步，元件一律經 t() 取字串。
// 模型輸出語言規範在後端 transport.rs，不在這裡。
//
// 新增語系：建 <code>.ts（照 en.ts 格式）→ 在 MESSAGES 與 languages.ts（語系清單，網頁版共用）各加一行。
// 同一語系另需後端 language_rule(transport.rs) 與範例桌內容(src-tauri/samples/<code>.json)，
// 三處缺一律不上該語系。

import { zh, type MsgKey as CoreMsgKey } from "./zh-TW";
import { en } from "./en";
import { zhCN } from "./zh-CN";
import { ja } from "./ja";
import { ko } from "./ko";
import { es } from "./es";
import { ptBR } from "./pt-BR";
import { de } from "./de";
import { fr } from "./fr";
import { ru } from "./ru";
import {
  apiCompatMessage,
  isApiCompatMsgKey,
  type ApiCompatMsgKey,
} from "./features/api-compat";
import {
  isOpenRouterOnboardingMsgKey,
  openRouterOnboardingMessage,
  type OpenRouterOnboardingMsgKey,
} from "./features/openrouter-onboarding";
import { isSmartFreeMsgKey, smartFreeMessage, type SmartFreeMsgKey } from "./features/smart-free";
import {
  isResponseTruncatedMsgKey,
  responseTruncatedMessage,
  type ResponseTruncatedMsgKey,
} from "./features/response-truncated";
import { backendMsgMessage, isBackendMsgKey, type BackendMsgKey } from "./features/backend-msg";
import {
  isTableDataMsgKey,
  tableDataMsgMessage,
  type TableDataMsgKey,
} from "./features/backend-msg-table";
import {
  isUpdaterMsgKey,
  updaterMsgMessage,
  type UpdaterMsgKey,
} from "./features/backend-msg-updater";
import { aiMsgMessage, isAiMsgKey, type AiMsgKey } from "./features/backend-msg-ai";
import { isNoteMsgKey, noteMsgMessage, type NoteMsgKey } from "./features/backend-msg-notes";
import {
  isTranscriptMarkerMsgKey,
  transcriptMarkerMessage,
  type TranscriptMarkerMsgKey,
} from "./features/transcript-marker";
import {
  claudeOverageMessage,
  isClaudeOverageMsgKey,
  type ClaudeOverageMsgKey,
} from "./features/claude-overage";
import { expandPlural } from "./plural";
import { detectLang, LANGUAGE_OPTIONS, normalizeLang, type Lang } from "./languages";

export type MsgKey =
  | CoreMsgKey
  | OpenRouterOnboardingMsgKey
  | ApiCompatMsgKey
  | SmartFreeMsgKey
  | ResponseTruncatedMsgKey
  | BackendMsgKey
  | TableDataMsgKey
  | UpdaterMsgKey
  | AiMsgKey
  | NoteMsgKey
  | TranscriptMarkerMsgKey
  | ClaudeOverageMsgKey;

const MESSAGES = {
  "zh-TW": zh,
  "zh-CN": zhCN,
  en,
  ja,
  ko,
  es,
  "pt-BR": ptBR,
  de,
  fr,
  ru,
} satisfies Record<Lang, Record<CoreMsgKey, string>>;

export { detectLang, LANGUAGE_OPTIONS, normalizeLang, type Lang };

let lang: Lang = "zh-TW";

export function setLang(next: Lang) {
  lang = next;
}

export function t(key: MsgKey, params?: Record<string, string | number>): string {
  const raw: string = isOpenRouterOnboardingMsgKey(key)
    ? openRouterOnboardingMessage(lang, key)
    : isApiCompatMsgKey(key)
      ? apiCompatMessage(lang, key)
      : isSmartFreeMsgKey(key)
        ? smartFreeMessage(lang, key)
        : isResponseTruncatedMsgKey(key)
          ? responseTruncatedMessage(lang)
          : isBackendMsgKey(key)
            ? backendMsgMessage(lang, key)
            : isTableDataMsgKey(key)
              ? tableDataMsgMessage(lang, key)
              : isUpdaterMsgKey(key)
                ? updaterMsgMessage(lang, key)
                : isAiMsgKey(key)
                  ? aiMsgMessage(lang, key)
                  : isNoteMsgKey(key)
                    ? noteMsgMessage(lang, key)
                    : isTranscriptMarkerMsgKey(key)
                      ? transcriptMarkerMessage(lang, key)
                      : isClaudeOverageMsgKey(key)
                        ? claudeOverageMessage(lang, key)
                        : MESSAGES[lang][key as CoreMsgKey];
  return formatMessage(raw, lang, params);
}

/** 模板代入：單複數先在模板上選好分支（# 變回 {參數}），再單次代入參數；plural 壞掉就原樣回傳。 */
export function formatMessage(
  template: string,
  forLang: Lang,
  params?: Record<string, string | number>,
): string {
  const text = expandPlural(template, forLang, params);
  // plural 語法壞掉：整串原樣回傳，不半代入（字典語法由 check:i18n 擋）
  if (text === null) return template;
  if (!params) return text;
  // 對模板只掃一次：代入的值裡就算有 {名} 也不會再被換；沒給的佔位符原樣留著。
  return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
    Object.prototype.hasOwnProperty.call(params, name) ? String(params[name]) : whole,
  );
}
