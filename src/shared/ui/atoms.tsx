import { useMemo } from "react";
import { t } from "../../i18n";
import { renderStoryMarkdown } from "./story-markdown";
import { explainAiError, redactAiErrorDetail } from "./ai-error";
import { backendCode, backendText } from "./backend-text";

// AI 失敗的呈現：命中分流就顯示人話，原始字串一律保留在小字（玩家與協助者仍看得到真相）。
// transport 給得出來就傳：認證失敗要指對地方（API 換金鑰／CLI 重新登入）。
// text 存後端原文：分流吃原文，顯示時才經 backendText 翻譯代碼、再遮掉帳號識別碼。錯誤列與失敗彈窗共用。
export function AiErrorText({ text, transport }: { text: string; transport?: string }) {
  const key = explainAiError(text, transport);
  const shown = redactAiErrorDetail(backendText(text));
  if (!key) return <>{shown}</>;
  return (
    <>
      {t(key)}
      <br />
      <small>{shown}</small>
    </>
  );
}

/** 這則錯誤的下一步就是換幕：模型一次讀不完這一幕，或換幕容量鎖擋下了這次送出 */
export function offersSceneAdvance(text: string, transport?: string): boolean {
  return (
    explainAiError(text, transport) === "errContextTooLong" ||
    backendCode(text) === "scene_capacity_full"
  );
}

/** onAdvanceScene 只有聊天錯誤列傳：這一幕太長時，下一步就是換幕，鈕直接放在錯誤旁 */
export function ErrorNote({
  text,
  transport,
  onAdvanceScene,
}: {
  text: string;
  transport?: string;
  onAdvanceScene?: () => void;
}) {
  const offerAdvance = onAdvanceScene && offersSceneAdvance(text, transport);
  return (
    <p role="alert">
      <AiErrorText text={text} transport={transport} />
      {offerAdvance && (
        <>
          <br />
          <button type="button" className="btn btn-sm" onClick={onAdvanceScene}>
            {t("sceneAdvance")}
          </button>
        </>
      )}
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
