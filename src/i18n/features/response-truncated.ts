const COPY = {
  "zh-TW": "（回應被中途中斷）",
  "zh-CN": "（回应被中途中断）",
  en: "(Response interrupted)",
  ja: "（応答が途中で中断されました）",
  ko: "(응답이 중간에 중단됨)",
  es: "(Respuesta interrumpida)",
  "pt-BR": "(Resposta interrompida)",
  de: "(Antwort wurde unterbrochen)",
  fr: "(Réponse interrompue)",
  ru: "(Ответ был прерван)",
} as const;

type ResponseTruncatedLang = keyof typeof COPY;

export type ResponseTruncatedMsgKey = "responseTruncated";

export function isResponseTruncatedMsgKey(key: string): key is ResponseTruncatedMsgKey {
  return key === "responseTruncated";
}

export function responseTruncatedMessage(lang: ResponseTruncatedLang): string {
  return COPY[lang];
}
