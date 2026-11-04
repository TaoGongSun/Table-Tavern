// 房主頁的邀請（room-seat-delegate §2.2、§3.6）：「邀請新朋友」顯示現行連結＋QR、不重生（還沒到的老朋友手上的連結照樣能用）；
// 「換新連結」另一顆，確認舊連結作廢才重生。桌面版直接用資訊列那條；手機帶自己網址上的密語問伺服器，對不上再問電腦。
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { Confirm } from "../room/Confirm";
import { errorText } from "../room/error-text";
import type { RoomView } from "../room/room-state";
import type { Act } from "../room/useRoom";
import { QrCode } from "./QrCode";

export interface InviteSource {
  origin: string;
  /** 桌面版：資訊列那條（電腦本來就握有現行連結） */
  inviteUrl?: string | null;
  /** 手機：自己網址上的 #k=（可能已過期，伺服器會驗） */
  joinSecret?: string | null;
}

/** `initialOpen`：桌面資訊列的彈層一打開就畫現行連結與 QR（朋友多半站在旁邊等掃），並跟著最新的連結走 */
export function InviteControls({ view, act, src, initialOpen = false }: { view: RoomView; act: Act; src: InviteSource; initialOpen?: boolean }) {
  const [link, setLink] = useState<string | null>(initialOpen ? (src.inviteUrl ?? null) : null);
  // 桌面版握有現行連結：已經顯示著（或一開始就展開）時，換新連結後跟上
  useEffect(() => {
    if (src.inviteUrl) setLink((cur) => (cur !== null || initialOpen ? src.inviteUrl! : cur));
  }, [src.inviteUrl, initialOpen]);
  const [note, setNote] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  const url = (secret: string) => `${src.origin}/r/${view.room.code}#k=${secret}`;
  const show = async () => {
    if (src.inviteUrl) return void (setLink(src.inviteUrl), setNote(null));
    setNote(t("hostInviteLoading"));
    const reply = await act("invite.current", src.joinSecret ? { candidate: src.joinSecret } : {});
    if (reply.ok) return void (setLink(url(reply.result.joinSecret)), setNote(null));
    setLink(null);
    setNote(reply.code === "E_EXECUTOR_OFFLINE" ? t("hostInviteOffline") : errorText(reply.code, reply.retryAfterMs));
  };
  const regenerate = async () => {
    setAsking(false);
    const reply = await act("invite.regenerate");
    if (reply.ok) return void (setLink(url(reply.result.joinSecret)), setNote(null));
    setNote(errorText(reply.code, reply.retryAfterMs));
  };
  return (
    <>
      <div className="panel-actions">
        {!initialOpen && <button type="button" data-testid="invite-current" onClick={() => void show()}>{t("hostInviteCurrent")}</button>}
        {/* 新連結要先存進電腦才生效：電腦離線時不給換 */}
        <button type="button" className="ghost" disabled={!view.executor.online} title={view.executor.online ? undefined : t("hostInviteNeedsExecutor")} onClick={() => setAsking(true)}>{t("hostInviteNew")}</button>
      </div>
      {!view.executor.online && <p className="muted">{t("hostInviteNeedsExecutor")}</p>}
      {note && <p className="muted" data-testid="invite-note">{note}</p>}
      {link && (
        <div className="invite-box">
          <div className="row">
            <input className="invite" readOnly value={link} onFocus={(e) => e.currentTarget.select()} data-testid="invite" />
            <button type="button" className="ghost" onClick={() => void navigator.clipboard?.writeText(link)}>{t("hostCopy")}</button>
          </div>
          <QrCode value={link} label={t("hostInviteCurrent")} />
        </div>
      )}
      {asking && (
        <Confirm title={t("hostInviteNew")} testId="invite-new-confirm" onCancel={() => setAsking(false)} actions={[{ label: t("hostInviteNew"), onClick: () => void regenerate(), danger: true }]}>
          <p>{t("hostInviteNewConfirm")}</p>
        </Confirm>
      )}
    </>
  );
}
