// 封存與暫離的角色：寬欄是清單底部的折疊區，窄欄是欄底一顆圖示鈕彈出的面板，
// 兩邊列內容完全相同（名字＋自動隱藏徽章、編輯、復原、⋯ 永久刪除）。
import {
  type FocusEvent,
  type RefObject,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { t } from "../../i18n";
import type { CharacterMeta } from "./card-model";
import { IconArchive, IconDelete, IconEdit } from "../../shared/ui/icons";
import { MoreMenu } from "../../shared/ui/MoreMenu";

export interface ArchiveActions {
  locked: boolean;
  onEdit: (id: string) => void;
  onRestore: (id: string) => void;
  onRestoreAutoHidden: (id: string) => void;
  onDelete: (id: string) => void;
}

/**
 * 列被還原或刪除而消失時，焦點會掉到 body：改移到同位置接上來的下一列（沒有就上一列）的
 * 第一個控制項；整張清單空了就交給 onEmpty。只在焦點確實掉到 body 時才動，
 * 玩家若已經去別處（例如確認框、別的欄位）不搶焦點。回傳的兩個 handler 掛在容器上。
 */
function useRowFocus(
  containerRef: RefObject<HTMLElement | null>,
  ids: string[],
  onEmpty?: () => void,
) {
  const focused = useRef<{ id: string; index: number } | null>(null);
  const key = ids.join("\n");

  useLayoutEffect(() => {
    const was = focused.current;
    if (!was || ids.includes(was.id)) return;
    focused.current = null;
    const active = document.activeElement;
    if (active && active !== document.body && active.isConnected) return;
    if (ids.length === 0) {
      onEmpty?.();
      return;
    }
    const rows = containerRef.current?.querySelectorAll("[data-archive-row]");
    const row = rows?.[Math.min(was.index, ids.length - 1)];
    row?.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
  }, [key]);

  return {
    onFocusCapture: (event: FocusEvent<HTMLElement>) => {
      const row = (event.target as HTMLElement).closest("[data-archive-row]");
      const id = row?.getAttribute("data-archive-row");
      focused.current = id ? { id, index: ids.indexOf(id) } : null;
    },
    // 移除節點時瀏覽器不一定補送 blur；還連在文件上的 blur 才是玩家真的把焦點移走
    onBlurCapture: (event: FocusEvent<HTMLElement>) => {
      if ((event.target as HTMLElement).isConnected) focused.current = null;
    },
  };
}

function ArchiveRows({
  archived,
  locked,
  onEdit,
  onRestore,
  onRestoreAutoHidden,
  onDelete,
}: ArchiveActions & { archived: CharacterMeta[] }) {
  return (
    <div className="archive-list">
      {archived.map((character) => {
        // 沒被玩家手動封存、純粹本幕還沒出場才算自動隱藏；封存優先於自動隱藏顯示
        const isAutoHidden = !character.archived && character.auto_hidden;
        return (
          <div className="archive-row" key={character.id} data-archive-row={character.id}>
            <span className="archive-row-name">
              <span className="archive-row-name-text" title={character.name}>
                {character.name}
              </span>
              {isAutoHidden && <span className="archive-row-badge">{t("autoHiddenBadge")}</span>}
            </span>
            <span className="archive-row-actions">
              {/* 隱藏卡也要進得了編輯器：轉成世界書條目只能在隱藏狀態下按 */}
              <button
                type="button"
                className="btn btn-ghost btn-icon archive-row-icon"
                aria-label={t("editCardSummary", { name: character.name })}
                title={t("editCardSummary", { name: character.name })}
                disabled={locked}
                onClick={() => onEdit(character.id)}
              >
                <IconEdit />
              </button>
              <button
                type="button"
                className="btn btn-sm"
                disabled={locked}
                onClick={() =>
                  isAutoHidden ? onRestoreAutoHidden(character.id) : onRestore(character.id)
                }
              >
                {t("restoreCharacter")}
              </button>
              <MoreMenu
                label={t("itemOptionsAria", { name: character.name })}
                className="btn btn-ghost btn-icon archive-row-icon"
                disabled={locked}
                items={[
                  {
                    key: "delete",
                    label: t("deleteCharacter"),
                    icon: <IconDelete />,
                    danger: true,
                    onSelect: () => onDelete(character.id),
                  },
                ]}
              />
            </span>
          </div>
        );
      })}
    </div>
  );
}

/** 寬欄：清單底部的折疊區 */
export function ArchiveSection({
  archived,
  onEmpty,
  ...actions
}: ArchiveActions & { archived: CharacterMeta[]; onEmpty: () => void }) {
  const ref = useRef<HTMLDetailsElement>(null);
  const focus = useRowFocus(
    ref,
    archived.map((character) => character.id),
    onEmpty,
  );
  if (archived.length === 0) return null;
  return (
    <details ref={ref} className="archive-section" {...focus}>
      <summary>
        {t("archiveSectionTitle")}
        <span className="archive-count">{archived.length}</span>
      </summary>
      <ArchiveRows archived={archived} {...actions} />
    </details>
  );
}

const PANEL_EDGE = 8;

/**
 * 窄欄：欄底圖示鈕（帶張數，0 張不出現）→ 彈出面板。面板 portal 到 body、錨在鈕右側底緣對齊，
 * 夾在視窗內。Esc 與點外面關閉，開啟時移焦到第一個控制項，關閉還焦觸發鈕。
 * 張數歸零或欄切回寬欄時觸發鈕已不存在，焦點交給 onTriggerGone（陣容欄的「＋」鈕）。
 */
export function NarrowArchive({
  archived,
  onTriggerGone,
  ...actions
}: ArchiveActions & { archived: CharacterMeta[]; onTriggerGone: () => void }) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ top: number; left: number } | null>(null);
  const openRef = useRef(false);
  openRef.current = open;
  const count = archived.length;
  const focus = useRowFocus(
    panelRef,
    archived.map((character) => character.id),
  );

  // 張數歸零：面板與觸發鈕一起消失
  useEffect(() => {
    if (count === 0 && openRef.current) {
      setOpen(false);
      onTriggerGone();
    }
  }, [count, onTriggerGone]);

  // 切回寬欄（整個元件卸載）時面板若開著，同樣把焦點交出去
  const goneRef = useRef(onTriggerGone);
  goneRef.current = onTriggerGone;
  useEffect(
    () => () => {
      if (openRef.current) goneRef.current();
    },
    [],
  );

  function reposition() {
    const trigger = triggerRef.current;
    const panel = panelRef.current;
    if (!trigger || !panel) return;
    const rect = trigger.getBoundingClientRect();
    const width = panel.offsetWidth;
    const height = panel.offsetHeight;
    const left = Math.max(PANEL_EDGE, Math.min(rect.right + 6, window.innerWidth - width - PANEL_EDGE));
    const top = Math.max(
      PANEL_EDGE,
      Math.min(rect.bottom - height, window.innerHeight - height - PANEL_EDGE),
    );
    setPlace({ top, left });
  }

  useLayoutEffect(() => {
    if (!open) {
      setPlace(null);
      return;
    }
    reposition();
    // 定位前面板是 visibility:hidden，不能取得焦點；定位後在下一個 effect 移焦
  }, [open, count]);

  useLayoutEffect(() => {
    if (!open || !place) return;
    const panel = panelRef.current;
    if (panel && !panel.contains(document.activeElement)) {
      panel.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
    }
  }, [open, place]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      // ⋯ 選單 portal 在面板外面，點它的項目不算「點外面」
      if (
        panelRef.current?.contains(target) ||
        triggerRef.current?.contains(target) ||
        (target as Element).closest?.('[role="menu"]')
      ) {
        return;
      }
      const hadFocus = panelRef.current?.contains(document.activeElement) ?? false;
      setOpen(false);
      // 焦點原本在面板內：等瀏覽器處理完這次點擊，焦點若掉到 body 才還給觸發鈕；
      // 玩家點的是別的控制項、焦點已經在那裡就不搶
      if (!hadFocus) return;
      setTimeout(() => {
        const active = document.activeElement;
        if (!active || active === document.body || !active.isConnected) triggerRef.current?.focus();
      }, 0);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      // 面板裡的 ⋯ 選單開著時，Esc 先給選單（它自己會 stopPropagation）
      if (event.key !== "Escape") return;
      event.preventDefault();
      setOpen(false);
      triggerRef.current?.focus();
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", reposition);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", reposition);
    };
  }, [open]);

  if (count === 0) return null;
  const label = t("castArchiveAria", { n: count });
  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className="btn btn-ghost rail-archive-btn"
        aria-label={label}
        title={label}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        <IconArchive />
        <span className="archive-count" aria-hidden="true">
          {count}
        </span>
      </button>
      {open &&
        createPortal(
          <div
            ref={panelRef}
            className="archive-popover"
            role="dialog"
            aria-label={label}
            style={place ? { top: place.top, left: place.left } : { visibility: "hidden" }}
            onBlur={(event) => {
              // Tab 走出面板：焦點離開就關（走去它自己的 ⋯ 選單不算離開）
              const next = event.relatedTarget as Element | null;
              // 走去自己的觸發鈕不在這裡關：交給它的 click 切換，免得先關再被開
              if (
                !next ||
                panelRef.current?.contains(next) ||
                triggerRef.current?.contains(next) ||
                next.closest('[role="menu"]')
              ) {
                return;
              }
              setOpen(false);
            }}
            {...focus}
          >
            <ArchiveRows archived={archived} {...actions} />
          </div>,
          document.body,
        )}
    </>
  );
}
