> 結案 2026-10-04：三塊實作與大綱可編輯、三項回饋皆完成；代測抓到的缺陷由 [ai-table-generator-fixes](ai-table-generator-fixes.md) 修完，測試通道六項全過，Sol 驗收無必改。

# 一句話開桌（ai-table-generator）

## 結論
- 後端 `src-tauri/src/genesis.rs`＋`src-tauri/src/commands/genesis.rs`：三指令 generate_table_outline／generate_table_character／generate_table_expand。提示用英文標記 `## WORLD:`／`## CHARACTER:`／`## OPENING`，內容跟介面語言走；容錯解析（標記 0–6 個 #、大小寫不拘、半全形冒號、缺 EMOJI→🎭、缺 PRIVATE→空、缺 OPENING 照樣成桌）；先解析成功才動磁碟，不留半套桌；落桌走 create_sample_world 同路徑，重名補「 2」「 3」，桌名照玩家草稿標題。
- 前端 `src/features/lobby/GenerateTableDialog.tsx`：一句話＋六顆題材 chips、額度小字；大綱可編輯（標題、摘要、角色列增刪、AI 生成角色）；點視窗外不關、生成中鎖關閉與動作；失敗走共用 `AiErrorText` 人話並遮 user_id，解析失敗顯示模型原文供重骰。
- 規格要點：角色數由模型自判、不錨定數字；免費功能，額度明示用玩家自己的。i18n gen* 鍵十語系齊。

## 驗收
2026-10-04 測試通道（OpenRouter dots-studio/dots-3-note-preview:free）六項全過：①開窗、點外不關、× 關　②生成大綱　③改大綱各欄與手寫角色　④AI 生成角色　⑤照大綱開桌（桌名＝草稿標題、刪掉的角色不在、emoji／公開／私密、GM 開場白）　⑥單人題材只生一個角色。重骰與介面切 en 為同日前一輪代測已過，修正輪相關程式未改、未重跑。
