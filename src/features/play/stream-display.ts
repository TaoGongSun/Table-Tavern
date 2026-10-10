// 串流中的回覆尾端會冒出控制區塊（狀態欄、`<UpdateVariable>`），整則寫完才由後端剝乾淨；
// 這裡先截掉，免得玩家每回合都看到一段圍欄或標籤閃過去。旁白與角色台詞共用（char-line-status-strip）。
// 規則與後端收尾共用案例：src/shared/contracts/reply-cleanup/cases.json（stream 欄）。

/** 從第一個出現處截到結尾，不管後面有沒有閉合（拿掉已閉合區塊、後文接著顯示只在落檔的最終文字做） */
const CONTROL_START = /```|<details|<status|<updatevariable/i;
/** 要拆的外殼，不是控制區塊：串流時只拿掉標籤字面 */
const SHELL_TAG = /<\/?maintext>/gi;
/** 結尾停在這些標記的前半時先扣住，等後續字到確定不是標籤再顯示 */
const HELD_MARKERS = ["```", "<details", "<status", "<updatevariable", "<maintext>", "</maintext>"];

export function streamDisplayText(text: string): string {
  const unshelled = text.replace(SHELL_TAG, "");
  const start = unshelled.search(CONTROL_START);
  const shown = start === -1 ? unshelled : unshelled.slice(0, start);
  const lower = shown.toLowerCase();
  let held = 0;
  for (const marker of HELD_MARKERS) {
    for (let length = marker.length - 1; length > held; length--) {
      if (lower.endsWith(marker.slice(0, length))) {
        held = length;
        break;
      }
    }
  }
  return held === 0 ? shown : shown.slice(0, shown.length - held);
}
