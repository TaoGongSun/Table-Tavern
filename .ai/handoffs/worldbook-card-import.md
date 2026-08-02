# Handoff: worldbook-card-import

## Current state
匯入實作完成、自驗全綠；追加條目「就地展開編輯」UX 改版、「純世界書開局」（匯入成功自動選 GM＋零角色提示改寫）與「條目換編輯對象自動儲存」皆完成；2026-08-02 修掉實測踩到的資料遺失（匯完世界書的新桌被空桌回收整包刪掉）。等使用者實機驗收後結案。

## Completed
- 後端：`import::worldbook_json`（src-tauri/src/import.rs:407 起）——PNG 先解 chara chunk，角色卡剝到 `character_book` 層；`import_worldbook` 命令改收位元組（src-tauri/src/lib.rs:288）。
- 前端：匯入改傳位元組、選檔放寬為 .json/.png（src/App.tsx）；刪除已無人用的 `worldbookReadError` 字串（十個語系檔）。
- 測試：`worldbook_json_unwraps_lorebook_cards`（import.rs 測試模組末）蓋 PNG 卡／JSON 卡／一般世界書 JSON 三路，含 keys=null 常駐條目。
- UX 改版（2026-07-30 使用者回饋：表單固定頂端，點下方條目的編輯看不出反應）：條目編輯改就地展開——編輯取代該列、新增在清單底部展開並自動捲到可見，表單抽成 `entryForm` 共用，儲存／取消鈕移到表單頂端（src/App.tsx WorldEditor）。
- 刪輸出長度限制（2026-07-30 使用者實測回報：世界書萬字進 GM、回覆只有百字）：根因是 `narrate_instruction` 寫死「簡短旁白、一到三句」（mvp-4 時代過場定位的遺留），為模型看到的最後一條指令，壓過世界書與 world.md 的長度要求。旁白改「篇幅不設限，依劇情需要自由發揮」，換場摘要的 300 字／200 words 上限一併刪除，測試同步更新（src-tauri/src/transport.rs）。換場標題「10 字內」保留（介面欄位，非故事輸出）。
- 解除配角與心理描寫的禁令（2026-07-30 使用者指出）：GM 的「不要替任何角色說話」把沒有角色卡的配角（路人、店主、反派）也禁掉了，世界上只有 GM 能演他們，結果旁白只剩環境描寫。改成「沒有角色卡的配角由你全權扮演，可自由出場、說話、行動；『登場角色』名單上的角色與玩家不要代言」（system prompt 與導演指示兩處）。角色卡那條「只輸出台詞與動作描寫」同型問題（排除內心戲），改成「台詞、動作與心理描寫，也可以寫他眼中所見的環境與感受」（transport.rs 開場＋lib.rs 收尾兩處）。
- 純世界書開局（2026-07-30 使用者回報：只匯世界書、沒角色就開不了局，但其實點 GM 卡即可玩；問題在介面沒指引）：世界書匯入成功（至少一條）且桌上零角色時自動把發言對象設成 GM（src/App.tsx:1209 `onImported` prop、:1398 觸發、:3557 App 端零角色才選 GM）；零角色輸入框提示由「先建立一個角色」改為「建立角色，或匯入世界書後找 GM 開局」（十個語系檔 `composerNoCharacter`，用詞對齊各語系既有的世界書／GM 譯名）。commit 7367c1e。

- 條目換編輯對象改自動儲存（2026-07-31 使用者回饋：一條條整理世界書時，每次切換都跳未儲存確認、要回去點儲存太擋路——1cc4ea7 補的那道確認反而變成阻力）：條目本來就是即時寫檔，切換編輯對象或離開世界設定時直接存起來就走。`saveEntry` 抽出不依賴表單事件的 `persistDraft`（src/App.tsx:1388），由 `openDraft`（:1340）與 `confirmLeave`（:1306）共用；存檔失敗不切走，錯誤留在清單訊息列。還沒存過的新條目維持確認（免得半成品留在清單上），表單「取消」鈕也維持確認；既有條目的改動不再計入未儲存提示（:1291）。commit 132a997。

- 空桌回收誤刪世界書（2026-08-02 使用者實測回報：新桌匯完世界書、桌名還沒改，點別桌整桌連世界書一起消失）：`reclaim_world_if_empty` 判空只看訊息／角色／world.md 三項，世界書就躺在同一個資料夾的 worldbook.json 裡，跟著 `remove_dir_all` 一起沒了。判空加上世界書條目數（src-tauri/src/data.rs:436），讀不出來（檔案損毀）一律當有內容保留，不因讀取失敗刪桌。前端桌名那道防線本來就對——改過名的桌直接跳過回收，不看空不空（src/App.tsx:2929），已驗。

## Verification
- `cargo test`：117 passed, 0 failed。
- 真卡煙霧（TestCards/b3d7fd3600ab58d3252e8b38340390c4.png，臨時測試已移除）：`real card imported 17 entries`，抽查條目標題「世界观」「app-求治者」等與 constant 旗標正確。
- `npm run build` exit 0（UX 改版後重跑仍綠）。
- 就地展開瀏覽器實測（vite＋暫時 Tauri stub，17 條假資料，已清）：點條目 10 編輯→表單原地取代該列；取消→17 列復原；新增→表單在清單底部展開且完整入視野（getBoundingClientRect fullyVisible: true）。
- 踩雷已修：新掛的 useEffect 一度放在元件早退 return 之後，觸發 Rules of Hooks 全黑畫面——已移到早退前；scrollIntoView 的 smooth 會被後續 render 打斷停在半路，改瞬間捲動。
- 純世界書開局：`npx tsc --noEmit` exit 0、`npm run check:i18n` 十語系全 OK。自動選 GM 的行為需 Tauri 後端（瀏覽器 vite 預覽 `invoke` 不存在、載不起來），留給實機驗收。
- 條目自動存：`npx tsc --noEmit` exit 0、`npm run check:i18n` 十語系全 OK（未動字串）。同樣需 Tauri 後端才點得到，留給實機驗收。
- 空桌回收：`cargo test` 151 passed, 0 failed；`reclaims_only_untouched_worlds` 補兩個回歸情境（匯了世界書的新桌不回收、worldbook.json 損毀的桌不回收）。`cargo clippy --all-targets` 新增行零警告（既有 7 個警告不在改動範圍）。

## Remaining / Next action
- 使用者實機：世界設定 → 世界書「匯入」選該 PNG → 確認 17 條入列；點下方條目「編輯」確認就地展開；「新增條目」確認底部展開並捲到可見。
- 使用者實機（條目自動存）：改條目 A 內容 → 直接點條目 B 的「編輯」，確認沒彈窗、清單下方顯示「條目已儲存」、A 的改動有留下；改到一半按「返回」也應自動存；「新增條目」填一半跳走仍會問。
- 使用者實機（純世界書開局）：開一張零角色新桌 → 確認輸入框提示是新文案 → 匯入世界書卡 → 回聊天畫面確認發言對象已是 GM、直接打字 GM 會旁白接話。回報後結案。
- 使用者實機（空桌回收修復，需重新打包）：開新桌 → 不改桌名直接匯世界書 → 點別桌 → 回頭確認那桌還在、世界書條目都在。已被舊版刪掉的桌救不回來（`remove_dir_all`，無回收桶）。
- 使用者實機（長度限制刪除＋配角／心理描寫解禁）：需重新打包（已裝的 0.2.0 仍是舊行為），跑同一張世界書卡確認 GM 旁白篇幅放開、配角會開口說話、角色回覆有內心戲。

## Constraints
- 匯入併進當前開啟的桌，不自動開新桌（2026-07-30 與使用者確認現狀即此，如要「一鍵成新桌」另開任務）。
