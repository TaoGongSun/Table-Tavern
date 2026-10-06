// Claude 訂閱進入超額用量的一次性提示（claude-1h-cache）。倍數只在確定整段寫成 1 小時時才講；
// 觀測不到用量就不寫倍數，也不從用量推斷是誰的設定。
const COPY = {
  "zh-TW": {
    claudeOverageOneHour:
      "Claude 訂閱額度已用完，這輪起按超額計費；這輪的快取以 1 小時寫入，每次寫入約為一般輸入的 2 倍價格。",
    claudeOverageOther:
      "Claude 訂閱額度已用完，這輪起按超額計費；快取寫入按實際時效計價（5 分鐘約 1.25 倍、1 小時約 2 倍）。",
    claudeOverageUnknown: "Claude 訂閱額度已用完，這輪起按超額計費。",
  },
  "zh-CN": {
    claudeOverageOneHour:
      "Claude 订阅额度已用完，本轮起按超额计费；本轮的缓存以 1 小时写入，每次写入约为普通输入的 2 倍价格。",
    claudeOverageOther:
      "Claude 订阅额度已用完，本轮起按超额计费；缓存写入按实际时效计价（5 分钟约 1.25 倍、1 小时约 2 倍）。",
    claudeOverageUnknown: "Claude 订阅额度已用完，本轮起按超额计费。",
  },
  en: {
    claudeOverageOneHour:
      "Your Claude subscription limit is used up; from this turn on you are billed for extra usage. This turn's cache was written for 1 hour, and each cache write costs about 2× normal input.",
    claudeOverageOther:
      "Your Claude subscription limit is used up; from this turn on you are billed for extra usage. Cache writes are priced by their actual lifetime (about 1.25× for 5 minutes, about 2× for 1 hour).",
    claudeOverageUnknown:
      "Your Claude subscription limit is used up; from this turn on you are billed for extra usage.",
  },
  ja: {
    claudeOverageOneHour:
      "Claude サブスクリプションの上限に達しました。このターンから追加使用量として課金されます。このターンのキャッシュは 1 時間で書き込まれ、書き込みごとに通常入力の約 2 倍の料金です。",
    claudeOverageOther:
      "Claude サブスクリプションの上限に達しました。このターンから追加使用量として課金されます。キャッシュ書き込みは実際の保持時間で課金されます（5 分は約 1.25 倍、1 時間は約 2 倍）。",
    claudeOverageUnknown:
      "Claude サブスクリプションの上限に達しました。このターンから追加使用量として課金されます。",
  },
  ko: {
    claudeOverageOneHour:
      "Claude 구독 한도를 모두 사용했습니다. 이번 턴부터 초과 사용량으로 과금됩니다. 이번 턴의 캐시는 1시간으로 기록되며, 기록할 때마다 일반 입력의 약 2배 요금입니다.",
    claudeOverageOther:
      "Claude 구독 한도를 모두 사용했습니다. 이번 턴부터 초과 사용량으로 과금됩니다. 캐시 기록은 실제 유지 시간에 따라 과금됩니다(5분 약 1.25배, 1시간 약 2배).",
    claudeOverageUnknown: "Claude 구독 한도를 모두 사용했습니다. 이번 턴부터 초과 사용량으로 과금됩니다.",
  },
  es: {
    claudeOverageOneHour:
      "Agotaste el límite de tu suscripción de Claude; desde este turno se cobra como uso adicional. La caché de este turno se escribió por 1 hora y cada escritura cuesta unas 2 veces la entrada normal.",
    claudeOverageOther:
      "Agotaste el límite de tu suscripción de Claude; desde este turno se cobra como uso adicional. Las escrituras de caché se cobran según su duración real (unas 1,25 veces para 5 minutos, unas 2 veces para 1 hora).",
    claudeOverageUnknown:
      "Agotaste el límite de tu suscripción de Claude; desde este turno se cobra como uso adicional.",
  },
  "pt-BR": {
    claudeOverageOneHour:
      "O limite da sua assinatura do Claude acabou; a partir deste turno a cobrança é de uso extra. O cache deste turno foi gravado por 1 hora, e cada gravação custa cerca de 2× a entrada normal.",
    claudeOverageOther:
      "O limite da sua assinatura do Claude acabou; a partir deste turno a cobrança é de uso extra. Gravações de cache são cobradas pela duração real (cerca de 1,25× para 5 minutos, cerca de 2× para 1 hora).",
    claudeOverageUnknown:
      "O limite da sua assinatura do Claude acabou; a partir deste turno a cobrança é de uso extra.",
  },
  de: {
    claudeOverageOneHour:
      "Dein Claude-Abo-Kontingent ist aufgebraucht; ab diesem Zug wird zusätzliche Nutzung berechnet. Der Cache dieses Zugs wurde für 1 Stunde geschrieben, jeder Schreibvorgang kostet etwa das 2-Fache normaler Eingabe.",
    claudeOverageOther:
      "Dein Claude-Abo-Kontingent ist aufgebraucht; ab diesem Zug wird zusätzliche Nutzung berechnet. Cache-Schreibvorgänge werden nach tatsächlicher Lebensdauer berechnet (5 Minuten etwa 1,25-fach, 1 Stunde etwa 2-fach).",
    claudeOverageUnknown:
      "Dein Claude-Abo-Kontingent ist aufgebraucht; ab diesem Zug wird zusätzliche Nutzung berechnet.",
  },
  fr: {
    claudeOverageOneHour:
      "Le quota de votre abonnement Claude est épuisé ; à partir de ce tour, l'usage supplémentaire est facturé. Le cache de ce tour a été écrit pour 1 heure, chaque écriture coûte environ 2 fois l'entrée normale.",
    claudeOverageOther:
      "Le quota de votre abonnement Claude est épuisé ; à partir de ce tour, l'usage supplémentaire est facturé. Les écritures de cache sont facturées selon leur durée réelle (environ 1,25 fois pour 5 minutes, environ 2 fois pour 1 heure).",
    claudeOverageUnknown:
      "Le quota de votre abonnement Claude est épuisé ; à partir de ce tour, l'usage supplémentaire est facturé.",
  },
  ru: {
    claudeOverageOneHour:
      "Лимит подписки Claude исчерпан; с этого хода оплачивается дополнительное использование. Кэш этого хода записан на 1 час, каждая запись стоит примерно в 2 раза дороже обычного ввода.",
    claudeOverageOther:
      "Лимит подписки Claude исчерпан; с этого хода оплачивается дополнительное использование. Запись кэша оплачивается по фактическому сроку (5 минут — примерно в 1,25 раза, 1 час — примерно в 2 раза дороже).",
    claudeOverageUnknown:
      "Лимит подписки Claude исчерпан; с этого хода оплачивается дополнительное использование.",
  },
} as const;

type ClaudeOverageLang = keyof typeof COPY;

export type ClaudeOverageMsgKey = keyof (typeof COPY)["zh-TW"];
export const CLAUDE_OVERAGE_MESSAGE_KEYS = Object.keys(COPY["zh-TW"]) as ClaudeOverageMsgKey[];

export function isClaudeOverageMsgKey(key: string): key is ClaudeOverageMsgKey {
  // 與其他補充字典一樣容忍非字串鍵（既有呼叫端可能傳 undefined 進 t()）
  return typeof key === "string" && key in COPY["zh-TW"];
}

export function claudeOverageMessage(lang: ClaudeOverageLang, key: ClaudeOverageMsgKey): string {
  return COPY[lang][key];
}
