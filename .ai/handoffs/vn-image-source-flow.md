# vn-image-source-flow

Status: 七項施工已通過Grok工程驗收、主線verify全綠（使用者回報）。作者換章裁決、第2輪12項及結案前5項文字修正均已完成；待主線原生重拍。分支`vn-image-source-flow`，只commit，不push／合併。
規格：[方案](../plans/vn-image-source-flow.md)。

## 現況

- 第2輪opus-review及截圖共12項全部採納。標題讀回前、完成及重播桌不出美術提示；未開始且缺立繪提示到美術與音樂開始；已開始缺立繪且無失敗顯示進度；有失敗顯示數量與重試；額度暫停保留恢復提示。十語系齊全。
- `vn_art_status`只有started桌回傳全域來源pause；不改generate_png gate，因此未開始桌手動候選撞額度時按鈕自己的錯誤仍在。來源pause仍在記憶體、不持久化。
- 唯一timer改為最多每分鐘比對實際UTC，闔蓋後不再睡滿原本剩餘時長；取消／替換及gate內期限重驗照舊。起床到檢查最多仍有一分鐘延遲。
- 章卡額度鈕文字獨立為「生圖額度用完 · 查看」，開既有美術與音樂；平時角標仍是「生圖額度用完」。普通失敗鈕只重試目前backdrop，其他地點失敗不再顯示為章卡重試。章卡隱藏角標、結局隱藏角標；精靈／標題維持隱藏角標，精靈另隱藏左上地點字。
- 面板z-index 6高於章卡4，只切panel狀態，不卸載播放器、不停止開場；既有整合測試確認面板開著仍可寫完開場，關閉後保留內容。原生實際疊放仍待主線截圖。
- 張數錯位原因：估計只算第一個背景，暫停文案卻用含表情及失敗紀錄的queue長度。現在背景計本幕所有不同缺圖地點，估計與剩餘共用缺定稿＋缺背景統計；稍後角色表情仍獨立列總數，不能把兩種範圍混加。
- 美術與音樂的角色頁只留面板頂部來源下拉；獨立角色編輯仍有自己的下拉。共用useArtStatus讀取／訂閱以顯示pause，定稿／表情／變體及角色清單遇暫停的排隊狀態改等額度恢復。
- 表情數一致只顯示每人約N張（既有facesPerCharacter），不一致才列全部；中文數字與單位加空格。相對時間用完整單位及既有ICU單複數，保持小寫片段；十語系同步。
- 美術說明兩段原受共用field-note負margin影響，區域內改明確flex間距、段落margin 0與line-height 1.5；不硬拼句間空格。地點膠囊樣式與既有清理保留。
- queue／prepare、CLI、lane與生成gate未改。未呼叫模型、GUI、ignored／capture測試；未變更權限、登入或Connector。其他工作線文件未動。

## 驗證

- 本輪相關Vitest 4檔75例通過，包含章卡專用文案、平時角標維持原字樣、每人張數與不一致退回總數、英文1小時／2小時及1分鐘／40分鐘；十語系檢查通過。
- `npm run verify`：結構、版本、harness隔離、個資、fmt、桌面162檔2059例、十語系、桌面build、web48檔878通過1跳過、web build、party格式通過；party 273通過、同批5例listen EPERM，停第12步。party typecheck／build另補跑通過。
- 無Rust變更，本輪未重跑Rust套件／harness；前輪pause 2例、命令顯示1例、cargo check及harness 44例已通過。主線仍需在可開TCP的環境跑完整verify。
- 本輪核心7行、測試10行（加刪計，排除十語系62行）；測試未超核心2倍。

### 本機TCP受限清單（party/server/node/node-server.test.ts）

- 建房、執行端與成員連線；ev送出時已落地；同DB重開序號接續／舊連線離線。
- 錯誤訊框與畸形路徑、靜態檔、穿越、來源與限流。
- 45秒心跳逾時關線與離線廣播。
- 玩家卡上傳／執行端取檔／秘密段的真HTTP與WS。
- index.html指向根外連結時不送單頁fallback。

未提交skip設定，主線直接重跑verify。

## 下一步與已知限制

- 主線重跑verify、安排真app重拍：新桌精靈不露地點／其他桌pause、標題未開始／未齊／完成／失敗／重播、章卡只重試目前地點、結局無角標、角色頁單一下拉與等額度字樣、數字相符及十語系排版。DOM測試不代表原生視覺驗收。
- 沿用已接受限制：全域來源、pause不落檔、未知reset手動重試、重開重新推導；故事拒絕只識別已知短句，舊立繪不遷移。真實agy品質與圖片比例由主線安排；本輪未做真機闔蓋過夜測試。

## 本輪功能檔前後行數

基準`5c46ee1b`。十語系字典1067行是同一純資料owner，未機械拆表。

| 檔案 | 前 | 後 |
| --- | ---: | ---: |
| `src/features/vn/ChapterCardView.tsx` | 77 | 77 |
| `src/features/vn/art-flow/ArtFlowNotice.tsx` | 49 | 49 |
| `src/features/vn/art-flow/useQuotaRecovery.test.ts` | 33 | 35 |
| `src/features/vn/background-flow.test.tsx` | 51 | 51 |
| `src/features/vn/vn-sprites.css` | 234 | 235 |
| `src/i18n/features/vn-wizard.ts` | 1057 | 1067 |
