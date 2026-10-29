# card-prompt-only-regex 規格

案別：中案（核心估 ~190 行，≤300；實作核心 +262／−21，含註解與 rustfmt 展開；測試 +181）；不碰信任邊界、spawn、鎖，直接實作、做完送一位審查員驗收。

## 盤點（TestCards，2026-10-29）
有 promptOnly 腳本的卡 5 張（含複本 8 檔），世界書與重構卡都沒有。replaceString 全空、trimStrings 全空、substituteRegex 全 0。

| 形狀 | 卡 | findRegex | placement | 深度 |
|---|---|---|---|---|
| MVU 剝變數更新 | HeroTrainingUnderSide、bcd368…、clean-hero-roster | `/<(update(?:variable)?)>…/gsi`（反向參照、前看） | 1、2 | 無 |
| 剝狀態欄 | NorthHall（＋structure 版） | `.*?<Status_block>([\s\S]*?)<\/Status_block>`（無斜線＝無旗標） | 2 | 無 |
| 剝舊狀態欄／舊 BGM | RPGImmortal（2 條） | `<Status_block>…`、`<BGM>…`（無旗標） | 2 | minDepth 2 |

最小版涵蓋 5/5 張、3/3 形狀（minDepth 只是一個比較，一併做）。

## 做法
- 正則：`world_info/regex_key.rs` 加 ST 正則擴充的 `regexFromString` 語意（無斜線＝樣式本身、無旗標；`/樣式/旗標` 拆開），重用既有 JS→fancy-regex 轉譯與回溯上限；`g` 決定換全部或只換第一個。回溯超限或轉譯失敗＝該條不套、原文照送。
- 腳本來源：`read_card_interfaces` 讀原卡時另收 promptOnly 腳本（啟用、placement 非空；markdownOnly 不管）；D24 不允許 regex 的桌一併清空；不支援的卡不收。只收 replaceString 空、trimStrings 空、substituteRegex 0 的條目，其餘略過。
- 套用點：送模型渲染那一步（`render_for_prompt` 與 lane 的 `lane_event_line` 呼叫處），套在 `prompt_text` 回的本文、加名字前綴之前。玩家發言＝placement 1，GM 旁白與角色台詞＝2，系統事件不套。深度＝非系統事件從尾端數（最後一則 0），lane 對整份事件算、只渲染水位後。GM 與角色兩側都套（ST 同一份歷史同一套過濾）。
- 換幕後 GM system 的開場格式範例（`GmOpening.example`）也套（當最舊一則）：ST 裡開場在歷史裡同樣被剝；格式由卡的世界書與介面教，不靠範例裡的狀態欄。
- 世界書掃描、巨集、事件本身、lane 水位與指紋都不動（只動送出副本）。量測（scene_budget）與實送走同一份。

## raw／text 決定
GM 歷史照送 text：text 已由 app 剝掉卡片狀態標籤與 `<UpdateVariable>`，改送 raw 會讓所有卡多送 app 已接管的區塊；promptOnly 套在送出的那份上（平常是 text，本幕開場事件照前案送 raw）。lane 已在 session 裡的 GM 回覆維持 CLI 原紀錄，不改寫（要動 session 檔與鎖，另案）。

## 已知限制
- 只支援替換成空字串、無 trimStrings、無巨集替換的 promptOnly 腳本；其餘略過不套。
- `/foo/gg`、`/foo/q` 這類不合法旗標寫法，網頁版退成整串裸樣式、後端直接略過（實卡不含）〔模型判斷·未裁決〕。
- CLI 續聊線 session 內 GM 自己的舊回覆維持原文（含卡片狀態欄、UpdateVariable），不改寫；重開線時才以處理後的版本重建。
- 依深度的腳本（minDepth）在續聊線只作用於當輪新送的事件；已送進 session 的不會回頭剝，重開線才生效。
- 多卡桌合併所有在場卡的 promptOnly 腳本（ST 只套當前角色的局部腳本）。
- 封存或自動隱藏的卡不提供腳本（沿用 `read_card_interfaces` 的在場篩選）；只有 promptOnly、沒有顯示腳本的單卡桌換幕後主卡若被收進隱藏區，腳本跟著不套。

## 測試
- 三個實際樣式各一：MVU（閉合／未閉合）、NorthHall（只剝第一個、`.*?` 不跨行）、RPGImmortal（minDepth 2 保留最近兩則）。
- GM 開場 raw 送出時剝掉 `<Status_block>`；換幕範例同。
- D24：允許＝剝、不允許＝不剝（改寫 web_save 既有測試）。
- 世界書掃描不受影響靠構造保證（只動送出副本，掃描吃原事件），不另寫測試。

## 觀察
- greeting-context-st-parity 方案檔「promptOnly 剝除卡會多送」那條已知限制，本案合併後過時（由主線處理）。
