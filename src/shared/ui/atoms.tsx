import { useMemo } from "react";
import { t } from "../../i18n";
import { renderStoryMarkdown } from "./story-markdown";
import { explainAiError } from "./ai-error";
import { backendText } from "./backend-text";

// 錯誤列：命中分流就顯示人話，原始字串一律保留在小字（玩家與協助者仍看得到真相）。
// transport 給得出來就傳：認證失敗要指對地方（API 換金鑰／CLI 重新登入）。
// text 存後端原文：分流吃原文，顯示時才經 backendText 翻譯代碼。
export function ErrorNote({ text, transport }: { text: string; transport?: string }) {
  const key = explainAiError(text, transport);
  if (!key) return <p role="alert">{backendText(text)}</p>;
  return (
    <p role="alert">
      {t(key)}
      <br />
      <small>{backendText(text)}</small>
    </p>
  );
}


export function StoryText({ text }: { text: string }) {
  const html = useMemo(() => renderStoryMarkdown(text), [text]);
  return (
    <span
      className="text rendered"
      // 卡片內嵌的圖是熱連作者自己的圖床，失效是常態（擋熱連、圖被刪、玩家離線）：
      // 載不到就整張藏掉，故事中間留一個破圖示比沒有更糟。img 的 error 不冒泡，只能用 capture 收
      onErrorCapture={(event) => {
        const target = event.target as HTMLElement;
        if (target.tagName === "IMG") target.classList.add("img-failed");
      }}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
