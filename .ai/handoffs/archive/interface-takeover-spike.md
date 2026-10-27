> 結案 2026-10-27：介面桌只走 GM 敘事線已進 main，Sol／Grok 驗收無必改。剩已存在的 lane 升級後續聊未實機驗（排實測佇列梯 2）。

# interface-takeover-spike 交接（2026-10-27）

## 對話約束（使用者當日裁決，新對話必守）
- 禁長考：判斷題直答。
- 實測可花額度，照測項挑模型（重構高階、一般回合低階）〔作者裁決 2026-10-11〕；露骨成人卡不拿來讓模型生成內容。
- 查卡資料一律讀 `TestCards/` 原卡（PNG tEXt chara chunk），不查桌目錄。

## 現況
介面接管已成立並進 main（西幻資料槽型、狀態欄型、整頁型、MVU 型都驗過，見 [plans/interface-takeover-card-types.md](../../plans/interface-takeover-card-types.md)；舊產殼路線清理見 [plans/interface-shell-cleanup.md](../../plans/interface-shell-cleanup.md)）。

## 本輪：介面桌 NPC 開口蓋掉旁白（卡片介面接管段順序①）
- 問題：角色一開口，`pickCardShell` 拿角色樓試卡腳本、不中就用骨架填 `{{本回合.正文}}`；角色樓不帶 patch，介面被角色台詞蓋掉。
- 做法：介面桌停用角色點名，只走 GM 一條敘事線；不做回合聚合正文，介面桌失去多角色開口。〔作者裁決 2026-10-27〕
- 定案做法、驗收結果與沒實機驗過的項目：[plans/interface-takeover-gm-only.md](../../plans/interface-takeover-gm-only.md)。

## 拍板規格
- **AI 永不產介面**：重構只把卡的每回合輸出格式抄成骨架，變動處挖 `{{狀態樹路徑}}`，正文槽固定 `{{本回合.正文}}`。
- **卡原文的規定分三類**（這是整案最關鍵的教訓）：①值格式（條目寫法、數量、白名單）照搬進 GUIDE，殼靠它渲染；②傳輸容器（「每次回复必须且只能输出一个XML数据块」「必须依次输出五大模块」）一律丟掉，那是 ST 不保管狀態的產物，抄進來 GM 就會多印一份廢資料；③固定資產（地圖矩陣、白名單表）照抄成骨架固定文字，不做欄位。
- **更新指令逐卡產**，容器用 app 的 `<UpdateVariable>` JSONPatch（GM 實測會照做，值格式仍照卡原文）。
- **「所有欄位每回合重報」不照抄**：依性質分兩組——跟著劇情走的（時間／地點／可用物品／技能／推薦行動）每回合必報，世界層面慢變的（委託／傳聞／地點／積分／裝備／基本資料／筆記）變動才報。
- **兩份原卡語意**：`source-card.png` 原封留桌上＝介面唯一來源；重構只動世界書／角色／GM 提示詞。
- **拆角色 vs 保留介面交給玩家選擇**（2026-08-12 使用者拍板）：整頁介面的卡拆出角色後，NPC 一開口就掉出介面（一格 Story 槽只能一個敘事者）。不由 app 自動判斷——玩家對一張卡有興趣才會抓下來玩，他知道自己要什麼。已做成 [refactor-mode-split](refactor-mode-split.md)。

## 陷阱備忘
- 這台機器 tauri dev 冷啟不重編 Rust；驗證法＝比 `target/debug/table-tavern` 與 `.rs` 的 mtime，或 `grep -a` 新字串在不在 binary 裡。
- 重現面板不必開 app：`npx tsx` import `src/features/refactor/refactor-shell.ts`／`interface-card.ts`，餵真骨架＋真樹＋原卡 `regex_scripts`，`buildShellDocument` 落檔後起 http server 用 Browser 讀 console。**要記得填 `本回合.正文`**——不填就重現不出「正文內容劫持卡 regex」那類 bug（第二輪地圖全毀就是這個：GM 在正文重印的 XML 讓 `findRegex` 抓到內嵌的 `</CurrentView>`）。
- 監聽桌目錄變化很有效（`refactor-outcome.json`／`state.json`／`interface-shell.html`／`transcript/*.jsonl` 各報一行摘要），實測時免問使用者要證物。
- zsh glob 無命中會炸腳本（`setopt null_glob`）；`grep` 的 `\|` 交替在這台的 ugrep 會炸，用 `-F -e`。
