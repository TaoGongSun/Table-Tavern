# Handoff: player-card（已結案 2026-07-28）

## Current state
玩家角色卡實作完成並結案。使用者實機建了玩家卡「布奇」（含上傳大圖），側欄與編輯畫面逐項調整到滿意。

## Completed
- **存檔格式**：玩家卡就是一張普通 `CharacterCard`，存該桌 `characters/<id>.md`，卡的 id 記在 `state.json` 新欄位 `player_card_id`。頭像／上傳圖／大圖窗／AI 生圖／圖庫全部沿用角色卡既有那一套，**未新增任何 tauri command**。
- **排除機制**：`list_characters` 讀 state 後濾掉玩家卡，側欄角色清單、GM 的「登場角色」、GM 點名 roster 一次全乾淨（三處都走這條）。
- **提示詞**：`assemble_messages` 與 `assemble_gm_messages` 各加 `player: Option<&CharacterCard>`，有玩家卡才注入「## 同桌的玩家」／「## 玩家角色」段；沒卡時 system 逐字與改動前相同（有測試守住）。
- **點名哨兵**：`suggest_instruction` 有玩家名字時改說「若現在應該輪到玩家（名字）行動」，`pick_speaker` 把玩家名字也列入候選並映射回 `PLAYER_SENTINEL`（「玩家」）；前端 `App.tsx` 的 `if (characterId === "玩家") break;` 不動。
- **側欄**：GM 卡正下方固定一張玩家卡，與 GM 共用皮革＋燙金內框樣式（CSS 選擇器共用，不複製規則）。沒建卡時是同一張皮壓暗＋虛線內框的空位卡「＋ 你的角色卡」，點擊建立——這是設計拍板外新增的必要入口（沒有它玩家永遠建不了第一張卡）。
- **編輯器**：沿用 `CardEditor`，加 `isPlayer` prop 隱藏私有設定欄、檔位、封存鈕，公開欄標題改「別人眼中的你（社會身份、外表、風評）」；存檔時 `private_md` 一律空。第一次存檔才把 id 寫進 `state.player_card_id`，刪卡後清回 null。
- **逐字稿**：玩家發言的 `speaker_name` 有卡就用玩家名字，沒卡維持 `t("playerLabel")`。改名後舊事件保留舊名（既有拍板行為）。
- i18n zh／en 各補 7 鍵（含玩家版的名稱標籤與提示字）。

### 實機看過後的 UI 調整（2026-07-28，使用者逐項拍板）

- 玩家卡名稱欄改玩家口吻：「你的角色名稱／你在這桌叫什麼名字」（NPC 卡不動）。
- 玩家卡皮革混白 18%（`--leather-base` 抽出來，玩家卡在其上調亮），七個主題自動跟著自己的 `--gm-leather` 走。
- composer 送出鈕移到最左、三顆 AI 動作鈕移到右邊：送出在右下容易誤按成「請某某發言」。
- 卡片編輯畫面頂部切兩塊：左上是這張卡的動作（儲存／隱藏／刪除一列，返回獨立第二列），右邊是圖片與五顆圖片鈕、頂端與按鈕列齊高；打字欄位維持全寬在下方，窄視窗自動疊回上下。分界定在 40%（左窄右寬），圖片與按鈕從分界靠左排，圖落在畫面中央。
- 有圖可顯示（有頭像、或有大圖且顯示開關開著）時隱藏 emoji 圖示欄；關掉顯示開關又沒頭像時那一欄自己回來。

## Verification
- `cargo test` → `test result: ok. 106 passed; 0 failed`（基線 105，+1 正向測試 `player_card_enters_character_and_gm_context`；另有沒卡時逐字不變、pick_speaker 玩家名字映射回哨兵、list_characters 排除玩家卡三項）
- `cargo clippy --all-targets` → 5 個既有警告（`data.rs:47`、`import.rs:219/227`、`lib.rs:499/668`），本次未新增
- `npm run build` → tsc 綠
- 檔案：`src-tauri/src/data.rs:189,1127,1207`、`src-tauri/src/transport.rs:80,150,290,306`、`src-tauri/src/lib.rs:910,933,1008`、`src/App.tsx:2302,2344,2634,2680,3005,3253`、`src/i18n.ts:166`、`src/App.css:519`
- 實機（使用者截圖佐證）：玩家卡「布奇」建立成功、側欄掛在 GM 卡正下方且皮革色與 GM 可分辨、玩家卡編輯畫面欄位與版面逐項調整到滿意。
- **未見實機紀錄的一項**：GM 點名輪到玩家、而玩家已有名字時是否正確停下。程式面由 `pick_speaker` 單元測試涵蓋，實機路徑未驗；若哪天 GM 點到玩家卻沒停下來，先查這裡。

## Remaining（結案後的已知小事，非阻塞）
- 玩家卡與某 NPC 撞名時，`pick_speaker` 會判給 NPC（roster 先比對）。極少見，暫不處理。
- 編輯畫面「儲存角色卡」「請先填角色名稱」等文案與 NPC 卡共用，玩家卡下讀起來略生硬。
