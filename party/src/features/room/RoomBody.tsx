// 房間畫面本體（連線狀態、致命錯誤、通知列＋RoomScreen）：手機外殼與桌面版嵌入共用。
import { useState, type ReactNode } from "react";
import { t } from "../../i18n";
import { errorText } from "./error-text";
import { RoomScreen } from "./RoomScreen";
import type { RoomSession } from "./useRoomSession";

/** 桌面版（room-host-desktop-play）把房間畫面嵌進 app：離開鈕換成關房、不給匯出與配對碼欄、看得到保留給電腦的卡 */
export interface DesktopHooks {
  leaveLabel: string;
  onLeave: () => void;
  /** 資訊列那條邀請連結：房主頁「邀請新朋友」直接顯示它 */
  inviteUrl: string | null;
  /** GM 額度頁多一段（生圖開關：花的是這台電腦的額度，只有桌面做得到） */
  payerExtra?: ReactNode;
  /** 卡住時回合列的「詳情」：桌面用本機遮罩過的原文；沒給就用房間的分類說明 */
  heldDetail?: ReactNode;
  /** 選角區的桌面版入口：建立／用我的角色卡（左欄玩家卡）、匯入角色卡 */
  seatEntry?: ReactNode;
}

export function RoomBody({
  room,
  origin,
  desktop,
  settings,
}: {
  room: RoomSession;
  origin: string;
  desktop?: DesktopHooks;
  /** 手機：名單頁底部的顯示設定（房號、語系、字級） */
  settings?: ReactNode;
}) {
  const [left, setLeft] = useState(false);
  // 房間關了仍留著畫面唯讀（房主還能匯出，§2）；其餘致命錯誤整頁顯示原因
  const closed = room.fatal === "E_ROOM_CLOSED" && room.view;
  if (left) return <p className="center-note">{t("leftRoom")}</p>;
  if (room.fatal && !closed)
    return (
      <p className="center-note" data-testid="fatal">
        {errorText(room.fatal)}
      </p>
    );
  if (!room.view) return <p className="center-note">{t("connecting")}</p>;
  const notice = room.notice && errorText(room.notice);
  return (
    <>
      {closed ? (
        <div className="banner">{t("errRoomClosed")}</div>
      ) : (
        room.status !== "online" && <div className="banner">{t("reconnecting")}</div>
      )}
      {notice && (
        <button type="button" className="banner" onClick={room.clearNotice}>
          {notice}
        </button>
      )}
      <RoomScreen
        view={room.view}
        c={{
          ...room,
          exportRoom: desktop ? null : room.exportRoom,
          upload: desktop ? null : room.upload,
          origin,
          onLeave: desktop ? desktop.onLeave : () => (room.leave(), setLeft(true)),
          leaveLabel: desktop?.leaveLabel,
          pairForm: !desktop,
          inviteUrl: desktop?.inviteUrl,
          joinSecret: desktop ? null : room.joinSecret,
          payerExtra: desktop?.payerExtra,
          heldDetail: desktop?.heldDetail,
          seatEntry: desktop?.seatEntry,
          settings,
        }}
      />
    </>
  );
}
