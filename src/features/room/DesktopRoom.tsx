// 電腦上的房間畫面（room-host-desktop-play）：上方房主資訊列，下面就是手機同一套房間元件（party/）。
// 連線本身在 app 最外層（useDesktopRoom），這裡只畫；離開這個畫面連線照開著。
import { useState } from "react";
import { RoomBody } from "@party/App";
import "@party/App.css";
import { setLangOnly } from "@party/i18n";
import { currentLang, t } from "../../i18n";
import { QrCode } from "./QrCode";
import type { DesktopRoomCtl } from "./useDesktopRoom";

function CopyRow({ label, value }: { label: string; value: string }) {
  const [qr, setQr] = useState(false);
  return (
    <div className="room-row">
      <span className="room-label">{label}</span>
      <input className="room-value" readOnly value={value} onFocus={(event) => event.currentTarget.select()} />
      <button type="button" className="btn btn-sm" onClick={() => void navigator.clipboard?.writeText(value)}>
        {t("roomCopy")}
      </button>
      <button type="button" className="btn btn-sm" aria-pressed={qr} onClick={() => setQr(!qr)}>
        {t("roomShowQr")}
      </button>
      {qr && (
        <div className="room-qr-box">
          <QrCode value={value} label={label} />
        </div>
      )}
    </div>
  );
}

export function DesktopRoom({ ctl }: { ctl: DesktopRoomCtl }) {
  const [open, setOpen] = useState(false);
  setLangOnly(currentLang());
  const { status, room, notice, handoff, nickname, busy } = ctl;

  const v = ctl.view;
  const iAmHost = v !== null && v.room.hostId === v.me.memberId;

  return (
    <section className="desktop-room" aria-label={t("roomTitle")}>
      <div className="room-bar" data-testid="room-bar">
        {/* 一行摘要常駐；邀請連結、暱稱、交給手機收在「詳細」裡，故事區才有空間 */}
        <div className="room-row">
          {status?.roomCode && <strong>{t("roomBadge", { code: status.roomCode })}</strong>}
          <span>{status?.online ? t("roomOnline") : t("roomOffline")}</span>
          {status && status.keepAwake !== null && (
            <span>{status.keepAwake ? t("roomKeepAwakeOn") : t("roomKeepAwakeOff")}</span>
          )}
          {status?.modelLabel && (
            <span>
              {t("roomModel")}：{status.modelLabel === "default" ? t("roomModelDefault") : status.modelLabel}
            </span>
          )}
          <span className="toolbar-spacer" />
          <button type="button" className="btn btn-sm" aria-expanded={open} onClick={() => setOpen(!open)}>
            {t("roomInvite")}
          </button>
          <button type="button" className="btn btn-sm" disabled={busy} onClick={ctl.pause}>
            {t("roomPause")}
          </button>
        </div>
        {notice && <p role="status">{notice}</p>}
        {v && !iAmHost && (
          <div className="room-row" data-testid="room-phone-host">
            <span>{t("roomPhoneHost", { name: v.members.find((m) => m.memberId === v.room.hostId)?.nickname ?? "—" })}</span>
            <button type="button" className="btn btn-sm" disabled={busy} onClick={ctl.takeHost}>
              {t("roomReclaim")}
            </button>
          </div>
        )}
        {status?.lastError && (
          <div className="room-row">
            <span className="room-label">{t("roomLastError")}</span>
            <span>{status.lastError === "E_SERVER_OUTDATED" ? t("roomServerOutdated") : status.lastError}</span>
          </div>
        )}
        {(open || handoff) && (
          <>
            {status?.inviteUrl && <CopyRow label={t("roomInvite")} value={status.inviteUrl} />}
            {v && (
              <form
                className="room-row"
                onSubmit={(event) => {
                  event.preventDefault();
                  const name = nickname.trim();
                  if (name && name !== v.me.nickname) void room?.act("profile.rename", { nickname: name });
                }}
              >
                <label className="room-label" htmlFor="room-nickname">
                  {t("roomNickname")}
                </label>
                <input id="room-nickname" className="room-value" maxLength={24} value={nickname} onChange={(e) => ctl.setNickname(e.target.value)} />
              </form>
            )}
            {v && iAmHost && !handoff && (
              <div className="room-row">
                <button type="button" className="btn btn-sm" disabled={busy || !status?.online} onClick={ctl.toPhone}>
                  {t("roomToPhone")}
                </button>
              </div>
            )}
            {handoff && (
              <>
                <p>{t("roomToPhoneLink")}</p>
                <CopyRow label={t("roomToPhone")} value={handoff} />
                <div className="room-row">
                  <span className="room-label">{t("roomPairCode")}</span>
                  <strong>{status?.pairCode ?? "—"}</strong>
                </div>
              </>
            )}
          </>
        )}
      </div>
      {ctl.stopped ? (
        <p className="room-stopped">
          {t("roomStopped")}{" "}
          <button type="button" className="btn btn-sm" onClick={ctl.reconnect}>
            {t("roomReconnect")}
          </button>
        </p>
      ) : room ? (
        <div className="app party-embedded">
          <RoomBody room={room} origin={ctl.origin} desktop={{ leaveLabel: t("roomPause"), onLeave: ctl.pause }} />
        </div>
      ) : (
        <p className="room-stopped">{t("roomConnecting")}</p>
      )}
    </section>
  );
}
