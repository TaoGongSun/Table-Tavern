# 重構雙軌定向：介面優先 vs 角色優先（兩段式選擇）

本檔存放 [refactor-mode-split](../tasks/refactor-mode-split.md) 的規格細節，由任務檔 Summary 連回。2026-08-13 立案；2026-08-14 使用者五項拍板＋Sol 第 1 輪覆核結論併入（逐字稿在 Codex app 的 companion 串；三條關鍵技術主張經主線抽驗屬實）；同日使用者拍板文案稿與初判失敗預設，全案無待拍板。

## 問題定調（2026-08-13 使用者）

卡片介面與拆出角色卡本質衝突：拆出的角色自由對話不帶 patch、一開口就掉出介面（一格正文槽只有一個敘事者），也不符合卡作者的預期玩法。兩件事不能同時要，重構必須二選一：

| | 介面優先（保介面） | 角色優先（拆角色） |
|---|---|---|
| 玩法 | 與原卡完全相同：純介面內操作、格式不崩、數值跟動 | app 多角色卡對話模式接手 |
| 角色 | 不拆。人物條目照搬進世界書（判官 PERSONS 留空），GM 代言 NPC；混寫條目走需拆，人物設定不丟 | 走認人／升格管線拆成角色卡 |
| 介面 | 走接管軌：骨架＋狀態樹＋逐卡 RULES/GUIDE，AI 每輪只出正文＋少量 patch | 產物一律不建不顯示，**含 NorthHall 型逐訊息狀態欄**（2026-08-14 拍板）；混在介面條目裡的設定／機制仍拆出保留，不整條陪葬 |
| 代表卡 | WestFantsy（已驗證）、Transfur | NorthHall |

優先項目＝介面優先軌的「玩法與原卡完全相同」。

**模式必須持久化**（Sol 覆核抓到的主洞，抽驗確認）：現行 `useCardInterfaceController` 在無重構殼時會退回「近 10 則掃 event.raw」路徑，用原卡 regex 照樣渲染介面——角色優先光是「不建產物」擋不住顯示。模式要寫進重構產物與桌面狀態，角色優先明確停用卡片介面 fallback；匯出／匯入一併保存 mode。機械稽核也要 mode-aware：「依模式不生成」不得誤報成漏網。

## 既有地基（已驗證或已拍板）

- 介面接管在西幻卡三回合驗證通過：每回合 5944→2670 字（省 55%）、劇情 ×2.8、零拒收（commit d3f8a7e）。規格＝AI 永不產介面、骨架挖 `{{狀態樹路徑}}`、正文槽 `{{本回合.正文}}`、卡規定分三類（值格式照搬／傳輸容器丟棄／固定資產抄成固定文字）。
- 2026-08-12 已拍板：拆角色 vs 保留介面由玩家決定，app 不自動判斷。本案是這條拍板的具體化。
- 盤點判官架構見 [refactor-survey-spans](refactor-survey-spans.md)：判官讀全卡出小抄（四分類＋分組＋命名權威）。

## 定向流程（2026-08-14 拍板＋Sol 補強）

1. **本機前置＝三態偵測**：`none`（無介面）／`supported`（可接管的介面）／`unsupported`（TrainEmperor 型雲端載入器，擋下、不進二選一）。介面腳本存在與否本機判得準；人物相關一律不用本機猜（ST 卡的人物資料各作者亂塞）。
2. **none：不問，直接跑角色線完整第二段**（不出初判）。這是唯一免問的路。
3. **supported：一律詢問玩家**，兩段式判官：
   - **第一段（初判）**：判官帶全卡，只輸出兩行——`RECOMMEND: interface|characters`＋`EVIDENCE: 一句人話證據`。不產小抄。
   - **第二段（整理）**：玩家選定後同一條串續跑，帶模式專屬提示詞出 survey-spans 完整小抄，**外加 `MODE` 回聲**，app 核對與玩家選擇一致才收。
   - 第二段帶 run id＋卡片內容指紋：取消、重跑或卡片異動後，舊第一段作廢不得續用。
4. **反悔路**：選錯就取消、重按重構鈕重跑（原卡 PNG 留檔）；不做「改用另一軌」入口。

### 兩段呼叫的實作契約

- 現行重構呼叫是無狀態單發（`stream_via_transport`，抽驗確認）；兩段判官要建**獨立短命的 refactor session**——沿用 resume 續聊機制，但不借遊玩 GM lane，免污染桌上續聊的快取與上下文。
- **快取前提（拍板 3 的成本依據）**：兩段同檔位同 lane（快取以模型為界）；卡片資料排共用前綴、斷點下在共用段結尾，命中時第二段卡片部分約 0.1 倍價，整套只比單段判官多約三分之一次讀卡。「小呼叫」指快速判斷（輸出小），不指小模型（2026-08-14 拍板）。
- 預設快取壽命 5 分鐘：玩家考慮逾時＝第二段回原價（沒省到而已），可接受、不做保溫。
- 非 Claude lane 或 resume 失敗：降級成第二段重送全卡——只損省費，不改語意。

## 二選一對話框（2026-08-14 拍板定稿）

- 標題：「這張卡要保留哪種玩法？」
- 判官句（EVIDENCE 直出）：「建議保留原卡玩法：這張卡有完整遊戲介面。」／「建議改成多角色對話：卡內有 8 位帶完整設定的人物。」
- 選項（後果式）：
  - 保留原卡玩法——玩法跟原卡一樣；人物不會出現在角色清單。
  - 改成多角色對話——卡片介面會消失；改用本 app 的多角色對話。
- 預選建議項；主按鈕「照建議繼續」、次按鈕「自己選」。
- 初判呼叫失敗時：不偽造證據，仍顯示兩選項，預設介面優先。

## 開工（兩分鐘程序；2026-08-14 裁決：主線禁再推規格、禁重讀 codebase）

使用者說「開工」＝立刻發包。實作細節（檔案落點、簽名、UI 細節）由執行者自己想，主線只發包＋收貨。

1. `handoff checkpoint`（skill 硬性要求）。
2. 發包（執行者由使用者當下點名，外部背景程序）：
   `cd <repo> && env -u ANTHROPIC_BASE_URL claude -p --model <點名檔位> --dangerously-skip-permissions "<指令>" > /tmp/pkgN.log 2>&1 &`
   指令固定一句：「整份讀 .ai/plans/refactor-mode-split.md 與 .ai/tasks/refactor-mode-split.md，實作分包 N（實作定案段為準），自驗 cargo test／npm test／npm run build／npm run check:i18n 全綠後回報改了哪些檔。」
3. 收貨：主線重跑四件套＋逐條對驗收→commit `refactor-mode-split: 包N …`→發下一包。包 3 發包前主線出判官提示詞正文（拍板保留項）；包 4 實跑歸使用者。

環境陷阱（2026-08-14 實測）：`--dangerously-skip-permissions` 被會話 auto-mode classifier 擋——開工前使用者先在 settings 加 Bash 允許規則，或當場核可該指令。

## 分包與排序（2026-08-14 開工定稿：降級路先立，session 後補）

1. **包 1＝路由＋三態偵測＋二選一 UI＋單發版兩段**：`refactor_recommend` 先以無狀態單發落地（單發版本身就是日後的降級路）；`refactor_survey` 加 `mode` 參數透傳；characters 模式 pool 過濾（interface／statusbar 任務不發）。
2. **包 2＝session 升級**：兩段改 Open／Resume 同一短命 session＋run id／指紋＋resume 失敗退回包 1 的單發路。
3. **包 3＝模式行為**：模式專屬提示詞正文＋MODE 回聲核對＋mode-aware 稽核（rule 5）＋持久化（WorldState）＋介面 fallback 抑制＋匯出入 mode。
4. **包 4＝穩定性驗收矩陣**（實跑歸使用者）。

## 實作定案（2026-08-14 主線親查 codebase 後拍定，發包依此）

- **三態判定**（前端純函式，素材＝`card_interfaces` 回傳的 `CardInterface[]`）：任一卡 `unsupported===null && scripts.length>0`→`supported`；否則任一卡 `unsupported!==null`→`unsupported`（擋下顯示訊息，不跑重構）；否則→`none`（mode=characters 直跑 survey，無初判）。
- **兩段呼叫**：兩段同用 `gm_tier`；system＝現行 `system_message(context)` 逐位元組共用（快取前綴）。第一段輸出 `RECOMMEND:`＋`EVIDENCE:` 兩行；transport≠claude 一律單發。指紋＝`usage_log::text_hash(context)`，第二段 Rust 端重算比對，不合→單發；指紋是 64-bit 雜湊等價、非逐位元組證明——正常 UI 等價成立，雜湊碰撞／異常 caller 理論可繞（2026-08-14 審查拍板：記錄即可，不加固）。session id 生成復用 `lanes::new_session_id`（pub 化）；不進 lanes.json、不保溫、不清 session 檔。
- **mode 欄位鏈**：`RefactorSurveyOutcome.mode`（包 1 由 app 填、包 3 改 AI `## MODE` 回聲核對，不一致＝Err）→`RefactorOutcome.mode: Option<String>`（serde default；`None`＝舊產物＝照 interface 行為）→套用時寫 `WorldState.refactor_mode`；匯出／匯入全程保留。mode 只收 interface／characters 二值：匯入端正規化大小寫與前後空白、未知值拒收，套用端 trim 後非法值不落地。
- **characters 模式行為**：interface／statusbar 呼叫不發；INTERFACE 條目與 statusbar spans 進 dropped **rule 5「依模式捨棄」**（可見、可放回成 carry 條目）；absorb／group／FIELDS 照舊（app 狀態樹與機制不受模式影響）；apply 時防禦性刪 interface-shell.html；涵蓋與機制守恆稽核把 rule 5 算「已處置」。
- **interface 模式行為**：判官 PERSONS 留空、`person name:` route 停用（提示詞禁用；仍出現時 app 以 `entry title:<人名>` 照搬兜底）；人物條目走 ENTRIES carry；MODE 回聲核過後後端再清空違規 PERSONS（`normalize_survey_for_mode`）——認領作廢、其餘 route／verdict 照原判，無下落者由涵蓋稽核與餘段兜底補 carry（內容全有下落，非全走照搬）。
- **fallback 抑制**：新 command `refactor_table_mode(world_id)->Option<String>` 讀 `WorldState.refactor_mode`；`useCardInterfaceController` 讀到 `"characters"` 時 `cardInterfaceShell` 恆 null（開介面鈕不出現、近 10 則掃 raw 的 fallback 不啟動）；tableMode 三態——undefined（載入中或讀失敗）同樣不顯殼，未知不放行；重構結果只剩 dropped／unabsorbed／audit 也開結果視窗（此類產物匯出後匯入端照收，round-trip 不拒），純介面卡選 characters 才有產物可套、mode 才落地；讀取端（`refactor_table_mode`）正規化舊版壞值——合法大小寫就地修正、未知值回 Err，controller 維持未知不 fallback。
- **二選一 UI**（自製 modal，比照結果面板）：第一層＝標題＋判官句（`建議…：{EVIDENCE 直出}`）＋主鈕「照建議繼續」＋次鈕「自己選」＋可 Esc／取消；按「自己選」展開兩張選項卡（radio 預選建議項、各附後果行）＋確認鈕。初判失敗＝不顯判官句、預選 interface。none 卡不彈框。
- **初判進度**：refactorProgress 顯示新鍵 `refactorProbing`；兩段皆走 `inflight::register` 可取消。

## 穩定性驗收（拍板 5）

- 同一張卡連跑三次皆產出可運行產物才算過；次數由使用者實測時視額度調整。
- 跨卡型矩陣：WestFantsy（資料槽）／bcd368（MVU 前端）／Transfur（整頁前端）／NorthHall（狀態欄＋真人物）／TrainEmperor（unsupported 該被擋）。

## 關聯任務

- [interface-takeover-spike](../tasks/interface-takeover-spike.md)：其待辦 1（玩家選擇）併入本案；待辦 2（逐型驗卡）、4（清舊路線）留原案。
- [refactor-survey-spans](../tasks/refactor-survey-spans.md)：判官流程拆兩段、提示詞帶模式，動工前先過該案 T1–T4 驗收。
- [person-promote](../tasks/person-promote.md)：角色優先軌的認人拆卡，不做第二套。
- [interface-scene-change](interface-scene-change.md)：介面優先軌的換幕配套，另案。
