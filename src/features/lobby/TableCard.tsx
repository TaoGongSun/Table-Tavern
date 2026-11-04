// 大廳的桌卡：封面整塊一顆進桌按鈕，下方一列桌名、徽章與 ⋯。封面不疊任何東西。
// 就地改名只落在桌名那一格；送出／取消用 ref 保證一次只結算一次（Esc 先標記取消再卸載輸入框，
// 卸載引發的 blur 就不會又把它送出去）。
import { useEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import { listBadge } from "../world-format/open-world";
import type { WorldMeta } from "../../shared/contracts/backend-contracts";
import { IconDelete, IconEdit, IconUsers } from "../../shared/ui/icons";
import { MoreMenu } from "../../shared/ui/MoreMenu";
import gmBook from "../../assets/gm-book.png";
import { coverLayout } from "./table-cover";
import { useTableCover } from "./useTableCover";

interface TableCardProps {
  world: WorldMeta;
  /** 進桌／出桌進行中或生成中：封面、⋯ 一律停用 */
  busy: boolean;
  onEnter: (id: string) => void;
  onRename: (id: string, raw: string) => void;
  onDelete: (id: string) => void;
  /** 可用現有桌開房的桌（單人聊天桌；玩過的也行，種子最前面標「房主之前自己玩的」） */
  roomFromTable?: { onOpen: () => void };
  /** 另一間房開著（一次只開一間） */
  otherRoomActive?: boolean;
  /** 開過房的桌（room-host-desktop-play）：徽章與「重開房間／回房間」「結束並刪除房間」 */
  room?: { code: string; active: boolean; otherActive: boolean; onReopen: () => void; onEnd: () => void };
}

export function TableCard({ world, busy, onEnter, onRename, onDelete, room, roomFromTable, otherRoomActive }: TableCardProps) {
  const { ref, images } = useTableCover(world.id);
  const [draft, setDraft] = useState<string | null>(null);
  const settled = useRef(false);
  const footRef = useRef<HTMLDivElement>(null);
  // Enter／Esc 結束改名時輸入框卸載、焦點會掉到 body：還給這張卡的 ⋯ 鈕。blur 結束代表玩家
  // 已經去別處，不搶焦點
  const refocus = useRef(false);
  useEffect(() => {
    if (draft !== null || !refocus.current) return;
    refocus.current = false;
    footRef.current?.querySelector<HTMLElement>('[aria-haspopup="menu"]')?.focus();
  }, [draft]);
  const badge = listBadge(world);
  const badgeLabel =
    badge === null ? "" : t(badge === "repair" ? "needsRepairBadge" : "readOnlyBadge");
  const layout = images === null ? null : coverLayout(images.length);

  function startRename() {
    settled.current = false;
    setDraft(world.name);
  }

  function settle(commit: boolean, restoreFocus: boolean) {
    if (settled.current) return;
    settled.current = true;
    refocus.current = restoreFocus;
    const value = draft ?? "";
    setDraft(null);
    if (commit) onRename(world.id, value);
  }

  return (
    <div className="table-card">
      <button
        ref={ref}
        type="button"
        className="table-cover"
        aria-label={t("lobbyEnterTable", { name: world.name })}
        title={world.name}
        disabled={busy}
        onClick={() => onEnter(world.id)}
      >
        {layout !== null && (
          <span className="table-cover-art" data-layout={layout}>
            {layout === "book" ? (
              <img className="table-cover-book" src={gmBook} alt="" />
            ) : (
              images?.map((src, index) => <img key={index} src={src} alt="" />)
            )}
          </span>
        )}
      </button>
      <div className="table-card-foot" ref={footRef}>
        {draft !== null ? (
          <form
            className="table-card-rename"
            onSubmit={(event) => {
              event.preventDefault();
              settle(true, true);
            }}
          >
            <input
              autoFocus
              value={draft}
              aria-label={t("tableNameAria")}
              onChange={(event) => setDraft(event.currentTarget.value)}
              onBlur={() => settle(true, false)}
              onKeyDown={(event) => {
                if (event.key === "Escape") settle(false, true);
              }}
            />
          </form>
        ) : (
          <span className="table-card-name" title={world.name}>
            {world.name}
          </span>
        )}
        {world.table_kind === "vn" && (
          <span className="table-badge">{t(world.replay ? "vnWorkBadge" : "vnBadge")}</span>
        )}
        {badge !== null && (
          <span className="table-badge" title={badgeLabel}>
            {badgeLabel}
          </span>
        )}
        {room?.code && (
          <span className="table-room-icon" role="img" title={t("roomBadge", { code: room.code })} aria-label={t("roomBadge", { code: room.code })}>
            <IconUsers />
          </span>
        )}
        <MoreMenu
          label={t("itemOptionsAria", { name: world.name })}
          disabled={busy}
          items={[
            ...(room
              ? [
                  {
                    key: "room",
                    label: t(room.active ? "roomBack" : "roomReopen"),
                    icon: <IconUsers />,
                    disabled: badge !== null || room.otherActive,
                    note: room.otherActive ? t("roomOtherTable") : undefined,
                    onSelect: room.active ? () => onEnter(world.id) : room.onReopen,
                  },
                ]
              : []),
            // 另一間房開著時停用並直接寫理由
            ...(roomFromTable
              ? [
                  {
                    key: "room-open",
                    label: t("roomOpenFromTable"),
                    icon: <IconUsers />,
                    disabled: otherRoomActive,
                    note: otherRoomActive ? t("roomOtherTable") : undefined,
                    onSelect: roomFromTable.onOpen,
                  },
                ]
              : []),
            {
              key: "rename",
              label: t("lobbyRename"),
              icon: <IconEdit />,
              // 唯讀與需修復的桌不能寫入，改名停用
              disabled: badge !== null,
              onSelect: startRename,
            },
            ...(room
              ? [
                  {
                    key: "room-end",
                    label: t("roomEnd"),
                    icon: <IconDelete />,
                    danger: true,
                    onSelect: room.onEnd,
                  },
                ]
              : []),
            {
              key: "delete",
              label: t("deleteTableTitle"),
              icon: <IconDelete />,
              danger: true,
              onSelect: () => onDelete(world.id),
            },
          ]}
        />
      </div>
    </div>
  );
}
