// 貼底：捲動容器原本停在最底，內容或容器尺寸變了（陣容欄寬窄切換、視窗縮放、輸入區長高、
// 文字重新折行）仍留在最底；玩家自己往上捲離開底部就不動它。
import { type RefObject, useCallback, useEffect, useRef } from "react";

// 離底不到這個距離都算「在底部」：小數捲動位置與捨入誤差
const BOTTOM_SLACK_PX = 8;

export function useStickToBottom(ref: RefObject<HTMLElement | null>) {
  const stuck = useRef(true);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const onScroll = () => {
      stuck.current =
        element.scrollHeight - element.scrollTop - element.clientHeight <= BOTTOM_SLACK_PX;
    };
    element.addEventListener("scroll", onScroll);
    // 尺寸變了才補捲：版面變高時瀏覽器不會自己動 scrollTop，變矮時夾回的位置本來就在底
    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(() => {
            if (stuck.current) element.scrollTop = element.scrollHeight;
          });
    observer?.observe(element);
    return () => {
      element.removeEventListener("scroll", onScroll);
      observer?.disconnect();
    };
  }, [ref]);

  /** 程式主動捲到底（換桌、換幕、新訊息）後呼叫：之後的尺寸變化也要跟著貼底 */
  return useCallback(() => {
    stuck.current = true;
  }, []);
}
