# Handoff: ai-table-generator

## Current state
2026-08-02 三塊全數實作完成。2026-10-04 測試通道代測（OpenRouter 免費模型，實際回應來自 dots-studio/dots-3-note-preview:free）驗出兩個缺陷待修，修完再驗後結案。

## Completed
- 塊 1 後端（codex gpt-5.6-terra 實作、主線審過）：genesis.rs 新檔——outline／expand 提示組裝（角色數零數字錨定）、容錯解析（標記 0–6 個 #、大小寫不拘、半全形冒號、缺 EMOJI→🎭、缺 PRIVATE→空、缺 OPENING 照樣成桌）、materialize 落桌（先解析成功才動磁碟；重名補「 2」「 3」；六色輪配；開場白事件形狀同 create_sample_world data.rs:422）。lib.rs 兩指令 generate_table_outline／generate_table_expand（src-tauri/src/lib.rs:1275、1310）已註冊，回傳 camelCase：`{parsed, raw}`／`{worldId, raw}`，解析失敗＝null＋raw 原文，Err 只留 API 錯誤。
- codex 順手 cargo fmt 掃到五個範圍外檔（cli/data/import/install/transport），純排版零語意，已全數退回。
- 塊 2 前端（codex gpt-5.6-terra 實作、主線審過）：側欄「開新桌」下方 `.gen-table` 按鈕（src/App.tsx:3505）；生成視窗——textarea＋六顆題材 chips（存鍵、呼叫時轉當前語言文字）＋生成鈕（空輸入且零 chip 時 disabled）＋額度小字緊鄰（src/App.tsx:3455-3460）；大綱預覽（標題粗體＋分段＋角色列）；動作列掛在 raw 存在與否（src/App.tsx:3479），展開失敗後重骰／再試開桌都保留；busy 鎖住關閉與全部動作鈕；開桌成功→關窗→重抓桌列→enterTable（src/App.tsx:2867-2895）。i18n 18 個 gen* 鍵：zh-TW＋en 定稿，其餘八語系暫英文佔位。

## Verification
- 主線實跑 `cargo test`：133 passed; 0 failed（127 既有＋6 新增，見 genesis.rs:351-427）；`cargo check` 0 warning。codex 沙盒回報的 3 紅確認是 loopback 禁令誤傷，本機全綠。
- 主線逐行審 genesis.rs：提示詞與拍板規格逐字一致；materialize 動磁碟前必先解析成功。
- 塊 2：主線實跑 `npm run build` ✓ built in 511ms、`npm run check:i18n` 九語系全 OK；範圍檢查 12 檔全在授權清單；JSX 六態逐一對照過。
- 塊 3（codex gpt-5.6-luna 實作、主線複驗）：八語系 gen* 鍵各 17 個換道地翻譯；主線實跑 check:i18n 九語系全 OK（寬度檢查過，de/es/pt-BR/ru 的重骰鈕縮短詞）、build ✓ 504ms；殘留佔位掃描零命中（fr「Depuis un prompt」、ja/ko「SF」為合法 ASCII 譯文）。
- 大綱可編輯（使用者 2026-08-02 追加需求；codex terra 實作、主線審過）：預覽區改可編輯欄位——標題 input、世界摘要 textarea、角色列（名字＋定位＋×移除）＋「＋加一個角色」；`serializeGeneratedOutline`（src/App.tsx:80）把草稿組回標記格式、空名角色略過；開桌改送序列化草稿；展開失敗只設錯誤原文、草稿保留續改；開桌鈕在標題或摘要空時 disabled；新鍵 genAddCharacter／genRemoveCharacter ×10 語系。
- 三項使用者回饋（2026-08-02；後端 codex terra＋前端 codex terra 二輪、主線審過）：(1) 生成視窗 overlay 關窗移除、只留 × 鈕（src/App.tsx:3476）；(2) 角色定位欄改 textarea rows=2 自動長高（helper src/App.tsx:94，ref＋onInput 都套）；(3) AI 生成角色——後端第三指令 generate_table_character（lib.rs:1314、註冊 :1442；提示組裝 genesis.rs:83、parse_character genesis.rs:136，取第一個名字非空角色段）＋前端提示框與按鈕（只在有草稿時顯示，src/App.tsx:3578-3593），成功 append 進草稿並清提示框、失敗依情境顯示 genCharParseFail（genResultMessage 切換，outline／expand 開跑會歸位 "outline"，src/App.tsx:2869、2925）。新鍵 genAddCharacterAI／genCharHintPlaceholder／genCharGenerating／genCharParseFail ×10。前端第一輪因後端 job 尚未落地自我封鎖零改動，第二輪附後端行號證據重發成功（同驗收條件共二輪，符合上限）。

## Verification（大綱可編輯＋三項回饋）
- 大綱可編輯包：主線實跑 build ✓ 482ms、check:i18n 十語系 OK（65 鈕）；序列化、失敗保草稿、disabled 條件逐段親讀核過。
- 三項回饋包：主線實跑 `cargo test` 139 passed; 0 failed（133＋parse_character 6 例）、`cargo check` 0 warning、build ✓ 479ms、check:i18n 十語系 OK（67 鈕）；overlay 無 onClick、自動長高、AI 生成四態與文案歸位逐段親讀核過。範圍兩包皆只動授權檔。

## Remaining / Next action
2026-10-04 代測結果（截圖在當時 scratchpad，未留存）：
- 過：①開視窗、合成事件點視窗外不關（Dialog backdrop=false）、× 關　②生成大綱、重骰　④AI 生成角色（有提示／留白各一，皆掛進列表）　⑤照大綱開桌：世界設定照草稿展開、刪掉的角色沒出現、留下的角色有 emoji／公開／私密、GM 開場白落檔　⑥單人攻略只生一個角色；介面切 en 後大綱為英文。另：刪角色、＋加一個角色（空列）、定位欄自動長高不橫捲正常。
- 不過 A（擋 ③ 與手寫角色）：改大綱任何文字欄整個 app 白屏（#root 卸載）。`src/features/lobby/GenerateTableDialog.tsx:247、253、265、278` 在 setGenOutline 的函式型 updater 裡讀 `event.currentTarget.value`，React 19 在 render 時才跑 updater、currentTarget 已是 null → `TypeError: null is not an object (evaluating 'q.currentTarget.value')`。重現：生成大綱後在標題欄輸入任一字（逐鍵單一 input 事件即重現，非通道雙事件假象）。修法方向：handler 先取 value 再進 updater。
- 不過 B（⑤ 標題）：開桌後桌名不是草稿標題，模型會重新命名。展開提示 `src-tauri/src/genesis.rs:78` 要模型重寫 `## WORLD: <campaign title>`，`materialize`（genesis.rs:174）用展開回傳的標題。
- 發現 C：生成失敗訊息直接顯示原始 JSON（含 OpenRouter user_id），同一段出現兩次（`GenerateTableDialog.tsx:327` 的 `<p>` 與 `:328` 的 `<pre>` 都放 `backendText(genError)`），沒走 `shared/ui/ai-error` 的 `explainAiError`。
修完 A、B（C 視拍板）後重驗 ③⑤。

## Constraints
- 規格全文見 tasks/ai-table-generator.md（標記文字大綱、角色數模型自判不錨定數字、解析失敗不留半套桌、額度明示、免費功能）。
- 提示詞已由主線定稿並隨塊 1 發包（英文標記 ## WORLD / ## CHARACTER / ## OPENING，內容跟介面語言走）。
- 落桌走 create_sample_world 同路徑；開場白事件形狀照抄該函式。
