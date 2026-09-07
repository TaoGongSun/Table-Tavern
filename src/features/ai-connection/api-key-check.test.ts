import { describe, expect, it } from "vitest";
import { checkApiKey } from "./api-key-check";

describe("checkApiKey", () => {
  it("認出真實事故：把文件的 Quick Start 指令整行貼進來", () => {
    // 2026-08-21 實際發生：OpenRouter 文件那個 Copy 按鈕複製走的是整行示範指令
    expect(checkApiKey("export OPENROUTER_API_KEY=sk-or-v1-...", "")).toBe("apiKeyLooksPasted");
  });

  it("認出各種從文件或程式碼複製到的形狀", () => {
    const pasted = [
      "OPENROUTER_API_KEY=sk-or-v1-abc", // .env 一行
      "$env:OPENROUTER_API_KEY=sk-or-v1-abc", // PowerShell
      "sk-or-v1-…", // 只剩省略號
      "Bearer sk-or-v1-abc", // 從 curl 的標頭複製
      "<your-api-key>", // 角括號佔位符
      "YOUR_API_KEY", // 文字佔位符
      '"sk-or-v1-abc"', // 從程式碼複製，連引號一起
      "`sk-or-v1-abc`", // Markdown 行內碼
      "```sk-or-v1-abc", // code fence
      "${OPENROUTER_API_KEY}", // 模板變數
      "process.env.OPENROUTER_API_KEY",
      "Authorization:sk-or-v1-abc",
      "https://openrouter.ai/api/v1", // 和 base URL 那欄貼反了
      "把金鑰貼在這裡", // 貼到說明文字
      "sk-or-v1-abc\u200b", // 尾端夾帶零寬字元
    ];
    for (const value of pasted) {
      expect(checkApiKey(value, ""), value).toBe("apiKeyLooksPasted");
    }
  });

  it("走 OpenRouter 時認出不是它家的金鑰；手填同一個 host 一樣套規則", () => {
    expect(checkApiKey("sk-proj-abc123", "")).toBe("apiKeyNotOpenRouter");
    expect(checkApiKey("sk-proj-abc123", "https://openrouter.ai/api/v1")).toBe(
      "apiKeyNotOpenRouter",
    );
  });

  it("接自訂端點時不套 OpenRouter 的前綴規則，只留形狀檢查", () => {
    // 中轉站、自架服務的金鑰格式無從假設，短值也可能合法
    expect(checkApiKey("sk-abc123", "https://api.tokenrouter.com/v1")).toBe(null);
    expect(checkApiKey("local", "http://localhost:11434/v1")).toBe(null);
    // 但貼到指令仍然認得出來——那與端點無關
    expect(checkApiKey("export KEY=abc", "http://localhost:11434/v1")).toBe("apiKeyLooksPasted");
  });

  it("正常金鑰與空值不出聲", () => {
    expect(checkApiKey("sk-or-v1-" + "a".repeat(64), "")).toBe(null);
    expect(checkApiKey("", "")).toBe(null); // 還沒填；ollama 這類端點本來就不需要
    expect(checkApiKey("   ", "")).toBe(null);
    // Base64 padding 的 = 不可誤判成變數賦值——padding 一定在結尾，變數賦值的 = 後面還有值
    expect(checkApiKey("sk-or-v1-YWJjZGVm==", "")).toBe(null);
    // 連字號都沒有的純 Base64 更容易踩到（Sol 驗收 2026-08-21 抓到的誤判）
    expect(checkApiKey("YWJjZGVmZ2hpams=", "https://api.example.com/v1")).toBe(null);
    expect(checkApiKey("abc123==", "https://api.example.com/v1")).toBe(null);
    // 冒號在 token 裡不罕見，只有整串以 authorization: 開頭才算貼到標頭
    expect(checkApiKey("sk-or-v1-abc:def", "")).toBe(null);
  });

  it("base URL 填了不成句的東西時，不硬套 OpenRouter 規則", () => {
    expect(checkApiKey("sk-proj-abc", "不是網址")).toBe(null);
  });

  it("host 比對不可只看結尾：notopenrouter.ai 不是 OpenRouter", () => {
    expect(checkApiKey("sk-proj-abc", "https://notopenrouter.ai/v1")).toBe(null);
    // 子網域才算自家
    expect(checkApiKey("sk-proj-abc", "https://api.openrouter.ai/v1")).toBe("apiKeyNotOpenRouter");
  });
});
