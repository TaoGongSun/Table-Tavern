// 自製對話窗共用外框：原生 <dialog> showModal()，top layer 與背景 inert 交給瀏覽器。
// ModalShell 只管開關、Esc、遮罩與焦點；Dialog 在上面排標題、工具列、內容與底部按鈕列
// （次鈕靠左、主鈕最後靠右）。系統確認窗（plugin-dialog）不走這裡。
import {
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  type ReactNode,
  type Ref,
  useId,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
} from "react";
import { t } from "../../i18n";
import { IconClose } from "./icons";

// 開啟中的對話窗由下到上：歸還焦點時只還給頂層那一個裡面的元素（其餘都在 inert 背景裡）
const openStack: HTMLDialogElement[] = [];

function removeFromStack(dialog: HTMLDialogElement) {
  const index = openStack.indexOf(dialog);
  if (index >= 0) openStack.splice(index, 1);
}

function pushToStack(dialog: HTMLDialogElement) {
  removeFromStack(dialog);
  openStack.push(dialog);
}

function canReturnFocus(target: HTMLElement | null, closing: HTMLDialogElement) {
  if (!target || !target.isConnected || closing.contains(target)) return false;
  if (target.matches(":disabled") || target.closest("[inert]")) return false;
  const top = openStack[openStack.length - 1];
  return top === undefined || top.contains(target);
}

function pointOutside(dialog: HTMLDialogElement, x: number, y: number) {
  const box = dialog.getBoundingClientRect();
  return x < box.left || x > box.right || y < box.top || y > box.bottom;
}

interface ModalShellProps {
  className?: string;
  labelledBy?: string;
  label?: string;
  /** Esc（與 backdrop 為真時點遮罩）呼叫；省略＝關不掉（例如執行中），由內容自己的按鈕收 */
  onDismiss?: () => void;
  /** 點遮罩是否等同 onDismiss */
  backdrop?: boolean;
  ref?: Ref<HTMLDialogElement>;
  children: ReactNode;
}

export function ModalShell({
  className,
  labelledBy,
  label,
  onDismiss,
  backdrop = false,
  ref,
  children,
}: ModalShellProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  // 主動關（卸載）時先立旗，close 事件就不會被當成意外關閉而重開
  const closingRef = useRef(false);
  const downOutsideRef = useRef(false);
  useImperativeHandle(ref, () => dialogRef.current!, []);

  useLayoutEffect(() => {
    const dialog = dialogRef.current!;
    const trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closingRef.current = false;
    if (!dialog.open) dialog.showModal();
    pushToStack(dialog);
    const initial =
      dialog.querySelector<HTMLElement>("[data-autofocus]") ??
      dialog.querySelector<HTMLElement>("[data-dialog-content]");
    initial?.focus();

    const onCancel = (event: Event) => {
      if (event.target === dialog) event.preventDefault();
    };
    // 非主動的 close（WebView 連按 Esc 繞過 cancel 之類）照樣重開；StrictMode 重掛時
    // 前一輪 close 事件可能晚到，那時已重新開著，open 為真就不理
    const onClose = (event: Event) => {
      if (event.target !== dialog || closingRef.current) return;
      if (dialog.isConnected && !dialog.open) {
        dialog.showModal();
        pushToStack(dialog);
      }
    };
    dialog.addEventListener("cancel", onCancel);
    dialog.addEventListener("close", onClose);
    return () => {
      dialog.removeEventListener("cancel", onCancel);
      dialog.removeEventListener("close", onClose);
      closingRef.current = true;
      dialog.close();
      removeFromStack(dialog);
      // 等這次 commit 的 DOM 更新都落地再判斷：觸發鈕可能同一輪才變 disabled／inert，
      // 或同一輪又開了別的對話窗（StrictMode 重掛也是）
      queueMicrotask(() => {
        if (canReturnFocus(trigger, dialog)) trigger!.focus();
      });
    };
  }, []);

  function onKeyDown(event: KeyboardEvent<HTMLDialogElement>) {
    if (event.key !== "Escape") return;
    // 組字中的 Esc 是給輸入法取消候選字的：不關窗，但也不讓外層（介面卡、上層對話窗）收到
    event.stopPropagation();
    if (event.nativeEvent.isComposing) return;
    event.preventDefault();
    onDismiss?.();
  }

  // 按下與放開都得落在框外的遮罩上才算：框內拖選文字拖到外面放開不關
  function onPointerDown(event: PointerEvent<HTMLDialogElement>) {
    const dialog = dialogRef.current!;
    downOutsideRef.current =
      event.target === dialog && pointOutside(dialog, event.clientX, event.clientY);
  }

  function onClick(event: MouseEvent<HTMLDialogElement>) {
    const dialog = dialogRef.current!;
    const downOutside = downOutsideRef.current;
    downOutsideRef.current = false;
    if (!backdrop || !onDismiss || !downOutside) return;
    if (event.target === dialog && pointOutside(dialog, event.clientX, event.clientY)) onDismiss();
  }

  return (
    <dialog
      ref={dialogRef}
      className={className}
      aria-labelledby={labelledBy}
      aria-label={label}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onClick={onClick}
    >
      {children}
    </dialog>
  );
}

/** 換字的按鈕：所有文案疊在同一格、寬度取最寬者，換字時按鈕不縮放、旁邊的鈕不位移。 */
export function SwapLabel({ labels, current }: { labels: string[]; current: number }) {
  return (
    <span className="dialog-swap">
      {labels.map((text, index) => (
        <span key={index} aria-hidden={index !== current}>
          {text}
        </span>
      ))}
    </span>
  );
}

interface DialogProps {
  title: ReactNode;
  size?: "s" | "m" | "l";
  className?: string;
  onDismiss?: () => void;
  backdrop?: boolean;
  /** 標題列右側的 ×：呼叫 onDismiss，沒有 onDismiss 時停用 */
  closeButton?: boolean;
  /** 標題下方固定不捲的工具列 */
  toolbar?: ReactNode;
  /** 底部靠左的次鈕 */
  start?: ReactNode;
  /** 底部靠右的鈕，主鈕放最後 */
  end?: ReactNode;
  children?: ReactNode;
}

export function Dialog({
  title,
  size = "m",
  className,
  onDismiss,
  backdrop = false,
  closeButton = false,
  toolbar,
  start,
  end,
  children,
}: DialogProps) {
  const titleId = useId();
  return (
    <ModalShell
      className={`dialog dialog-${size}${className ? ` ${className}` : ""}`}
      labelledBy={titleId}
      onDismiss={onDismiss}
      backdrop={backdrop}
    >
      <div className="dialog-head">
        <h2 id={titleId} className="dialog-title">
          {title}
        </h2>
        {closeButton && (
          <button
            type="button"
            className="btn btn-ghost btn-icon dialog-close"
            aria-label={t("closeBtn")}
            title={t("closeBtn")}
            disabled={!onDismiss}
            onClick={onDismiss}
          >
            <IconClose />
          </button>
        )}
      </div>
      {toolbar && <div className="dialog-toolbar">{toolbar}</div>}
      <div className="dialog-body" tabIndex={-1} data-dialog-content="">
        {children}
      </div>
      {(start || end) && (
        <div className="dialog-foot">
          <div className="dialog-foot-start">{start}</div>
          <div className="dialog-foot-end">{end}</div>
        </div>
      )}
    </ModalShell>
  );
}
