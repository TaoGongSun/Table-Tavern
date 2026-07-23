# Task handoff
Task-ID: sample-world-i18n
Updated: 2026-07-24T00:05:00+08:00
Status: in-progress

## Goal
範例桌內容依語系產生。首開順序 2026-07-23 使用者拍板：先跳語言選擇畫面（下拉、預選系統語系），選完寫入 config 再建對應語言的範例桌。

## Current state
前後端程式碼完成（主線 Fable 5 直做）：create_sample_world 吃 lang 參數產 zh-TW／en 內容；首開（無桌且 config 沒有 language 偏好）先渲染 FirstRun 語言選擇畫面。cargo test 41 綠、npm build 綠。剩使用者模擬首開實測。

## Completed
- data.rs `create_sample_world(root, lang)`：en 時桌名 "The Misty Tavern (sample)"、world.md／三張角色卡（Fox／Knight／Bard）／開場旁白全英文；非 en 一律走原中文內容；顏色／頭像／檔位兩語系共用一份 style 表
- lib.rs command 加 `lang: String` 參數
- App.tsx：
  - `FirstRun` 元件：標題＋一句說明＋語言下拉（預選 navigator.language 是 zh 開頭→zh-TW，否則 en；選單即選即換介面語言）＋「開始」鈕
  - 啟動流程：無桌且 `preferences.language === undefined` → 顯示 FirstRun；`startFirstRun(lang)` 寫 config → `create_sample_world({ lang })` → 進桌。無桌但語言已設（例如手動刪桌的老使用者）→ 直接建該語系範例桌不再問
- i18n：firstRunTitle／firstRunIntro／firstRunStart（zh＋en）

## Verification
- `cargo test`：**41 passed; 0 failed**（新增 data::tests::sample_world_english_content_follows_lang：en 桌名＋Fox/Knight/Bard＋world.md 含 Mistmouth＋開場旁白英文；原 sample_world_is_ready_to_play 改帶 "zh-TW" 全過）
- `npm run build`：rc=0

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 使用者實測首開：備份後清掉 `~/Library/Application Support/…/worlds` 目錄，並把 config.json 的 `preferences.language` 鍵整個刪掉（留著就不會觸發）→ 開 App 應見語言選擇畫面 → 選 English → 應建英文範例桌直接進桌；再重複一次選繁中驗證中文桌

## Next action
- 使用者實測通過即結案

## Constraints
- 只影響新建的範例桌；已存在的桌不回頭改；後端錯誤訊息 i18n 不在範圍；更多介面語言另立 i18n-more-languages
