// 把 .ttvn 拖進大廳就匯入（plans/vn-mode-pkg8.md §2.9）：Tauri 攔下系統拖放，HTML5 drop 收不到檔案，
// 只能聽 webview 的拖放事件。只收單一個作品包，其他檔案不理。
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef } from "react";
import { isWorkPath } from "./vn-work";

export function useWorkDrop(onWork: (path: string) => void) {
  const handler = useRef(onWork);
  handler.current = onWork;
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let gone = false;
    // 沒有 Tauri 視窗（網頁測試環境）就不聽
    Promise.resolve()
      .then(() =>
        getCurrentWebview().onDragDropEvent((event) => {
          if (event.payload.type !== "drop") return;
          const [path, ...rest] = event.payload.paths;
          if (path && rest.length === 0 && isWorkPath(path)) handler.current(path);
        }),
      )
      .then((stop) => (gone ? stop() : (unlisten = stop)))
      .catch(() => {});
    return () => {
      gone = true;
      unlisten?.();
    };
  }, []);
}
