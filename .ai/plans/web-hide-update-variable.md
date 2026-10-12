# 網頁版藏 `<UpdateVariable>`＋網頁存檔匯入提醒改寫

公開前門檻〔作者裁決 2026-10-12：公開前修〕。只修這兩件，罕見邊際列已知限制〔作者裁決 2026-10-12〕。

## 現況
- 網頁版 GM 訊息照原文顯示整段 `<UpdateVariable>`（[composite-card-continue](composite-card-continue.md) ②類）。逐字稿 `text` 只過一般 regex，顯示時 `displayText`（`web/src/features/chat/st-text.ts`）只套卡的 markdownOnly regex；串流中 `chat.streaming` 原文直接畫（`ChatView.tsx` 的 `streamingHtml`）。
- 桌面版在落檔層剝：`extract_state_block`（`src-tauri/src/transport/response.rs`）剝閉合的 `<UpdateVariable>…</UpdateVariable>`（交給 mechanism 解析），`cut_unclosed_tail` 切沒閉合的 `<UpdateVariable` 到結尾，存 `text`，原文存 `raw` 給卡片介面；串流時 `streamDisplayText` 從 `<updatevariable` 截掉、結尾停在標記前半先扣住。同一套也剝 `<StatusPlaceHolderImpl/>`（自閉合 `<status…/>`）——那是桌面版自己畫狀態欄。
- 酒館：MVU 擴充本身不藏，教學要卡作者加局部 regex `/<UpdateVariable>[\s\S]*?<\/UpdateVariable>/gm`（AI 輸出、僅格式顯示＋僅格式提示詞）；`<StatusPlaceHolderImpl/>` 是由卡的顯示 regex 換成狀態欄。DongeonMaster-safe 三支 regex 沒有藏 UpdateVariable 的那支，所以照搬 ST 也會露出。
- 網頁存檔匯入提醒 `importWebSaveWorldbookRules`（`src/i18n/*.ts`，桌面版 `useImportController.ts` 用）還說桌面版沒有 sticky／cooldown；worldbook-st-trigger-parity 後桌面版已照 ST 觸發、匯入時帶計時狀態（`import/web_save/mod.rs` `write_world_info_timed`）。

## 做法
1. **只改顯示，存檔不動**：`ChatEntry.text`／`raw`、網頁存檔、卡片介面拿到的訊息、MVU 解析全照原文（網頁版 MVU 從訊息原文讀指令，匯入桌面版也靠原文）。
2. 新增 `web/src/features/mvu/hide-update-variable.ts` 兩個純函式（規則照桌面版，只管 UpdateVariable；位置一律用帶 `/i` 的 regex 找，不先轉小寫再拿位置回原文切；標籤名要完整，`<UpdateVariable` 後面接 `>`、空白或結尾）：
   - `hideUpdateVariable(text)`：剝所有閉合的 `<UpdateVariable…>…</UpdateVariable>`，再從第一個剩下的 `<UpdateVariable` 切到結尾；切掉的尾巴裡有宿主補的 `<StatusPlaceHolderImpl/>`（`finishReplyText`）就補回結尾，狀態欄照畫。
   - `hideUpdateVariableStreaming(text)`：從第一個 `<UpdateVariable` 截到結尾；結尾停在它的前半（含單獨 `<`）先扣住。
3. 接線：`displayText` 對 `role === "char"` 的訊息先 `hideUpdateVariable`、再套卡的 markdownOnly regex（反過來的話占位已換成狀態欄 HTML，切尾會吃掉）；`streamingHtml` 改畫 `hideUpdateVariableStreaming(chat.streaming)`。
4. **送模的歷史不剝**：照 ST，卡自己帶 promptOnly regex 才剝。網頁版每回合不注入變數現值，沒有 stat_data 巨集也沒有 promptOnly regex 的卡（如 DongeonMaster-safe），歷史裡的 UpdateVariable 是模型看到變數現值的唯一來源。
5. **`<StatusPlaceHolderImpl/>` 不剝**：網頁版靠卡的顯示 regex 把它換成狀態欄（迷宫之书就是），剝了介面就不見；桌面版剝它是因為自己畫狀態欄，兩邊做法不同有理由。
6. 提醒改寫十語系：說明計時狀態（sticky、cooldown 等）已帶過來，只有少數特殊寫法的條目觸發結果可能略有出入。zh-TW 草稿：「提醒：世界書的觸發狀態（sticky、cooldown 等計時）已一併帶過來；少數特殊寫法的條目在桌面版的觸發結果可能和網頁版略有出入。」

## 測試
- `hide-update-variable.test.ts`：跑 `src/shared/contracts/reply-cleanup/cases.json` 的 `unclosed-update`、`unclosed-update-upper`、`closed-update-then-text`（final 欄配顯示函式、stream 欄配串流函式，與桌面版同一份案例）；另加截斷回歸：`正文<UpdateVariable>半截\n\n<StatusPlaceHolderImpl/>` 顯示成正文加占位。
- `mvu-turn.test.tsx` 加斷言：`displayText` 回傳的字串沒有區塊內文（`_.set(`）、保留正文與占位，逐字稿 `text` 仍含原文。
- `npm run verify` 綠；提醒只改字串，既有 `web-save-routing.test.tsx` 用 `t()` 不需動。

## 已知限制
- 卡自己用顯示 regex 把 `<UpdateVariable>` 換成別的東西的（美化摺疊、HeroTraining 與 bcd368 的「[变量更新完成]」提示），網頁版先剝了就看不到，與桌面版一致。
- 整則只有 UpdateVariable、沒有正文時畫面是空泡泡。
