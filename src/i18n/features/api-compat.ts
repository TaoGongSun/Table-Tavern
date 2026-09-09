// API 相容格式只服務設定頁的進階區；不塞回十份大型主字典。
// i18n/index.ts 的 t() 會把這份補充字典與主字典視為同一個 MsgKey 空間。
const COPY = {
  "zh-TW": {
    summary: "進階：API 相容設定",
    label: "API 格式",
    auto: "自動（相容舊設定）",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "「自動」只有在自訂 base URL 本身以 /responses 結尾時才使用 Responses；其他情況維持既有 /chat/completions 行為。",
  },
  "zh-CN": {
    summary: "高级：API 兼容设置",
    label: "API 格式",
    auto: "自动（兼容旧设置）",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "“自动”只有在自定义 base URL 本身以 /responses 结尾时才使用 Responses；其他情况保持原有 /chat/completions 行为。",
  },
  en: {
    summary: "Advanced: API compatibility",
    label: "API format",
    auto: "Auto (backward-compatible)",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "Auto uses Responses only when the custom base URL itself ends in /responses; otherwise it keeps the existing /chat/completions behavior.",
  },
  ja: {
    summary: "詳細設定：API互換設定",
    label: "API形式",
    auto: "自動（従来設定と互換）",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "自動では、カスタム base URL 自体が /responses で終わる場合のみ Responses を使います。それ以外は従来の /chat/completions を維持します。",
  },
  ko: {
    summary: "고급: API 호환 설정",
    label: "API 형식",
    auto: "자동(기존 설정 호환)",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "자동은 사용자 지정 base URL 자체가 /responses로 끝날 때만 Responses를 사용하고, 그 외에는 기존 /chat/completions 동작을 유지합니다.",
  },
  es: {
    summary: "Avanzado: compatibilidad de API",
    label: "Formato de API",
    auto: "Automático (compatible con versiones anteriores)",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "El modo automático usa Responses solo si la URL base personalizada termina en /responses; en los demás casos mantiene /chat/completions.",
  },
  "pt-BR": {
    summary: "Avançado: compatibilidade da API",
    label: "Formato da API",
    auto: "Automático (compatível com configurações antigas)",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "O modo Automático usa Responses apenas quando a URL base personalizada termina em /responses; caso contrário, mantém /chat/completions.",
  },
  de: {
    summary: "Erweitert: API-Kompatibilität",
    label: "API-Format",
    auto: "Automatisch (abwärtskompatibel)",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "Automatisch verwendet Responses nur, wenn die benutzerdefinierte Basis-URL auf /responses endet; sonst bleibt das bisherige /chat/completions-Verhalten erhalten.",
  },
  fr: {
    summary: "Avancé : compatibilité API",
    label: "Format API",
    auto: "Auto (rétrocompatible)",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "Le mode Auto utilise Responses uniquement si l’URL de base personnalisée se termine par /responses ; sinon il conserve le comportement /chat/completions existant.",
  },
  ru: {
    summary: "Дополнительно: совместимость API",
    label: "Формат API",
    auto: "Авто (обратная совместимость)",
    chatCompletions: "Chat Completions (/chat/completions)",
    responses: "Responses (/responses)",
    hint: "Авто использует Responses только если пользовательский base URL оканчивается на /responses; иначе сохраняется прежнее поведение /chat/completions.",
  },
} as const;

type ApiCompatLang = keyof typeof COPY;

const ACCESSORS = {
  apiCompatAdvancedSummary: (copy: (typeof COPY)[ApiCompatLang]) => copy.summary,
  apiFormatLabel: (copy: (typeof COPY)[ApiCompatLang]) => copy.label,
  apiFormatAuto: (copy: (typeof COPY)[ApiCompatLang]) => copy.auto,
  apiFormatChatCompletions: (copy: (typeof COPY)[ApiCompatLang]) => copy.chatCompletions,
  apiFormatResponses: (copy: (typeof COPY)[ApiCompatLang]) => copy.responses,
  apiFormatHint: (copy: (typeof COPY)[ApiCompatLang]) => copy.hint,
} as const;

export type ApiCompatMsgKey = keyof typeof ACCESSORS;

export function isApiCompatMsgKey(key: string): key is ApiCompatMsgKey {
  return key in ACCESSORS;
}

export function apiCompatMessage(lang: ApiCompatLang, key: ApiCompatMsgKey): string {
  return ACCESSORS[key](COPY[lang]);
}
