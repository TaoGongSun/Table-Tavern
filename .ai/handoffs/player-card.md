# Handoff: player-card

## Current state
玩家角色卡前後端全部實作完成，`cargo test` 106 綠＋`npm run build` 綠，等使用者實機驗收（必測「GM 點名輪到玩家」在玩家有名字時仍正確停下）。

## Completed
- **存檔格式**：玩家卡就是一張普通 `CharacterCard`，存該桌 `characters/<id>.md`，卡的 id 記在 `state.json` 新欄位 `player_card_id`。頭像／上傳圖／大圖窗／AI 生圖／圖庫全部沿用角色卡既有那一套，**未新增任何 tauri command**。
- **排除機制**：`list_characters` 讀 state 後濾掉玩家卡，側欄角色清單、GM 的「登場角色」、GM 點名 roster 一次全乾淨（三處都走這條）。
- **提示詞**：`assemble_messages` 與 `assemble_gm_messages` 各加 `player: Option<&CharacterCard>`，有玩家卡才注入「## 同桌的玩家」／「## 玩家角色」段；沒卡時 system 逐字與改動前相同（有測試守住）。
- **點名哨兵**：`suggest_instruction` 有玩家名字時改說「若現在應該輪到玩家（名字）行動」，`pick_speaker` 把玩家名字也列入候選並映射回 `PLAYER_SENTINEL`（「玩家」）；前端 `App.tsx` 的 `if (characterId === "玩家") break;` 不動。
- **側欄**：GM 卡正下方固定一張玩家卡，與 GM 共用皮革＋燙金內框樣式（CSS 選擇器共用，不複製規則）。沒建卡時是同一張皮壓暗＋虛線內框的空位卡「＋ 你的角色卡」，點擊建立——這是設計拍板外新增的必要入口（沒有它玩家永遠建不了第一張卡）。
- **編輯器**：沿用 `CardEditor`，加 `isPlayer` prop 隱藏私有設定欄、檔位、封存鈕，公開欄標題改「別人眼中的你（社會身份、外表、風評）」；存檔時 `private_md` 一律空。第一次存檔才把 id 寫進 `state.player_card_id`，刪卡後清回 null。
- **逐字稿**：玩家發言的 `speaker_name` 有卡就用玩家名字，沒卡維持 `t("playerLabel")`。改名後舊事件保留舊名（既有拍板行為）。
- i18n zh／en 各補 5 鍵。

## Verification
- `cargo test` → `test result: ok. 106 passed; 0 failed`（基線 105，+1 主線加的正向測試 `player_card_enters_character_and_gm_context`；codex 另加沒卡時不變、pick_speaker 映射、list_characters 排除三項）
- `cargo clippy --all-targets` → 5 個既有警告（`data.rs:47`、`import.rs:219/227`、`lib.rs:499/668`），本次未新增
- `npm run build` → `✓ built in 459ms`，tsc 綠
- 檔案：`src-tauri/src/data.rs:189,1127,1207`、`src-tauri/src/transport.rs:80,150,290,306`、`src-tauri/src/lib.rs:910,933,1008`、`src/App.tsx:2302,2344,2634,2680,3005,3253`、`src/i18n.ts:166`、`src/App.css:519`
- **未做實機驗證**：Tauri 原生視窗無法在本對話截圖，GUI 行為全部交使用者驗收。

## Remaining
- 使用者實機驗收（下列為必測清單，見 Next action）。
- 玩家卡與某 NPC 撞名時，`pick_speaker` 會判給 NPC（roster 先比對）。極少見，暫不處理；若實測踩到再拍板。

## Next action
使用者實機驗收，重點四項：
1. 沒建玩家卡的舊桌：說話、GM 旁白、GM 點名全部與改動前一模一樣。
2. 建一張玩家卡（含上傳頭像與大圖）→ 側欄 GM 卡下方出現、不能拖曳、不會變成發言對象。
3. 跟 NPC 對話：NPC 應該叫得出玩家名字、認得那段社會身份。
4. **GM 點名輪到玩家時仍正確停下**（玩家有名字後 GM 可能改喊名字，已做映射，這是最大陷阱）。

驗收過了就結案：任務行搬 `DONE.md`、本檔進 `handoffs/archive/`。
