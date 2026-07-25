# Handoff: ui-overhaul

## Current state
Emblem 設計系統第一輪實作完成：App.css 全面重寫成 token 制（深淺兩主題），App.tsx 側欄／對話區／composer 結構調整，en 按鈕文案 title case。npm build（tsc＋vite）綠；本機 dev 實跑視覺驗證通過（淺色主題）。深色模式與實聊 playbill 待使用者實機驗收。

## Completed
- **App.css 全面重寫**：設計 token 在 `:root`（淺色）＋`prefers-color-scheme: dark` 覆寫（深色＝定案值）。含 `--surface-0..3`、`--ink-1..3`、`--accent`、`--amber`（機密專用）、`--player`、`--control-h`（控制項統一高）、`--font-ui`／`--font-story`。舊版 `outline: none` 無障礙缺陷改為全域 `:focus-visible` 焦點圈。
- **側欄**：GM 卡壓成一行 `gm-row`（🎲 GM ⚙，整列點擊開世界設定，行為不變）；角色卡改桌遊組件卡 `tcard`（左圖窗角色色染底＋名字 wedge `tcard-plate`＋檔位寶石 `tcard-gem`，tier=default 不掛寶石；被選發言對象＝角色色描邊 `tcard-selected`；✎ 編輯鈕保留）。
- **對話區 playbill**：dialogue／player 事件改「發言名牌（wedge＋同色細線）＋散文」左對齊版式；narration 斜體 serif；system 小字 sans；訊息列表頂部加幕書籤 `act-divider`（顯示既有 `sceneDisplayLabel(scene)`，同一套換幕／前幕資料）。玩家發言名牌用新 i18n 鍵 `playerLabel`（玩家／You）。
- **Composer**：改直欄整寬書寫面——目標晶片（`opt-target`，顯示既有「發言對象」狀態，非新功能）＋整寬 serif 輸入框（打字所見＝故事字樣）＋按鈕列（請 X 發言／GM 旁白／GM 推進靠左、實色「送出 ➤」靠右）。所有 disabled 邏輯原樣。
- **世界書可見範圍徽章**：加 `worldbook-badge-visibility` 虛線琥珀（機密記號統一）。
- **i18n**：en 按鈕文案 Title Case（New Act、GM Narration、GM Advance、Export Transcript、Install with One Click 等 23 鍵）；新增 `playerLabel`（zh 玩家／en You）。zh-TW 文案未動。
- **刪除**：`Avatar` 元件（三個使用點全改版後無人引用）；舊 message-bubble／character-card 樣式。
- 合作者 pull 進來的 `cli-install-progress` 樣式已 token 化保留（App.css 尾段）。

## Verification
- `npm run build`：tsc＋vite 綠，無型別錯誤（輸出 dist/assets/index-*.css 14.80 kB）。
- 本機 `npm run tauri dev` 實跑截圖驗證（淺色）：角色卡 wedge＋寶石＋選中描邊、GM 一行列、桌名 wedge、幕書籤「第 1 幕」（開空桌驗證後已由 reclaim_world_if_empty 自動回收）、旁白 serif 斜體、composer 目標晶片＋整寬書寫面＋主要鈕「送出 ➤」、設定視窗兩分頁、世界設定編輯頁——皆如定案稿。
- 對比數據（定案稿量測，token 同值）：正文 12.9:1、旁白 9.7:1、按鈕 8.9:1、機密記號 9.3:1，全過 AA。
- 不發明 UI 查證：Say 模式／訊息內機密徽章／epithet 副標剔除；tier（CharacterMeta.tier）、發言對象（speaker state）、幕結構（advance_scene／pastScenes／ActReader）皆為既有功能（App.tsx:9-38、1343-1362）。

- **回饋修訂輪 1（2026-07-25）**：
  - 閱讀行寬：`.messages`／`.composer`／`.onboarding` 上限 42rem 靠左（推翻 2026-07-24 全寬拍板，改版設計輪重新拍板）。
  - 側欄卡片截字修正：`tcard-body` 右側 padding 2.1rem 留位給寶石與編輯鈕，名牌不再壓到絕對定位元素下。
  - 字級全面 token 化：六階 type scale（--fs-story/.96、--fs-body/.9、--fs-ui/.82、--fs-btn/.74、--fs-meta/.72、--fs-gem/.56，另 --fs-title/.85 桌名專用），App.css 內零散 font-size 清零（grep 驗證無 0.x rem 殘留）。

- **回饋修訂輪 2（2026-07-25）**：選中描邊被捲動容器裁掉→`.character-list` padding 3px/margin -3px 留邊；故事欄改整欄置中（`margin: auto`，欄內仍靠左）——使用者拍板「置中欄、左對齊內容」。

- **回饋修訂輪 3（2026-07-25）**：深色模式改 app 內建切換不跟系統——`config.preferences.theme` 寫在 `<html data-theme>`，**預設 dark**（:root 即深色 token，`:root[data-theme="light"]` 覆寫淺色；prefers-color-scheme 全移除）。設定 → 外觀新增「主題」下拉（i18n：themeLabel/themeDark/themeLight）。實測切 Light→寫檔生效→改回預設 dark。過程遇 WebView2 在螢幕解析度切換＋原生下拉開啟時凍結一次（跟 app 邏輯無關，殺程序重啟即復原）；另外多開 app 實例會共用 config 造成偏好互踩——期間 config 的 language/text_size 一度被舊實例覆寫掉，已還原（en／l）。

## Remaining
- ~~深色模式實機驗收~~ → 已完成（app 內建切換後深淺兩向實測通過，深色為預設）。
- 實聊驗收 playbill：dialogue 事件要有 API key 實聊才會出現；串流中打字指示已改 playbill 版式但未實測串流。
- 淺色主題是深色定案的推導稿，未經設計輪；使用者若覺得不對再開一輪。
- `.container`／first-run 首開畫面只粗略吃到新 token，未特別設計。

## Next action
使用者實機驗收（深色模式＋實聊 playbill＋串流打字指示）；回饋修訂後結案。驗收前不要再動 App.css 的 token 名——release-4 主題引擎會以這套 token 為介面。

## Constraints
- 不發明 UI：新視覺元素必須對應既有功能或狀態，先查 App.tsx／後端指令。
- 三條骨架規則不得稀釋：wedge＝名字、虛線琥珀＝機密、serif＝故事文字。
- en 按鈕 Title Case；zh-TW 不動；角色名原樣不轉大寫。
