# Task
Task-ID: act-fork-missing
Title: 單幕閱讀的「從這一幕繼續」按鈕沒有渲染出來
Status: todo

## Summary
開任何一桌的前幕（標題列「前幕（N）」→ 選一幕），單幕閱讀畫面的標題列只有「匯出本幕」與「返回」兩顆按鈕，第三顆「從這一幕繼續」（分岔續玩，`act-fork`）看不到。玩家因此無法從舊幕分岔續玩——這是那個畫面唯一能往前推進的動作。

2026-08-13 於 app-split 切片 5 的手動回歸中發現。**不是拆分造成的**：ActReader 的 JSX 與 `src/App.css` 從拆分起點 591a2e0 至今 `git diff` 完全空白，8/6 的舊 release 包與當天新打的包都一樣缺。

已排除的可能（都查過了，別重查）：
- JSX 有出貨：`dist/assets/index-*.js` 裡有 `className:"act-fork"` 與 `children:d("sceneFork")`，就接在 `backToNow` 那顆後面。
- 文案有出貨：bundle 內 `sceneFork:"從這一幕繼續"`（10 語系都在）。
- CSS 有出貨：`.act-fork{margin-left:auto}`、`.act-reader-header{flex:none;display:flex;align-items:center;gap:.75rem}`；全 dist CSS 只有這兩條碰得到，沒有任何 `display:none`／`visibility`／`opacity:0` 規則命中它。
- 全域 `button` 規則給了 `height:var(--control-h)` 與 `padding:0 1.05em`，所以就算文字是空字串也該是一顆看得見的膠囊。
- 父層 `.chat-body{display:flex;flex-direction:column}` 是預設 `align-items:stretch`，標題列吃滿寬，`margin-left:auto` 應該把它推到最右邊（螢幕約 x=1290）。實測那塊區域整片空白。

現況：程式碼、樣式、文案三邊都正確出貨，但畫面上就是沒有——缺的是 DOM 層的實地檢查，而 release 包沒有開發者工具。

## Next action
用 `npm run tauri dev` 起開發版（webview 可右鍵檢查元素），開任一桌的前幕，在 Elements 面板確認 `.act-fork` 節點是否存在、computed style 的 display／width／position 各是什麼，據此判斷是修樣式還是重做這顆按鈕。

## Constraints
- 這是 app-split 拆分任務期間發現的既有問題，**排在 15 個切片全部跑完之後才動**（使用者 2026-08-13 拍板），避免和拆分改動混在一起。
- 修好要在真桌上實測一次分岔續玩（`forkScene`）確實會建出新的一幕，不是只有按鈕出現。
- `sceneFork` 已在 10 語系齊備，改動若碰到文案要跑 `npm run check:i18n`（按鈕數只允許持平或上升）。
