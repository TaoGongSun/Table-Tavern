# 已完成任務

結案任務按結案時間新到舊排列。進行中與待辦見 [TASKS.md](TASKS.md)。

- [card-import-flow](tasks/card-import-flow.md) — 匯入流程 v2：單一入口＋收據復原＋第二張卡路由 — 2026-08-06 實機驗收清單七項全數通過，結案。四包（指 GM／收據復原／單一入口分流／第二張卡路由）＋十項後續修正；最後一輪由樣本卡實測驅動的四件：雙世界書由硬擋改成可融合、身分判定改版（世界書內容寫在人設欄的卡也匯得進來，備用開場白數當主按鈕判準）、收據為空時拿條目當保險、角色卡路徑也給開場白。三條被推翻的原拍板：雙世界書硬擋、純角色卡零詢問直匯、配套世界書 companion 零打擾（配套與否程式偵測不出來，只有玩家知道）。明細見 [handoffs/archive/card-import-flow-completed.md](handoffs/archive/card-import-flow-completed.md)
- [st-ecosystem-upgrades](tasks/st-ecosystem-upgrades.md) — SillyTavern 生態升級：匯入補強＋巨集替換＋訊息 Markdown＋GM 狀態欄＋條目互轉＋開場白選擇 — 2026-08-04 六項全部實作完成（cargo test 174 綠）並全數實機驗收通過，狀態欄後續鏈（GM 更新／手改／收回倒回／壞格式）末四條收尾；第四項第二期的數值機制拆成獨立任務 [state-values-mvu](tasks/state-values-mvu.md)，結案
- [cli-install-windows](tasks/cli-install-windows.md) — CLI 一鍵安裝 Windows 支援（PowerShell 分支） — 2026-08-03 Grok 回報者確認連上，Windows 全流程（一鍵安裝→登入→聊天）通過，系統代理自動下傳同時獲證（代理確實傳進聊天子程序）；含鎖檔白話提示、系統代理下傳、grok 探針換 `grok models` ＋ pre_probe，結案（Stage C 與安裝階段可見視窗留待日後拍板）
- [release-4-theme-pack](tasks/release-4-theme-pack.md) — 發佈 4：佈景主題引擎＋贊助包（回禮內容） — 2026-08-03 使用者拍板縮減結案：贊助解鎖檔（`.ttpack`）與 Ko-fi 商品頁連結已上線並實機驗收；原 scope 的主題載入引擎／自選桌布／AI 產生主題確認不做，五套贊助配色維持寫死在 App.css，NewPlan §16.1 同步縮減，結案
- [theme-pack-component-skins](tasks/theme-pack-component-skins.md) — 主題包元件裝飾：華麗發言送出鈕（贊助包 v2 回購誘因） — 2026-08-03 隨 release-4 主題引擎不做一併關閉：沒有主題檔格式可掛「元件裝飾」schema，免費版送出鈕功能完整不受影響；日後若重啟主題引擎再一併復活，結案
- [cli-auto-connect](tasks/cli-auto-connect.md) — CLI 自動連接：背景偵測＋登入跳轉自動回 — 2026-08-03 使用者拍板不做：cli-install-windows 的一鍵安裝已解掉最痛的安裝段，CLI 登入是一次性動作；且「風險告知勾選不可被自動化繞過」使自動化上限本來就低，投報比不足，結案（未做前置查證即關閉）
- [ai-error-messages](tasks/ai-error-messages.md) — AI 失敗訊息人話化：額度用完與未登入分流 — 2026-07-28 使用者實機確認兩類訊息都正常顯示，結案
- [drag-reorder-lists](tasks/drag-reorder-lists.md) — 角色卡與世界書條目改拖曳排序（GM 固定最上、刪 ↑↓ 鈕、列不可反白） — 2026-07-27 使用者九項實測全通過（含順序持久化與舊卡不跳位回歸），結案
- [character-image-avatar](tasks/character-image-avatar.md) — 角色圖片管理：加入／更換／移除全身圖＋圓形頭像取代 emoji＋lightbox＋透明 app 圖示 — 2026-07-27 使用者實測通過（含五輪回饋修訂與 Mac DMG 0.2.0 打包驗證），結案
- [post-mvp-more-cli-providers](tasks/post-mvp-more-cli-providers.md) — 擴充 CLI 訂閱供應商（agy／Gemini CLI 端到端＋一鍵安裝＋Grok CLI） — 2026-07-24 使用者實測三路全過（agy 實聊、一鍵安裝終端機流程、grok 實聊），結案
- [worldbook-st-format](tasks/worldbook-st-format.md) — 世界書 v2：ST 相容條目化＋一鍵匯入＋可見性資訊邊界 — 2026-07-24 使用者實測通過（匯入、條目管理、置頂與移動、未儲存提示、資訊邊界實聊：指定角色知情／未指定不知情），結案
- [post-mvp-character-archive](tasks/post-mvp-character-archive.md) — MVP 後：角色卡隱藏區（軟刪除）＋真刪除警告 — 2026-07-24 使用者實測收起／還原／刪除確認框通過，結案
- [scene-history-browser](tasks/scene-history-browser.md) — 前幕：場景歷史瀏覽＋單幕匯出＋主欄閱讀優先版面 — 2026-07-24 使用者驗收通過（含三輪回饋修訂），結案
- [post-mvp-scene-summary](tasks/post-mvp-scene-summary.md) — MVP 後：場景切換＋場景摘要 — 2026-07-24 使用者實測換場通過，結案
- [sample-world-i18n](tasks/sample-world-i18n.md) — 範例桌內容依語系產生（首開先選語言） — 2026-07-24 使用者實測首開通過，結案
- [ui-settings-panel](tasks/ui-settings-panel.md) — 設定視窗：單一入口內分頁（外觀預設／AI 連線）＋文字大小五檔 — 2026-07-24 使用者複驗通過，結案
- [ui-layout-rework](tasks/ui-layout-rework.md) — 版面重構：角色卡移左側欄＋桌列表可摺疊（NewPlan §9.4） — 2026-07-23 使用者視覺驗收通過，結案
- [transcript-export](tasks/transcript-export.md) — 一鍵下載跑團紀錄（劇情歷史匯出） — 2026-07-23 使用者實測另存對話框存檔通過，結案
- [post-mvp-st-import](tasks/post-mvp-st-import.md) — MVP 後第一優先：SillyTavern 角色卡匯入（含存 PNG＋角色圖顯示/隱藏） — 2026-07-23 使用者實測匯入範例卡＋角色圖開關通過（附截圖），結案
- [ui-i18n-switch](tasks/ui-i18n-switch.md) — UI 語系切換（zh-TW／en） — 2026-07-23 前端 i18n 字典＋語言下拉、後端 LANGUAGE_RULE 依語系注入，npm build＋cargo test 全綠，結案
- [post-mvp-i18n-language-rule](tasks/post-mvp-i18n-language-rule.md) — MVP 後：多語系時 LANGUAGE_RULE 改依使用者語系注入 — 2026-07-23 隨 ui-i18n-switch 完成，結案
- [mvp-7-packaging](tasks/mvp-7-packaging.md) — MVP 切片 7：打包 DMG＋README — 2026-07-22 實測 Gatekeeper，修掉 linker-signed 被判「已損毀」的缺陷＋README 步驟更新，結案（公證移交 release-1）
- [mvp-6-onboarding](tasks/mvp-6-onboarding.md) — MVP 切片 6：Onboarding（BYOK 引導） — 2026-07-22 使用者實測首開範例桌＋BYOK 面板通過，另修冪等／幣別文案／按鈕間距，結案
- [mvp-4-director](tasks/mvp-4-director.md) — MVP 切片 4：簡易導演（GM） — 2026-07-22 使用者實測 world.md 編輯／GM 旁白／GM 推進全通過，結案
- [character-card-avatar-issues](tasks/character-card-avatar-issues.md) — 角色卡回饋修訂＋建卡流程改版＋刪除入口＋GM 卡 — 2026-07-28 七項全部實測通過，改名提示末項改為儲存時 confirm 後收尾；生成圖庫目錄放錯層留給 stable-id-storage 吃掉，結案
- [cli-detect-state](tasks/cli-detect-state.md) — CLI 偵測空窗期：加「正在偵測…」三態＋四支並行＋結果快取 — 2026-07-28 使用者實測設定頁重開不再閃「一鍵安裝」，結案（並行實測 2.8 倍）
- [cli-install-all-providers](tasks/cli-install-all-providers.md) — CLI 一鍵安裝擴充到 claude／codex／grok（比照 agy） — 2026-07-28 使用者實機按四家安裝／驗證鈕全數成功，結案
- [cli-connected-badge](tasks/cli-connected-badge.md) — CLI 已連結狀態記憶：按鈕換「已連結 ✓」＋不重發登入 — 2026-07-28 Mac 全數實測通過（補上 Mac 缺的驗證回傳通道、grok 探針換 `grok models` 由 26 秒降到瞬間、badge 對齊）；Windows 端新探針與 pre_probe 併 cli-install-windows 那輪驗，結案
- [stable-id-storage](tasks/stable-id-storage.md) — 儲存改用穩定代碼定址（桌與角色的路徑是代碼，顯示名是欄位） — 2026-07-28 實作＋實機驗收全通過：同名角色並存可分辨、改名不動任何路徑、舊對話仍顯示舊名、桌名可含 `/`；順帶修掉生成圖庫放錯層與空桌誤回收，結案
- [player-card](tasks/player-card.md) — 玩家角色卡：給玩家名字與社會身份，NPC 能叫得出你 — 2026-07-28 實作完成並實機建卡驗收：玩家卡＝存該桌 characters/ 下的一張角色卡（id 記在 state.json 的 player_card_id，list_characters 濾掉它，後端零新命令），提示詞注入玩家身份、GM 改喊玩家名字時映射回哨兵；同輪把編輯畫面切成三塊、送出鈕移左、有圖時收起 emoji 欄。最後補上：代號改 `__PLAYER__`（人名撞不到）、GM 點到玩家會留下點名紀錄再停，實機驗證通過，結案
- [card-export](tasks/card-export.md) — 角色卡匯出（SillyTavern chara_card_v2）：編輯畫面「匯出卡」另存 PNG（tEXt chara chunk）或 JSON — 2026-07-28 內容由現在的卡重建（公開五段回原欄位、手寫卡歸簡介、私有筆記轉自帶世界書），底圖取卡片圖→頭像→1×1 透明並清掉舊的 chara／ccv3；cargo test 110 綠＋新增四項測試，待使用者實機丟回 ST 驗收，結案
