# interface-takeover-spike 交接（2026-08-12 收工）

## 對話約束（使用者當日裁決，新對話必守）
- 禁長考：判斷題直答。禁自行測試燒額度：使用者按按鈕實測，本機編譯／cargo／vitest 可跑（零額度）。
- 查卡資料一律讀 `TestCards/` 原卡（PNG tEXt chara chunk），不查桌目錄。

## 結論：介面接管成立，軌不取消
西幻卡（`TestCards/WestFantsy.png`）三回合實測，任務檔三項驗收全過：

| | 原卡直玩 | 接管後第 1 回合 | 第 2 回合起 |
|---|---|---|---|
| 每回合輸出 | 5944 字 | 4285 | **2670／2523** |
| 其中劇情 | ~460 | 736 | **1270／1136** |
| patch 條數 | — | 16（初始化） | **7／6** |

畫面與原卡無差別（地圖 11×7 完整、區域點擊互動、五個分頁欄位都有值）；狀態正確跟動（時間 6:20→6:47→7:08、道具隨劇情長出「巨獸溫熱心臟（父王所賜信物）」）；零拒收。省 55% token，同一次呼叫的劇情多 2.8 倍。

## 拍板規格
- **AI 永不產介面**：重構只把卡的每回合輸出格式抄成骨架，變動處挖 `{{狀態樹路徑}}`，正文槽固定 `{{本回合.正文}}`。
- **卡原文的規定分三類**（這是整案最關鍵的教訓）：①值格式（條目寫法、數量、白名單）照搬進 GUIDE，殼靠它渲染；②傳輸容器（「每次回复必须且只能输出一个XML数据块」「必须依次输出五大模块」）一律丟掉，那是 ST 不保管狀態的產物，抄進來 GM 就會多印一份廢資料；③固定資產（地圖矩陣、白名單表）照抄成骨架固定文字，不做欄位。
- **更新指令逐卡產**，容器用 app 的 `<UpdateVariable>` JSONPatch（GM 實測會照做，值格式仍照卡原文）。
- **「所有欄位每回合重報」不照抄**：依性質分兩組——跟著劇情走的（時間／地點／可用物品／技能／推薦行動）每回合必報，世界層面慢變的（委託／傳聞／地點／積分／裝備／基本資料／筆記）變動才報。
- **兩份原卡語意**：`source-card.png` 原封留桌上＝介面唯一來源；重構只動世界書／角色／GM 提示詞。
- **拆角色 vs 保留介面交給玩家選擇**（2026-08-12 使用者拍板）：整頁介面的卡拆出角色後，NPC 一開口就掉出介面（一格 Story 槽只能一個敘事者）。不由 app 自動判斷——玩家對一張卡有興趣才會抓下來玩，他知道自己要什麼。做法見下方待辦 1。

## 已完成變更（cargo 480／tsc／vitest 108／build 綠）
1. **狀態樹不再被收回沖掉**：[data.rs](../../src-tauri/src/data.rs) `sync_scene_state_tree`＋[refactor.rs](../../src-tauri/src/refactor.rs) apply 後呼叫——新樹補進這一幕每則事件快照的 `tree`／`jumps`。舊行為是「檯面恆等於最後一則事件快照」，重構改的樹不在任何快照裡，一次收回就換回舊欄位（實測踩過：面板全空白）。
2. **逐卡 update block**：`RefactorInterface` 加 `rules`／`guide`；apply 有殼時開 `incremental`、併 rules、落 guide。`Mechanism` 加 `guide`。[refactor-review.ts](../../src/refactor-review.ts) `parseInterface`／`merge` 要帶上兩欄——前端會重建 outcome 再傳回 Rust，不補就整個掉。
3. **展開契約四區塊**：[refactor_ai.rs](../../src-tauri/src/refactor_ai.rs) STATE／SHELL／RULES／GUIDE；`MECHANISM_SCHEMA` 拆成 `MECHANISM_FIELD_SCHEMA`／`MECHANISM_TRIGGER_SCHEMA`（接管只要欄位規則）；`INTERFACE_SHELL_RULES` 補固定資產處置；`INTERFACE_UPDATE_RULES` 限定照搬值格式＋分兩組；`strip_html_fence` 連語言標記一起剝。
4. **介面歸屬聲明**：[transport.rs](../../src-tauri/src/transport.rs) `interface_owned_notice`，接管桌才附，**壓在欄位說明之後**——模型會模仿最後讀到的排版，放前面它就照 guide 的 markdown 把狀態逐條寫進正文（實測踩過，那輪 patch 完全消失）。措辭要避開「資料區塊」這種會誤傷 `<UpdateVariable>` 的字。
5. 移除卡片介面的 ⓘ 說明鈕＋十語系 2 個 key（使用者要求）。

## 待辦（依序）
### 1. 拆角色 vs 保留介面交給玩家選擇
重構結果對話框已分組顯示「角色 N 位／介面 1 份」。介面被判 playable 時**預設只勾介面、角色那組不勾**，組標題掛一句「這張卡是完整遊戲介面，NPC 由 GM 代言」。資訊給足、玩家自己決定——不做自動判斷，因為「有幾位真人物」本機猜不準（分型腳本把西幻的地名、HeroTraining 的 `[Event]`／`[Script]` 條目都算成人物了）。

### 2. 其他型別的卡驗證（目前只有西幻型過關）
`TestCards/` 分型結果（腳本名稱是卡作者自己命名的，比啟發式分類可靠）：

| 型別 | 代表卡 | 特徵 |
|---|---|---|
| 資料槽型 ✅已過 | WestFantsy | `西幻`：捕 5 群＋`<script type="text/xml">` 資料槽 5 個。**全套卡裡只有這一張** |
| MVU 前端型 | **bcd368…**、HeroTrainingUnderSide | 帶 `[仅提示]变量更新中`／`[不发送]去除变量更新` ——狀態本來就走變數更新，協定同源，應最順，優先驗 |
| 整頁前端非 MVU | DongeonMaster(149KB)、Transfur(628KB)、RPGImmortal(327KB) | 整段替換成完整 HTML，不走資料槽 |
| 狀態欄＋真人物多 | **NorthHall** | `一体式美化状态栏`／`只美化状态栏`／`隐藏`（空）；世界書 8 位真人物各帶設定＋性格。預期結論是「拆角色、介面不接管」，要驗的是它會不會被誤判 playable |
| 雲端載入器 | TrainEmperor | 腳本全 0KB（`云端版`／`实时链接`），畫面靠外部載入，`CardInterface.unsupported` 該擋掉 |

分型腳本在 scratchpad（`classify-cards.py`，換 session 會清，重寫成本約十分鐘）。一型一張、四次驗證就覆蓋完。

### 3. 角色發言無視介面渲染（本質問題，使用者評估「很可能無法解決」）
拆出來的 NPC 一開口，`cardInterfaceShell` 的 direct-first 拿它的訊息試卡腳本、不中就用骨架填 `{{本回合.正文}}`，而 NPC 發言不帶 patch。整頁型卡靠待辦 1 迴避；狀態欄型卡（NorthHall）反而沒問題——那種介面跟著每則訊息走，角色發言各自帶狀態欄，走現有「近 10 則掃 `event.raw`」那條路就渲染得出來。

### 4. 舊產殼路線清理
HTML 殼分支、`INTERFACE_SHELL` 舊語意殘註解與多語系殘留。使用者原本明令實測通過才清——現在通過了，可以清。

## 陷阱備忘
- 這台機器 tauri dev 冷啟不重編 Rust；驗證法＝比 `target/debug/table-tavern` 與 `.rs` 的 mtime，或 `grep -a` 新字串在不在 binary 裡。
- 重現面板不必開 app：`npx tsx` import `src/refactor-shell.ts`／`interface-card.ts`，餵真骨架＋真樹＋原卡 `regex_scripts`，`buildShellDocument` 落檔後起 http server 用 Browser 讀 console。**要記得填 `本回合.正文`**——不填就重現不出「正文內容劫持卡 regex」那類 bug（第二輪地圖全毀就是這個：GM 在正文重印的 XML 讓 `findRegex` 抓到內嵌的 `</CurrentView>`）。
- 監聽桌目錄變化很有效（`refactor-outcome.json`／`state.json`／`interface-shell.html`／`transcript/*.jsonl` 各報一行摘要），實測時免問使用者要證物。
- zsh glob 無命中會炸腳本（`setopt null_glob`）；`grep` 的 `\|` 交替在這台的 ugrep 會炸，用 `-F -e`。
