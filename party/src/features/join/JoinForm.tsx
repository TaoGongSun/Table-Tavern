// 加入：填暱稱（新裝置第一次進房才要）；微信內建瀏覽器提示改用瀏覽器開啟（計畫 §9）。
import { useState } from "react";
import { t } from "../../i18n";

export function WeChatBanner() {
  return <div className="banner" data-testid="wechat">{t("wechatOpenInBrowser")}</div>;
}

export function JoinForm({ code, canJoin, onJoin }: { code: string; canJoin: boolean; onJoin: (nickname: string) => void }) {
  const [nickname, setNickname] = useState("");
  if (!canJoin) return <p className="center-note">{t("joinNeedInvite")}</p>;
  return (
    <form
      className="join-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (nickname.trim()) onJoin(nickname.trim());
      }}
    >
      <h1>{t("joinTitle", { code })}</h1>
      <label>
        {t("joinNickname")}
        <input value={nickname} maxLength={24} autoFocus data-testid="nickname" onChange={(e) => setNickname(e.target.value)} />
      </label>
      <button type="submit" className="primary" disabled={!nickname.trim()}>{t("joinButton")}</button>
    </form>
  );
}
