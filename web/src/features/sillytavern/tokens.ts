// 一則提示訊息估多少 token：照 SillyTavern 釘版本 06bde939 的計數結構——前端 public/scripts/tokenizers.js
// （getTokenizerModel 的 OpenRouter 分支、countTokensOpenAIAsync 的 -2）＋伺服器 src/endpoints/tokenizers.js
// （/openai/count 依 tokenizer 分三條路）。網頁版沒有 ST 伺服器上的真 tokenizer，計數結構照搬、每個欄位（或整段）
// 「幾個 token」改用 ST 的 guesstimate 公式（UTF-8 位元組 ÷ 3.35 進位）逐欄位算——ST 載不到 tokenizer 時的退路本身
// 是對整份 JSON 算一次，這裡不是那條退路。之後要換真 tokenizer 只換 `TextTokens`。
// 估算與真 tokenizer 有落差（計畫 4b 風險欄），邊界附近仍可能被平台拒收。

/** 一段文字幾個 token。 */
export type TextTokens = (text: string) => number;

const BYTES_PER_TOKEN = 3.35;
const utf8 = new TextEncoder();

/** ST 的 guesstimate：`Math.ceil(Buffer.byteLength(str, 'utf8') / BYTES_PER_TOKEN)` */
export const guesstimate: TextTokens = (text) => Math.ceil(utf8.encode(text).length / BYTES_PER_TOKEN);

/** 送出的一則訊息（欄位先後照 ST：role、content、name）。 */
export interface CountedMessage {
  role: string;
  content: string;
  name?: string;
}

/**
 * 前端 getTokenizerModel 的 OpenRouter 分支：先看模型清單的 `architecture.tokenizer`，再看模型 id，
 * 都對不上就是 gpt-3.5-turbo。
 */
export function tokenizerModel(modelId: string, architectureTokenizer: string | null): string {
  switch (architectureTokenizer) {
    case "Llama2":
      return "llama";
    case "Llama3":
      return "llama3";
    case "Mistral":
      return "mistral";
    case "Yi":
      return "yi";
    case "Gemini":
      return "gemma";
    case "Qwen":
      return "qwen2";
    case "Cohere":
      return modelId.includes("command-a") ? "command-a" : "command-r";
  }
  if (modelId.includes("gpt-4o")) return "gpt-4o";
  if (modelId.includes("gpt-4")) return "gpt-4";
  if (modelId.includes("gpt-3.5-turbo")) return "gpt-3.5-turbo";
  if (modelId.includes("claude")) return "claude";
  if (modelId.includes("GPT-NeoXT")) return "gpt2";
  if (modelId.includes("jamba")) return "jamba";
  if (modelId.includes("deepseek")) return "deepseek";
  return "gpt-3.5-turbo";
}

/** /openai/count 走哪條路：HF 的 web tokenizer、sentencepiece，其餘是 tiktoken。 */
const WEB_TOKENIZERS = ["claude", "llama3", "qwen2", "command-r", "command-a", "deepseek"];
const SENTENCEPIECE = ["llama", "mistral", "yi", "gemma", "jamba"];

/** ST 伺服器 /openai/count 對一則訊息（陣列只有它）的計數。 */
function serverCount(model: string, message: CountedMessage, text: TextTokens): number {
  const values = Object.entries(message).filter(([, value]) => value !== undefined);
  // web tokenizer 與 sentencepiece：所有欄位值以空一行接起來整段算
  if (WEB_TOKENIZERS.includes(model) || SENTENCEPIECE.includes(model)) {
    return text(values.map(([, value]) => value).join("\n\n"));
  }
  // tiktoken：每則 3、每個欄位值各自算、有 name 再加 1，最後補 3
  let tokens = 3;
  for (const [key, value] of values) {
    tokens += text(value as string);
    if (key === "name") tokens += 1;
  }
  return tokens + 3;
}

/**
 * 這支模型的單則訊息計數器，照 ST 的 Message：沒有 name 時內容是空字串不算（createAsync）、有 name 一律算
 * （setName）；前端除了 claude 都扣 2（countTokensOpenAIAsync 的 full=false）。
 */
export function messageTokenCounter(
  modelId: string,
  architectureTokenizer: string | null,
  text: TextTokens = guesstimate,
): (message: CountedMessage) => number {
  const model = tokenizerModel(modelId, architectureTokenizer);
  return (message) => {
    if (message.name === undefined && message.content === "") return 0;
    const counted: CountedMessage =
      message.name === undefined ? { role: message.role, content: message.content } : { role: message.role, content: message.content, name: message.name };
    const tokens = serverCount(model, counted, text);
    return model === "claude" ? tokens : tokens - 2;
  };
}
