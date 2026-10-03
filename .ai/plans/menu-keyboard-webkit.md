# menu-keyboard-webkit — 修法與驗證

交接：[handoffs/menu-keyboard-webkit.md](../handoffs/menu-keyboard-webkit.md)

## 根因（有證據）

焦點移動本身正常；壞的是「看不看得到」。

- 樣式：選單項的反白與外框原本只掛在 `:focus-visible`。
- WebKit 的 `:focus-visible` 判斷：上一次取得焦點是滑鼠點擊時，之後由程式 `focus()` 移過去的焦點**不**符合 `:focus-visible`，按了被 `preventDefault` 的 ↑↓ 也不改；按一個沒被攔的鍵（←→）時，當下聚焦的元素才變成 `:focus-visible`，下一次 ↑↓ 移焦又不符合。
- 對上使用者描述：↑↓ 其實有移焦但沒畫面變化；→ 把當下那項亮出來；「先 ↑ 再 →」＝↑ 移了焦、→ 亮出來。
- 觸發條件是頁面上前一次焦點來自滑鼠點擊（例如先點過世界設定的文字框）。乾淨頁面直接點 ⋯ 不會發生，所以獨立元件測試抓不到。

| 引擎 | 前次滑鼠焦點後 ↑↓ 移焦可見？ |
|---|---|
| 系統 WKWebView（macOS 26.5.1、Safari 26.5、WebKit 21624.2.5.11.4） | 否（重現） |
| Playwright 1.55.1 WebKit 26.0（rev 2203） | 否（重現） |
| Playwright 1.63 WebKit 26.6（rev 2359） | 是（較新 WebKit 已改判斷，抓不到回歸） |
| Chrome | 是；只差滑鼠開啟時第一項沒反白 |

`focus({ focusVisible: true })` 自 Safari 18.4 的 WebKit 起支援；Tauri 用系統 WKWebView，支援與否看使用者的 macOS，不能由 Tauri 版本推定。

## 修法（已實作：`src/shared/ui/MoreMenu.tsx`、`src/styles/controls.css`、`src/styles/base.css`）

1. **輸入來源明確傳遞**：開啟、項目執行、返回 ⋯ 鈕都帶 `FocusSource`（`pointer`｜`keyboard`）。click 以 `event.detail > 0` 判為指標；`detail === 0` 涵蓋鍵盤 Enter/Space、輔助技術與程式 click，一律保守當成要顯示焦點。Enter/Space 在選單項上由 keydown 直接標成 keyboard。
2. **鍵盤移焦要求可見**：↑↓／Home／End、Esc、鍵盤執行後回 ⋯ 鈕、Tab 交回 ⋯ 鈕，皆 `focus({ focusVisible: true })`；指標造成的移焦傳 `focusVisible: false` 並 `preventScroll`。
3. **舊引擎兜底**：鍵盤移焦同時在元素上設 `data-focus-ring`，失焦即清；`base.css` 的焦點外框規則改為 `:focus-visible, [data-focus-ring]:focus`。不認 `focusVisible` 的引擎也有外框（測試以「剝掉 focusVisible 選項的 focus」模擬）。
4. **反白只跟著實際焦點**：`.menu-item:focus` 給底色，拿掉 `:hover` 底色；外框仍只給 `:focus-visible`／`data-focus-ring`。滑鼠 `pointermove`（只認 `pointerType === "mouse"`、目標不同才移）把焦點移到該項，所以游標停著、方向鍵移走時不會留第二個亮項，方向鍵也從滑鼠所在項接續。觸控與筆不搶焦點。
5. 停用項維持可聚焦（`aria-disabled`），滑鼠移入也會聚焦，執行防線仍在 `run()`。

不做：←→ 在選單內照舊不處理；其他元件不改。設定視窗分頁列已重現同類問題，另立 [settings-tabs-focus-visible](../handoffs/archive/settings-tabs-focus-visible.md)，本案不擴大，不代表全站已解決。

## 驗證

- **真實 WebKit 回歸**〔作者裁決 2026-10-02〕：vitest browser mode＋Playwright WebKit，`npm run test:webkit`（`vitest.webkit.config.ts` 只收 `*.webkit.test.tsx`），不進 `npm run verify`（一般 `vitest.config.ts` 明確排除 `*.webkit.test.tsx`）。`playwright` 鎖 `1.55.1`：它的 WebKit 26.0 與目前系統 WKWebView 同判斷，1.63 的 WebKit 26.6 已不重現。`@vitest/browser-playwright` 與 vitest 同版（4.1.10，lockfile 鎖定）。首次執行前 `npx playwright install webkit`。
  - `src/shared/ui/MoreMenu.webkit.test.tsx` 九案，點擊、移游標、按鍵全經 provider 真輸入，載入產品 CSS（關掉 transition 量終值）；每案先用滑鼠點文字框建立觸發條件，案後卸載重建。斷言具體焦點項、底色（只有一項亮）、外框。
  - 修前 7 敗 2 過（原本就成立的是「滑鼠執行回 ⋯ 鈕不留框」與「停用項可聚焦不執行」），修後 9 過。
  - macOS WebKit 預設 Tab 不停在按鈕上，「鍵盤開啟」案以程式把焦點放到 ⋯ 鈕後按 Enter。
- **happy-dom（`MoreMenu.test.tsx`）**：輔助鎖意圖——指標／非指標開啟、方向鍵帶 `focusVisible: true` 與 `data-focus-ring` 跟著焦點走、Esc／鍵盤執行留框而滑鼠執行不留、滑鼠 pointermove 移焦而觸控／筆不移。
- **系統 WKWebView 收尾**：附錄的 Swift 小程式重跑「點文字框 → 點 ⋯ → ↓↓↑ → Esc」，修後每步 `:focus-visible` 都符合，截圖可見外框。
- **實機**：排進[實測佇列](../reference/verification-queue.md)梯 1 第 2，含 VoiceOver 混用滑鼠與方向鍵（hover 移焦可能讓螢幕閱讀器播報，需實機確認不亂跳）。

## 附錄：最小重現

檔案在 [reference/menu-keyboard-webkit/](../reference/menu-keyboard-webkit/)：`world.html`（假 Tauri 後端）＋`world.tsx`（掛真的 `WorldEditor`）＋`wk.swift`（系統 WKWebView 驅動器：離畫面視窗，NSEvent 直送本視窗，不碰全域輸入；會短暫把自己設為前景 app，結束時還原）。

```sh
npx vite --port 5199 --strictPort            # repo 根目錄
swiftc -O .ai/reference/menu-keyboard-webkit/wk.swift -o /tmp/wk
/tmp/wk "http://localhost:5199/.ai/reference/menu-keyboard-webkit/world.html" \
  "click:textarea,click:[aria-haspopup=menu],Down,Down,Right,Up,Down,Right,Escape"
```

每步印出 `activeElement`、`:focus-visible` 命中元素，並存截圖。離畫面視窗的 transition 不跑，`bg` 欄位停在起始值，看底色要看截圖或 `npm run test:webkit`。修前（`373431b`）輸出關鍵行：

```
[click:[aria-haspopup=menu]] active=清理重複 focusVisible=null
[Down]  active=匯出世界書 focusVisible=null
[Down]  active=匯入重構卡 focusVisible=null
[Right] active=匯入重構卡 focusVisible=匯入重構卡
[Up]    active=匯出世界書 focusVisible=null
[Escape] active=BUTTON focusVisible=null
```

修後同步驟：每個 ↑↓ 與 Esc 都 `focusVisible` 命中當下焦點。
