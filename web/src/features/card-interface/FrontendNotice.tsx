// 第一次看到卡片介面時的提示（計畫 2.4）：卡片介面是卡片作者寫的網頁，可以連外，讀得到的對話內容可能被送出去；
// 金鑰不會交給它。看過一次就記在這個瀏覽器，不擋玩。
import { useState } from "react";
import { t } from "../../i18n";

const KEY = "tt-web:card-frontend-notice";

function seen(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

export function FrontendNotice() {
  const [hidden, setHidden] = useState(seen);
  if (hidden) return null;
  return (
    <div className="chat-notice" role="status" data-testid="card-frontend-notice">
      <p>{t("cardFrontendNotice")}</p>
      <button
        type="button"
        onClick={() => {
          try {
            localStorage.setItem(KEY, "1");
          } catch {
            // 瀏覽器不讓存：這次關掉就好
          }
          setHidden(true);
        }}
      >
        {t("cardFrontendNoticeOk")}
      </button>
    </div>
  );
}
