// VN 播放器的鍵盤（Ren'Py 慣例）：Space／Enter／→／↓／PageDown 推進、←／PageUp／Backspace 上一格、按住 Ctrl 快進、
// Esc 開遊戲選單（面板或紀錄開著＝關它）、A 自動、H 隱藏介面。掛在 window，播放器不在畫面上就不掛。
import { useEffect, useRef } from "react";

/** 蓋在播放器上的東西：closable＝面板或紀錄（Esc 關）；menuOnly＝完結畫面（只收 Esc 開選單）；blocked＝標題、字卡、匯出 */
export type VnLayer = "closable" | "menuOnly" | "blocked" | null;

export interface VnKeyActions {
  layer: VnLayer;
  hidden: boolean;
  onClose: () => void;
  onMenu: () => void;
  onUnhide: () => void;
  onHide: () => void;
  onAdvance: () => void;
  onBack: () => void;
  onAuto: () => void;
  /** 按住 Ctrl：true＝開始快進、false＝放開（還原按下前的快進開關） */
  onSkipHold: (held: boolean) => void;
}

const ADVANCE = new Set([" ", "Enter", "ArrowRight", "ArrowDown", "PageDown"]);
const BACK = new Set(["ArrowLeft", "PageUp", "Backspace"]);

const typingIn = (target: EventTarget | null) =>
  target instanceof Element && target.closest("input, textarea, select, [contenteditable='true']") !== null;

export function useVnKeys(actions: VnKeyActions) {
  const ref = useRef(actions);
  ref.current = actions;
  useEffect(() => {
    let holding = false;
    const release = () => {
      if (!holding) return;
      holding = false;
      ref.current.onSkipHold(false);
    };
    const down = (event: KeyboardEvent) => {
      const a = ref.current;
      // app 層的對話窗（回合失敗等原生 modal <dialog>）開著：鍵盤歸它
      if (event.isComposing || document.querySelector("dialog[open]")) return;
      // Esc 先處理：聚焦在設定欄位、存檔名欄位時也要能關面板
      if (event.key === "Escape") {
        if (a.layer === "closable") a.onClose();
        else if (a.hidden) a.onUnhide();
        else if (a.layer === null || a.layer === "menuOnly") a.onMenu();
        else return;
        event.preventDefault();
        return;
      }
      if (a.layer !== null || typingIn(event.target)) return;
      if (a.hidden) {
        event.preventDefault();
        a.onUnhide();
        return;
      }
      if (event.key === "Control") {
        if (!holding) {
          holding = true;
          a.onSkipHold(true);
        }
        return;
      }
      if (event.repeat || event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target instanceof Element ? event.target : null;
      // 其他按鈕的 Space／Enter 照原生啟動；對話框本身吃掉，免得原生 click 再推一次
      if ((event.key === " " || event.key === "Enter") && target?.closest("button") && !target.closest(".vn-dialogue")) return;
      if (ADVANCE.has(event.key)) a.onAdvance();
      else if (BACK.has(event.key)) a.onBack();
      else if (event.key === "a" || event.key === "A") a.onAuto();
      else if (event.key === "h" || event.key === "H") a.onHide();
      else return;
      event.preventDefault();
    };
    const up = (event: KeyboardEvent) => {
      if (event.key === "Control") release();
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", release);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", release);
      release();
    };
  }, []);
}
