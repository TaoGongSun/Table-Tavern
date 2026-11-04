// 電腦上的房間畫面（room-host-desktop-play）：上方一行房主資訊列，下面就是手機同一套房間元件（party/）。
// 連線本身在 app 最外層（useDesktopRoom），這裡只畫；離開這個畫面連線照開著。
// 資訊列只留房號、狀態、邀請連結、關房（room-desktop-layout §2-1）；QR、暱稱、交給手機收在「⋯」彈層，
// 生圖開關在 GM 額度頁，GM 錯誤在回合列，這裡只出非 GM 的錯誤。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { RoomBody } from "@party/features/room/RoomBody";
import "@party/App.css";
import { setLangOnly, t as partyT } from "@party/i18n";
import { gmErrorText } from "@party/features/room/gm-error";
import { InviteControls } from "@party/features/controls/InviteControls";
import { redactAiErrorDetail } from "../../shared/ui/ai-error";
import { backendText } from "../../shared/ui/backend-text";
import { currentLang, t } from "../../i18n";
import { QrCode } from "@party/features/controls/QrCode";
import type { RoomStatus } from "./room-status";
import type { DesktopRoomCtl } from "./useDesktopRoom";

function CopyRow({ label, value }: { label: string; value: string }) {
  const [qr, setQr] = useState(false);
  return (
    <div className="row">
      <span className="room-label">{label}</span>
      <input className="room-value" readOnly value={value} onFocus={(event) => event.currentTarget.select()} />
      <button type="button" className="ghost" onClick={() => void navigator.clipboard?.writeText(value)}>
        {t("roomCopy")}
      </button>
      <button type="button" className="ghost" aria-pressed={qr} onClick={() => setQr(!qr)}>
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

/** 玩家用 AI 生圖花的是這台電腦上的帳號：開關與最近紀錄放在 GM 額度頁（room-player-cards §3.4） */
function ImageGenRow({ status }: { status: RoomStatus }) {
  const [enabled, setEnabled] = useState<boolean | null>(null); // 送出後、輪詢還沒跟上前先顯示新值
  const [error, setError] = useState("");
  useEffect(() => {
    if (enabled === status.imageGen) setEnabled(null);
  }, [enabled, status.imageGen]);
  const on = enabled ?? status.imageGen;
  const log = (status.genLog ?? []).slice(-3).reverse();
  return (
    <div className="room-row" data-testid="room-image-gen">
      <label className="check">
        <input
          type="checkbox"
          checked={on}
          onChange={(event) => {
            const next = event.currentTarget.checked;
            setEnabled(next);
            invoke("room_image_gen", { enabled: next }).then(
              () => setError(""),
              (e) => (setEnabled(null), setError(String(e))),
            );
          }}
        />
        {t("roomImageGen")}
      </label>
      {log.map((x) => (
        <span key={`${x.at}-${x.name}`} className="muted room-nowrap">
          {new Date(x.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })} {t(x.ok ? "roomGenOk" : "roomGenFailed", { name: x.name })}
        </span>
      ))}
      {error && <span role="alert">{error}</span>}
    </div>
  );
}

/** 選角區的桌面版入口要的 app 動作（左欄的玩家卡與匯入，跟 CastRail 同一套） */
export interface DesktopSeat {
  playerName: string | null;
  onOpenPlayerCard: () => void;
  onImport: (file: File) => void;
  importLocked: boolean;
}

/** 選角區：保留給電腦的玩家卡還空著就直接入座它；還沒有玩家卡就開左欄的編輯；匯入走左欄同一條匯入 */
function SeatEntry({ seat, ctl }: { seat: DesktopSeat; ctl: DesktopRoomCtl }) {
  const file = useRef<HTMLInputElement>(null);
  const mine = ctl.view?.cards.find((c) => c.reserved && c.claimedBy === null);
  const name = mine?.name ?? seat.playerName;
  return (
    <>
      <button type="button" className="primary" onClick={() => (mine ? void ctl.room?.act("seat.claim", { cardId: mine.cardId }) : seat.onOpenPlayerCard())}>
        {name ? t("roomUseMyCard", { name }) : t("roomCreateMyCard")}
      </button>
      <button type="button" disabled={seat.importLocked} onClick={() => file.current?.click()}>
        {t("roomImportCard")}
      </button>
      <input
        ref={file}
        type="file"
        accept=".png,.json,image/png,application/json"
        hidden
        onChange={(event) => {
          const picked = event.currentTarget.files?.[0];
          event.currentTarget.value = "";
          if (picked) seat.onImport(picked);
        }}
      />
    </>
  );
}

export function DesktopRoom({ ctl, seat }: { ctl: DesktopRoomCtl; seat: DesktopSeat }) {
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const bar = useRef<HTMLDivElement>(null);
  setLangOnly(currentLang());
  const { status, room, notice, handoff, nickname, busy } = ctl;

  const v = ctl.view;
  const iAmHost = v !== null && v.room.hostId === v.me.memberId;
  const popover = open || handoff !== null;
  // 彈層：點外面或 Esc 收起（交給手機的連結出來時留著，等配對完）
  useEffect(() => {
    if (!open) return;
    const away = (event: PointerEvent) => !bar.current?.contains(event.target as Node) && setOpen(false);
    document.addEventListener("pointerdown", away);
    return () => document.removeEventListener("pointerdown", away);
  }, [open]);
  const copy = () => {
    if (!status?.inviteUrl) return;
    void navigator.clipboard?.writeText(status.inviteUrl).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  // 彈層下半：暱稱、交給手機（交給手機的連結與配對碼出來時接在下面）
  const more = (
    <>
      <hr className="room-sep" />
      {v && (
        <form
          className="row"
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
        <div className="panel-actions">
          <button type="button" disabled={busy || !status?.online} onClick={ctl.toPhone}>
            {t("roomToPhone")}
          </button>
        </div>
      )}
      {handoff && (
        <>
          <p>{t("roomToPhoneLink")}</p>
          <CopyRow label={t("roomToPhone")} value={handoff} />
          <div className="row">
            <span className="room-label">{t("roomPairCode")}</span>
            <strong>{status?.pairCode ?? "—"}</strong>
          </div>
        </>
      )}
    </>
  );
  // GM 錯誤只在回合列（卡住時）；這裡只出連線、伺服器過舊、遠端暫停、同步等非 GM 錯誤（處理完的舊 GM 錯誤不再掛著）
  const otherError = status?.lastError && status.lastErrorKind === null ? status.lastError : null;
  const gmDetail = status?.lastError && status.lastErrorKind !== null ? <ErrorDetail text={status.lastError} /> : undefined;

  return (
    <section className="desktop-room" aria-label={t("roomTitle")}>
      <div className="room-bar" data-testid="room-bar" ref={bar} onKeyDown={(event) => event.key === "Escape" && setOpen(false)}>
        <div className="room-row">
          <span className={status?.online ? "room-dot on" : "room-dot"} title={t(status?.online ? "roomOnline" : "roomOffline")} />
          {status?.roomCode && <strong>{status.roomCode}</strong>}
          {status && !status.online && <span>{t("roomOffline")}</span>}
          {status?.keepAwake === false && <span className="room-warn">{t("roomKeepAwakeOff")}</span>}
          {v && !iAmHost && (
            <span data-testid="room-phone-host">
              {t("roomPhoneHost", { name: v.members.find((m) => m.memberId === v.room.hostId)?.nickname ?? "—" })}{" "}
              <button type="button" className="btn btn-sm" disabled={busy} onClick={ctl.takeHost}>
                {t("roomReclaim")}
              </button>
            </span>
          )}
          {otherError && <OtherError text={otherError} />}
          <span className="toolbar-spacer" />
          <button type="button" className="btn btn-sm" disabled={!status?.inviteUrl} onClick={copy}>
            {copied ? t("roomCopied") : t("roomCopyInvite")}
          </button>
          <button type="button" className="btn btn-sm" aria-label={t("roomMore")} title={t("roomMore")} aria-expanded={popover} onClick={() => setOpen(!open)}>
            ⋯
          </button>
          <button type="button" className="btn btn-sm" disabled={busy} onClick={ctl.pause}>
            {t("roomPause")}
          </button>
        </div>
        {notice && <p role="status">{notice}</p>}
        {popover && (
          <div className="room-popover party-themed" data-testid="room-popover">
            {v && room && iAmHost ? (
              <InviteControls view={v} act={room.act} src={{ origin: ctl.origin, inviteUrl: status?.inviteUrl ?? null }} initialOpen>
                {more}
              </InviteControls>
            ) : (
              <>
                {status?.inviteUrl && <QrCode value={status.inviteUrl} label={t("roomInvite")} />}
                {more}
              </>
            )}
          </div>
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
          <RoomBody
            room={room}
            origin={ctl.origin}
            desktop={{
              leaveLabel: t("roomPause"),
              onLeave: ctl.pause,
              inviteUrl: status?.inviteUrl ?? null,
              payerExtra: status?.active && status.imageGenRoute ? <ImageGenRow status={status} /> : undefined,
              heldDetail: gmDetail,
              seatEntry: <SeatEntry seat={seat} ctl={ctl} />,
            }}
          />
        </div>
      ) : (
        <p className="room-stopped">{t("roomConnecting")}</p>
      )}
    </section>
  );
}

// 金鑰片段（sk-…、sk-or-…）：錯誤原文可能抄到，跟 user_id 一樣不印在畫面上（截圖求救、分享螢幕會外流）
const KEY = /\b(sk-(?:or-)?(?:v\d-)?)[A-Za-z0-9_-]{6,}/g;

const split = (text: string) => {
  const cut = text.search(/[：:]/);
  return { code: (cut > 0 ? text.slice(0, cut) : text).trim(), rest: cut > 0 ? text.slice(cut + 1).trim() : text };
};

/** 錯誤原文：遮掉帳號與金鑰（回合列「詳情」與這裡共用） */
function ErrorDetail({ text }: { text: string }) {
  return <>{redactAiErrorDetail(backendText(split(text).rest)).replace(KEY, "$1[redacted]")}</>;
}

/** 非 GM 錯誤：一句人話（伺服器過舊、遠端暫停有專句），原文收進「詳情」 */
function OtherError({ text }: { text: string }) {
  if (text === "E_SERVER_OUTDATED") return <span className="room-warn">{t("roomServerOutdated")}</span>;
  if (text === "ROOM_PAUSED_REMOTELY") return <span className="room-warn">{t("roomPausedRemotely")}</span>;
  return (
    <span className="room-warn">
      {gmErrorText(null, split(text).code)}{" "}
      <details className="room-error-detail">
        <summary>{partyT("gmErrDetail")}</summary>
        <small>
          <ErrorDetail text={text} />
        </small>
      </details>
    </span>
  );
}
