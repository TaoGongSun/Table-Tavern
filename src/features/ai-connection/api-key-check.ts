// 金鑰貼錯的形狀比對（純字串，不打網路）：使用者按了 OpenRouter 文件 Quick Start 的 Copy，
// 貼進來的是 `export OPENROUTER_API_KEY=sk-or-v1-...` 這種示範指令，app 照送出去只換得
// 一句 401「Missing Authentication header」，玩家看不出錯在哪（2026-08-21 真實事故）。
//
// 一律只提示、不擋存檔：app 可接任意 OpenAI-compatible 端點（中轉站、ollama、LM Studio），
// 自架端點的金鑰格式無從假設；且 OpenRouter 官方文件並未明文保證 `sk-or-` 這個前綴不變。

// 一眼看得出是「從文件或程式碼複製到的東西」，而不是金鑰本身。與端點無關。
// 刻意不收「含 =」：Base64 padding 與部分 token 本來就有 =，只抓像變數賦值的形狀。
// 第一條已經吃掉所有含空白的形狀（整行指令、curl 片段、說明文字），
// 其餘各條處理的是「沒有空白但仍然不是金鑰」的情況。
const PASTED_SHAPES: RegExp[] = [
  /\s/, // 內部空白／換行（存檔前已 trim，剩下的都在中間）
  /[\u0000-\u001f\u200b-\u200f\ufeff]/, // 控制字元、零寬字元、BOM
  /^[A-Za-z_$][\w$:.]*=[^=]/, // NAME=value、$env:NAME=：像變數賦值。`=` 後要有東西，
  // 否則純 Base64（`YWJjZGU=`、`abc123==`）會被誤殺——padding 的 = 一定落在結尾
  /\.{3}$|…/, // 省略號＝文件裡那句「你的金鑰接這裡」。三個點限結尾：`.` 是 token 合法字元
  /[<>]/, // <your-key> 這類角括號佔位符
  /[\u3000-\u9fff\uac00-\ud7af]/, // CJK：貼到的是說明文字
  /^https?:\/\//i, // 整串是網址＝和「自訂 base URL」那欄貼反了
  /^(["'`]).*\1$/, // 整串被成對引號包住＝從程式碼複製
  /^```/, // Markdown code fence
  /\$\{|process\.env|os\.getenv|getenv\(/i, // 程式碼裡的變數引用
  /your[_-]?(api[_-]?)?key|replace[_-]?me|xxxxx/i, // 文件常見的佔位字樣
  /^authorization:/i, // 從 HTTP 標頭那行複製；限開頭，token 中間帶冒號並不罕見
];

/** 這個 base URL 實際會打到 OpenRouter 嗎。留空＝走預設端點；手填同一個 host 也算，
 *  所以填了 `https://openrouter.ai/api/v1` 一樣套得到前綴規則。 */
function isOpenRouter(baseUrl: string): boolean {
  const url = baseUrl.trim();
  if (url === "") return true;
  try {
    const host = new URL(url).hostname;
    // 不可只用 endsWith：`notopenrouter.ai` 會被誤認成自家
    return host === "openrouter.ai" || host.endsWith(".openrouter.ai");
  } catch {
    return false; // 填了但不是合法網址：無從判定，不硬套 OpenRouter 的規則
  }
}

/**
 * 回 i18n 鍵或 null。null＝看不出問題（含「還沒填」——ollama 這類端點本來就不需要金鑰）。
 * 兩種提示的下一步不同：一個是「你複製到別的東西了」，一個是「金鑰本身像是別家的」。
 */
export function checkApiKey(
  key: string,
  baseUrl: string,
): "apiKeyLooksPasted" | "apiKeyNotOpenRouter" | null {
  const value = key.trim();
  if (value === "") return null;
  if (PASTED_SHAPES.some((shape) => shape.test(value))) return "apiKeyLooksPasted";
  if (isOpenRouter(baseUrl) && !value.startsWith("sk-or-")) return "apiKeyNotOpenRouter";
  return null;
}
