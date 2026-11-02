// 加入：填暱稱（新裝置第一次進房才要）；QQ、微信內建瀏覽器提示改用瀏覽器開啟（計畫 §9）。
import { useState } from "react";
import { t } from "../../i18n";

// 用 QQ、微信的是中國玩家，引導不跟介面語系、固定簡中〔作者裁決 2026-11-03〕
const OPEN_IN_BROWSER = {
  qq: "在 QQ 内打开可能连不上：请点右上角「⋯」，选「浏览器打开」。",
  wechat: "在微信内打开可能连不上：请点右上角「⋯」，选「在浏览器打开」。",
};

export function OpenInBrowserBanner({ app }: { app: "qq" | "wechat" }) {
  return <div className="banner" lang="zh-CN" data-testid={`in-app-${app}`}>{OPEN_IN_BROWSER[app]}</div>;
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
