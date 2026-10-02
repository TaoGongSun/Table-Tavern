// ⋯ 選單：工具列與日後編輯頁共用的溢位選單。自寫而不引套件，行為照 WAI-ARIA menu button：
// 開啟移焦第一項、↑↓／Home／End 循環、Enter/Space 執行後關閉、Esc 與點外面關閉、Tab 直接離開。
// 彈層 portal 到 body：主欄 .chat-main 是 overflow:hidden，掛在原地會被裁掉。
// 預設觸發鈕是 ⋯ 幽靈圖示鈕；新增鈕、幕晶片這類要自己的外觀就傳 trigger／className。
import {
  Fragment,
  type KeyboardEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { IconMore } from "./icons";

export interface MoreMenuItem {
  key: string;
  label: string;
  /** 滑鼠停留的補充說明 */
  hint?: string;
  icon?: ReactNode;
  onSelect: () => void;
  disabled?: boolean;
  /** 危險項（永久刪除類）：紅字，和上面的一般項之間隔一條分隔線 */
  danger?: boolean;
}

interface MoreMenuProps {
  /** 選單的名字，也是預設觸發鈕的 aria-label／title */
  label: string;
  items: MoreMenuItem[];
  /** 打開那一刻通知外層（例如收掉同區的其他浮層） */
  onOpen?: () => void;
  /** 自訂觸發鈕內容；省略＝⋯ 圖示 */
  trigger?: ReactNode;
  /** 自訂觸發鈕 class；省略＝幽靈圖示鈕 */
  className?: string;
  /** 觸發鈕本身有可見文字時傳 false：名字交給文字，只留 title */
  ariaLabelTrigger?: boolean;
  /** 停用：觸發鈕停用，已開的選單一併收掉 */
  disabled?: boolean;
}

// 彈層與視窗邊緣保留的距離
const EDGE = 8;

export function MoreMenu({
  label,
  items,
  onOpen,
  trigger,
  className = "btn btn-ghost btn-icon",
  ariaLabelTrigger = true,
  disabled = false,
}: MoreMenuProps) {
  const [open, setOpen] = useState(false);
  const [place, setPlace] = useState<{ top: number; left: number; maxHeight: number } | null>(
    null,
  );
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const focusPending = useRef(false);
  const menuId = useId();

  // 右緣對齊觸發鈕、水平夾在視窗內（右緣對齊會出界就改成左緣對齊，不蓋住觸發鈕）。
  // 下方放得下就向下開；放不下而上方比較大就向上開。高度給到該側剩餘空間為止，放不下就內捲
  const reposition = useCallback(() => {
    const trigger = triggerRef.current;
    const menu = menuRef.current;
    if (!trigger || !menu) return;
    const rect = trigger.getBoundingClientRect();
    const width = menu.offsetWidth;
    // scrollHeight 是內容全高（含已被 maxHeight 截掉的部分），再補上上下框線
    const natural = menu.scrollHeight + menu.offsetHeight - menu.clientHeight;
    const below = Math.max(0, window.innerHeight - rect.bottom - 4 - EDGE);
    const above = Math.max(0, rect.top - 4 - EDGE);
    const openUp = natural > below && above > below;
    const maxHeight = openUp ? above : below;
    const top = openUp ? rect.top - 4 - Math.min(natural, maxHeight) : rect.bottom + 4;
    const preferred = rect.right - width < EDGE ? rect.left : rect.right - width;
    const left = Math.max(EDGE, Math.min(preferred, window.innerWidth - width - EDGE));
    setPlace({ top, left, maxHeight });
  }, []);

  useLayoutEffect(() => {
    if (!open) {
      setPlace(null);
      return;
    }
    focusPending.current = true;
    reposition();
  }, [open, reposition]);

  // 定位完才移焦：定位前彈層是 visibility:hidden，瀏覽器不讓隱形元素取得焦點
  useLayoutEffect(() => {
    if (!place || !focusPending.current) return;
    focusPending.current = false;
    itemRefs.current[0]?.focus();
  }, [place]);

  // 觸發鈕轉成停用：選單不能留在畫面上讓人執行
  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  useEffect(() => {
    if (!open) return;
    // 點外面關閉：焦點留給被點的那個控制項，不搶回 ⋯ 鈕；點 ⋯ 鈕本身交給它自己的 onClick 切換
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (menuRef.current?.contains(target) || triggerRef.current?.contains(target)) return;
      setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [open, reposition]);

  function toggle() {
    if (!open) onOpen?.();
    setOpen(!open);
  }

  function closeToTrigger() {
    setOpen(false);
    triggerRef.current?.focus();
  }

  // 先把焦點還給 ⋯ 鈕再執行：項目若開了對話窗，對話窗掛載時自己的移焦會蓋過這一步，不被搶回來
  function run(item: MoreMenuItem) {
    if (item.disabled) return;
    closeToTrigger();
    item.onSelect();
  }

  function onMenuKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const nodes = itemRefs.current.filter((node): node is HTMLButtonElement => node !== null);
    const current = nodes.indexOf(document.activeElement as HTMLButtonElement);
    const focusAt = (index: number) => nodes[(index + nodes.length) % nodes.length]?.focus();
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        focusAt(current + 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        focusAt(current < 0 ? -1 : current - 1);
        break;
      case "Home":
        event.preventDefault();
        focusAt(0);
        break;
      case "End":
        event.preventDefault();
        focusAt(-1);
        break;
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        closeToTrigger();
        break;
      case "Tab":
        // 不攔預設：焦點先同步交回 ⋯ 鈕，瀏覽器的 Tab／Shift+Tab 就從 ⋯ 鈕往前後走；
        // 選單等下一個 tick 才卸載，免得焦點所在的節點在預設移焦前就消失
        triggerRef.current?.focus();
        setTimeout(() => setOpen(false), 0);
        break;
    }
  }

  // 停用項維持可聚焦（aria-disabled），鍵盤使用者才讀得到它存在；執行防線在 run()
  const firstDanger = items.findIndex((item) => item.danger);
  itemRefs.current.length = items.length;

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className={className}
        aria-label={ariaLabelTrigger ? label : undefined}
        title={label}
        disabled={disabled}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={toggle}
      >
        {trigger ?? <IconMore />}
      </button>
      {open &&
        createPortal(
          <div
            ref={menuRef}
            id={menuId}
            className="menu"
            role="menu"
            aria-label={label}
            style={
              place
                ? { top: place.top, left: place.left, maxHeight: place.maxHeight }
                : { visibility: "hidden" }
            }
            onKeyDown={onMenuKeyDown}
          >
            {items.map((item, index) => (
              <Fragment key={item.key}>
                {index === firstDanger && index > 0 && <hr className="menu-sep" />}
                <button
                  ref={(node) => {
                    itemRefs.current[index] = node;
                  }}
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  className={item.danger ? "menu-item menu-item-danger" : "menu-item"}
                  title={item.hint}
                  aria-disabled={item.disabled || undefined}
                  onClick={() => run(item)}
                  onKeyDown={(event) => {
                    // 自己接 Enter/Space，不等瀏覽器合成 click：Space 的 click 要到 keyup 才來
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      run(item);
                    }
                  }}
                >
                  {item.icon}
                  <span className="btn-label">{item.label}</span>
                </button>
              </Fragment>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}
