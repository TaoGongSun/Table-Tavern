// 手機客戶端外殼：解析邀請連結 → （新裝置）填暱稱 → 連房間。環境（網址、儲存、WebSocket、fetch）從外面注入，測試換成假伺服器。
import { useMemo, useState } from "react";
import { deviceId, inAppBrowser, joinedNickname, parseInvite, type Invite } from "./features/connection/invite";
import { JoinForm, OpenInBrowserBanner } from "./features/join/JoinForm";
import { LanguagePicker } from "./features/language/LanguagePicker";
import { RoomBody } from "./features/room/RoomBody";
import type { RoomEnv } from "./features/room/useRoom";
import { useRoomSession } from "./features/room/useRoomSession";
import { applyLang, t, type Lang } from "./i18n";

function Room({ env, invite, device, nickname }: { env: RoomEnv; invite: Invite; device: string; nickname: string }) {
  const room = useRoomSession(env, invite, device, nickname);
  return <RoomBody room={room} origin={new URL(env.href).origin} />;
}

export default function App({ env, lang: initial }: { env: RoomEnv; lang: Lang }) {
  const [lang, setLang] = useState(initial);
  const invite = useMemo(() => parseInvite(env.href), [env.href]);
  const inApp = inAppBrowser(env.userAgent);
  const device = useMemo(() => deviceId(env.storage), [env.storage]);
  const [nickname, setNickname] = useState(() => (invite ? joinedNickname(env.storage, invite.code) : null));
  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">Table Tavern{invite && <span className="muted"> · {invite.code}</span>}</span>
        <LanguagePicker lang={lang} onChange={(next) => (applyLang(next), setLang(next))} />
      </header>
      {inApp && <OpenInBrowserBanner app={inApp} />}
      {!invite ? (
        <p className="center-note">{t("joinNeedInvite")}</p>
      ) : nickname ? (
        <Room env={env} invite={invite} device={device} nickname={nickname} />
      ) : (
        <JoinForm code={invite.code} canJoin={!!invite.joinSecret} onJoin={setNickname} />
      )}
    </div>
  );
}
