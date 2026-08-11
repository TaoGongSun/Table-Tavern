# Handoff: refactor-survey-spans

## Current state
2026-08-11 五包全部實作完成、主線逐包複驗全綠（cargo **470**／vitest **108**／build 0／check:i18n 十語系 0）；**尚未實機驗收**——實作對話 context 過大，驗收在新對話照下方「實測清單」跑，全過與 refactor-dispatch 一起結案。

## Completed
- **包5**（sonnet 代理＋主線複驗，vitest 105→108，隨包 5 commit）：結果面板三新區——已淘汰（收合、逐項 rule 標籤＋展開全文＋「放回」走 restoreDropped 純函式轉 carry 進 entries 並勾選，App.tsx:2948 起）、未接管機制清單、稽核紅字；i18n 十語系各 +12 鍵。
- **包4**（sonnet 代理＋主線複驗，vitest 94→105、build 0，commit bded948）：runAiRefactor 整段重推（App.tsx:2115）——survey→refactor_assemble_local→人物/absorb/group/statusbar/interface 全並行 pool（chain 恆空、warmed:true 直進並行，refactor-run.ts 加 warmed 選項）；knownFields＝survey.fields 固定一份；產物合併含 local＋dropped/unabsorbed/audit 透傳；刪 RefactorPlanEntry／rewrite 分支／knownFields 累積。
- **包3**（sonnet 代理＋主線複驗，cargo 463→470，commit 978d46c）：absorb_messages/parse_absorb（本文照搬鎖定、AI 只出 RULES/TRIGGERS、`{{span:uid#sN}}` 指位）＋group_messages/parse_group（GroupKind、大組 >4000 字保險）＋expand_span_placeholders；lib.rs 三新 command refactor_absorb_entry（Rust 端組完整條目含 meta）/refactor_split_group/refactor_expand_spans（statusbar 段走 interface 型）；刪 PlanKind/RefactorPlanEntry/rewrite_messages/parse_rewrite/refactor_rewrite_entry。
- **包2**（sonnet 代理＋主線複驗，cargo 455→463，commit bc3eef7）：新模組 [refactor_assemble.rs](../../src-tauri/src/refactor_assemble.rs)（assemble_local:66——carry 含元資料 byte 相等、split 七路由組裝＋餘段兜底、clean 人物組卡、四項稽核 coverage/mechanism/split/drop_rule）；RefactorEntryMeta＋RefactorNewEntry.meta（refactor_ai.rs:1383）；apply() meta 雙軌落檔（refactor.rs:286，meta.order 不吃遞增）；RefactorOutcome 加 dropped/unabsorbed/audit 三欄；lib.rs:603 command refactor_assemble_local。
- **包1**（sonnet 代理＋主線複驗，cargo 442→455）：EntrySpan/segment_spans（refactor_ai.rs:103/112，空行切段、byte 相等 property 測試）＋⟦sN⟧ 標記（mark_entry_spans:164）＋PrescanSignal/prescan_worldbook（:215/:238，trigger:/rule:/逐日 regex）＋新 SURVEY_BODY 六區塊（:313）＋survey_messages 帶 signals（:420，lib.rs:569 接線）＋新 structs（RefactorSurveyPerson 擴充 mode/spans/private_spans、RefactorEntryVerdict、RefactorSpanRoute、RefactorSplitGroup、RefactorSurveyOutcome 刪 plan 改 verdicts/splits/groups/fields）＋六區塊解析器（locate_fields 固定欄序通用抽取）。RefactorPlanEntry／rewrite 舊中段保留待包 3 刪。

## 盤點結論（2026-08-11，主線親查）
- **18k 對號**（~/Documents/TableTavern/prompt-cache.jsonl，14:21–14:46 run）：14 筆＝survey（opus-4-7，3705 tok）＋條目重寫 11（5 設定小筆 0.5–4k＋6 機制大筆 7–18k）＋介面 1（2542）＋人物展開 1。**兩筆 18k＝機制條目重寫，非介面**；重寫可見正文僅 ~0.5–1.2k 字/條→輸出 token 大宗＝思考＋JSON 開銷。結論：照搬消滅整筆呼叫（含思考）是提速主力；介面軌無大額，維持「無介面卡不生介面」。
- **快取紅線**：[refactor_ai.rs](../../src-tauri/src/refactor_ai.rs) 測試 `all_stage_system_messages_are_byte_identical_for_same_context` 把關跨階段 system 相等；階段指示都在 user 訊息端。`assemble_card_context` 為重構專用（遊玩線走 transport.rs），context 內加 span 標記不影響遊玩、不破跨階段相等。
- **認人沿用**：PERSONS 區塊＋buildRefactorPersonPlan（單一專屬來源本地轉換）＋person_expand＝person-promote 實作，本案只擴充欄位、不做第二套認人。
- **IR 就緒**：MECHANISM_SCHEMA（FieldRule＋Trigger）→ RefactorNewEntry.rules/triggers → apply() 併 state.mechanism＋locked＋機制帳本（refactor.rs:272–303），接管線直接沿用、不重造。
- **apply() 現況**：新條目一律 keys=[]、constant=false（refactor.rs:279）→ 照搬路徑必須補元資料保留。

## 小抄合約 v1（包 1–5 依此實作）

### Span 基礎建設（app 端）
- 條目內容以「空行」切 span（段）：span＝原文 byte 區間，依序串回（含原分隔空行）必得原文 byte 相等。`format_worldbook_entry` 每段行首加標記 `⟦s1⟧`（各條各自從 s1 起編）；引用寫法 `uid#sN`。
- 結構預掃：app regex 掃各 span（不分大小寫）：`trigger:`／`rule:`／逐日樣式（`第[一二三四五六七八九十\d]+天`、`每日`、`\bday ?\d`）→ 訊號清單隨 survey user 訊息注入（「結構預掃訊號：uid=3#s2 含 `trigger:`」）。判官判定與訊號衝突（含訊號條目蓋 carry）必附 reason 一句。

### survey 輸出六區塊（新 SURVEY_BODY，只動 user 訊息端）
```
## PERSONS
- name: 霍玄 uids: 12,45 player: yes mode: clean spans: 12#s1,45#s2 private: 45#s3
- name: 阿蘭 uids: 8,9 mode: tangled
## INTERFACE
- uid=201 playable: no
## ENTRIES
- uid=3 action: carry
- uid=4 action: carry reason: 歷史年表非機制
- uid=9 action: absorb
- uid=5 action: drop rule: 2
- uid=7 action: split
## SPLITS
- span: 7#s1 route: statusbar
- span: 7#s2 route: gm
- span: 7#s3 route: drop rule: 1
- span: 23#s2 route: person name: 霍玄
- span: 23#s4 route: entry title: 王府概況
- span: 16#s2 route: group id: g1
- span: 16#s6 route: unabsorbed note: 擲骰檢定
## GROUPS
- id: g1 title: 格式與行為 kind: mechanism spans: 16#s2,16#s5,18#s1
## FIELDS
- 好感度
- 淪陷天數
```
- PERSONS／INTERFACE 沿用現行格式；PERSONS 新增選配欄（固定順序 name→uids→player→mode→spans→private）：`mode: clean`＝所列 span 原文組裝成卡（private 列私密段、其餘公開，零呼叫）；`mode: tangled`＝一人一次 balanced 呼叫（現行 person_expand）；mode 缺席＝沿現行分流。
- ENTRIES：每條非純人物條目一行，action 封閉字彙 `carry|absorb|drop|split`（照搬｜接管｜淘汰｜需拆）。drop 必附 `rule: 1|2|3`（①輸出容器紀律 ②版本標記/更新日誌 ③ST 引擎專屬鉤子）。純介面格式條目走 INTERFACE 即可；混寫的走 split。
- SPLITS route 封閉字彙：`statusbar`（欄位綱要＋顯示條件→該條目一次 interface 型呼叫抽 STATE，emoji 標籤留值裡）｜`gm`（敘事行為指令→原文組裝進 GM 規則條目，零呼叫）｜`drop rule: n`｜`person name: X`（X 必為 PERSONS canonical 名）｜`entry title: T`（設定段→同 title 段原文串接成新條目，零呼叫）｜`group id: gN`｜`unabsorbed note: …`（app 尚無機構的機制→原文組裝進 GM 規則條目＋列「未接管機制」清單）。
- FIELDS＝命名權威一次頒布＝knownFields 唯一來源；執行端未列名者照原文沿用、禁自創，產物欄位名事後機械 dedup。

### 執行端呼叫（全並行上限 4，序列鏈廢除）
- absorb：一條一次 balanced（新 absorb_messages）：入＝該條全文（含 span 標記）＋FIELDS＋MECHANISM_SCHEMA；出＝僅 `## RULES`＋`## TRIGGERS` JSON——**無 CONTENT，條目本文原文照搬＋locked**。trigger text／preamble 可寫 `{{span:uid#sN}}` 指位，app 組裝時替換原文。
- group：一組一次 balanced（新 group_messages）：入＝組內 span 原文＋FIELDS＋目標 schema；出＝kind=setting→`## CONTENT`；mechanism→CONTENT＋RULES＋TRIGGERS。大組保險：組源合計 >4000 字時 prompt 指示可用 `{{span:…}}` 指位照搬、只重寫真糾纏句。
- statusbar：per split 條目一次現行 interface 型呼叫（material＝該條 statusbar spans）。tangled person＝現行 refactor_expand_person。INTERFACE uid＝現行不變。
- 首發快取由 survey 本身建立（同 run 先行），pool 全並行直接開跑。

### 零呼叫組裝＋機械稽核（Rust，新 command refactor_assemble_local）
- carry：來源條目→產物條目，content byte 相等＋keys/constant/order/visibility 原樣（RefactorNewEntry 加選配 meta 欄，舊產物 JSON 不帶 meta 照舊可解）。
- clean person／entry title／gm／unabsorbed：span 原文串接組裝；gm 條目 title＝原標題（撞名才後綴 " (GM)"）。
- dropped：Vec<{uid,title,content,rule}> 全文入清單。
- 稽核四項（audit report，玩家可見）：
  1. 涵蓋：每 uid 必有下落（PERSONS/INTERFACE/ENTRIES 其一）；漏網→自動補 carry＋報告列出。
  2. 機制守恆：預掃訊號 span 必落 absorb/statusbar/gm/group(mechanism)/unabsorbed；落 carry 且無 reason→報告紅字＋列未接管機制清單（原文仍保留，資料不掉）。
  3. 拆組守恆：split 條目每個 span 必有 route；漏→補進「<原標題>（餘段）」carry 條目。carry byte 相等由「slice 原文組裝」保證＋測試斷言。
  4. 淘汰稽核：drop 缺編號或編號不在 1–3→自動退回 carry。

### 產物與 UI
RefactorOutcome 擴充：entries[].meta＋dropped[]＋unabsorbed[]＋audit[]。淘汰清單 UI：預設不套用、收合列表、逐條展開看全文、一鍵放回（轉 carry 進 entries 並勾選）。未接管機制＝資訊列表（內容已在 GM 規則條目裡）。

## 實測清單（新對話實機驗收用；2026-08-11 拍板：實作對話 context 過大，驗收另開對話）
環境：`npm run tauri dev`（Bash 啟動一律 `env -u ANTHROPIC_BASE_URL`）；時間帳看 `~/Documents/TableTavern/prompt-cache.jsonl`。
- **T1 orc-cave（主指標）**：①總時長 <5 分（jsonl 佐證）②豺狼人／深藍狼／巨魔等純設定＝carry，套用後 content byte 相等＋keys/constant 保留 ③「巴古克與古茲卡入侵劇情線」（逐日）＝absorb：kind=mechanism、locked、RULES/TRIGGERS 非空、帳本記已接管 ④版本標記條蓋 drop rule 2：入已淘汰清單、預設不套用、展開看全文、一鍵放回 ⑤audit 無紅字。
- **T2 NorthHall（23 條八角色）**：①每人「速览段＋人物設定＋性格」跨條合成一張完整卡不碎裂 ②三條「剧情-」觸發歸機制線（absorb 或 group mechanism）③「格式增强Plus」整條淘汰 ④「美化状态栏」三路拆＝欄位綱要→STATE（emoji 標籤留值）、行動選項→GM 規則條目、容器紀律→drop①。
- **T3 Transfur（16 條盲測定案）**：①目錄四條＋keyed 地區四條 carry 含元資料（keys 在）②核心設定歷史年表 carry 且該行附 reason（預掃衝突）③「格式」「COT」多路拆＝欄位綱要→STATE、敘事行為指令→GM 規則、gametext 容器→drop①、擲骰／ASCII 地圖→unabsorbed 清單可見。
- **T4 通用**：①取消鍵中止在途＋Cmd-Q 無孤兒（refactor-dispatch P4–P6）②API 未設 balanced 退 GM（P8）③舊 refactor-outcome.json（無 meta/dropped 三欄）匯入照舊可套用 ④淘汰／未接管面板十語系文字正常。
全過＝本案＋refactor-dispatch 一起結案。

## Next action
新對話實機驗收：`npm run tauri dev`（Bash 啟動帶 `env -u ANTHROPIC_BASE_URL`）匯入三張測試卡（TestCards/）照上方實測清單 T1–T4 逐項驗；發現問題帶本檔開修。全過＝本案結案＋refactor-dispatch P4–P6/P8 一起收（兩案狀態 completed、TASKS.md 行搬 DONE.md、已驗收段搬 archive/）。

## Constraints
- 新格式規格放 survey user 訊息端；survey／expand 共用 system 逐位元組相同紅線零觸碰（既有測試把關，span 標記在 context 內、各階段共用不破相等）。
- knownFields 單一權威＝小抄 FIELDS（取代鏈上累積）；並行上限 4 不上調。
- 不在封閉清單或拿不準→一律 carry；作者設計內容永不 drop。
- 驗收（主線）：orc-cave 總時長 <5 分＋照搬 byte 不變＋入侵劇情線接管可跑＋稽核綠；NorthHall 八角色一人一卡不碎裂；Transfur 16 條盲測項；過後補驗 refactor-dispatch P4–P6/P8。
