// 手機客戶端外殼：解析邀請連結 → （新裝置）填暱稱 → 連房間。環境（網址、儲存、WebSocket、fetch）從外面注入，測試換成假伺服器。
import { useMemo, useState } from "react";
import { deviceId, isWeChat, joinedNickname, parseInvite, rememberJoined, withoutPairCode, type Invite } from "./features/connection/invite";
import { JoinForm, WeChatBanner } from "./features/join/JoinForm";
import { LanguagePicker } from "./features/language/LanguagePicker";
import { errorText } from "./features/room/error-text";
import { RoomScreen } from "./features/room/RoomScreen";
import { useRoom, type RoomEnv } from "./features/room/useRoom";
import { applyLang, t, type Lang } from "./i18n";

function Room({ env, invite, device, nickname }: { env: RoomEnv; invite: Invite; device: string; nickname: string }) {
  const [left, setLeft] = useState(false);
  const room = useRoom(env, { code: invite.code, deviceId: device, joinSecret: invite.joinSecret, pairCode: invite.pairCode, nickname }, () => {
    rememberJoined(env.storage, invite.code, nickname);
    env.replaceHref(withoutPairCode(env.href));
  });
  const exportRoom = async () => {
    try {
      const res = await env.fetch(`/api/rooms/${invite.code}/export`, { headers: { Authorization: `Bearer ${device}` } });
      if (!res.ok) return false;
      const a = document.createElement("a");
      a.href = URL.createObjectURL(await res.blob());
      a.download = `table-tavern-room-${invite.code}.json`;
      a.click();
      URL.revokeObjectURL(a.href);
      return true;
    } catch {
      return false;
    }
  };
  // 房間關了仍留著畫面唯讀（房主還能匯出，§2）；其餘致命錯誤整頁顯示原因
  const closed = room.fatal === "E_ROOM_CLOSED" && room.view;
  if (left) return <p className="center-note">{t("leftRoom")}</p>;
  if (room.fatal && !closed) return <p className="center-note" data-testid="fatal">{errorText(room.fatal)}</p>;
  if (!room.view) return <p className="center-note">{t("connecting")}</p>;
  const notice = room.notice && errorText(room.notice);
  return (
    <>
      {closed ? <div className="banner">{t("errRoomClosed")}</div> : room.status !== "online" && <div className="banner">{t("reconnecting")}</div>}
      {notice && <button type="button" className="banner" onClick={room.clearNotice}>{notice}</button>}
      <RoomScreen
        view={room.view}
        c={{ ...room, exportRoom, origin: new URL(env.href).origin, onLeave: () => (room.leave(), setLeft(true)) }}
      />
    </>
  );
}

export default function App({ env, lang: initial }: { env: RoomEnv; lang: Lang }) {
  const [lang, setLang] = useState(initial);
  const invite = useMemo(() => parseInvite(env.href), [env.href]);
  const device = useMemo(() => deviceId(env.storage), [env.storage]);
  const [nickname, setNickname] = useState(() => (invite ? joinedNickname(env.storage, invite.code) : null));
  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">Table Tavern{invite && <span className="muted"> · {invite.code}</span>}</span>
        <LanguagePicker lang={lang} onChange={(next) => (applyLang(next), setLang(next))} />
      </header>
      {isWeChat(env.userAgent) && <WeChatBanner />}
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
