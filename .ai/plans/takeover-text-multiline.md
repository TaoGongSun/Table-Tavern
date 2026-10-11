# takeover-text-multiline：骨架填值兩個缺陷

填值全在前端 `src/features/refactor/refactor-shell.ts` 的 `fillSkeletonPlaceholders`（`card-shell-route.ts` 的 `floorMessage` 每樓呼叫），Rust 不參與。兩張卡是**兩個不同根因**，立案時「同一根因（多行值縮排）」的判斷經零 AI 重放更正如下。

## 根因

### A. Transfur-safe：行式記錄格式的正文槽
- 骨架 `<mainPage>` 裡 `mainText|{{本回合.正文}}`，該區不是 YAML 容器，走 `fillRaw` 直接字串替換。
- 原卡格式是一段一筆記錄（`mainText|段落1`、`mainText|段落2`…），卡腳本逐行切 `|` 認類型；正文第二段起沒有 `mainText|` 前綴，被當成未知類型丟掉。
- 前情提要樓同病：`card-shell-route.ts` 的 `restore` 在選殼後把正文槽佔位換回多行摘要，也是直接替換。

### B. RPGImmortal：佔位符在 YAML 鍵的位置
- 重放前次實測產物（`scratchpad/ic2/root/.../01M4NGDDXD8C56WKS1P96ESX42` 的骨架與 state.json，yamlTags＝`Status_block`），多行值的縮排其實正確（既有 YAML 容器邏輯已處理）。
- 真正壞的是骨架行 `{{地圖類型}}:`（卡格式的鍵本身是變數）。`YAML_LINE` 的鍵不認 `{` 開頭，整行被當一般純量，填成 `"洞府:"`（因 `:$` 加引號），js-yaml 報 `a colon is missed (7:14)`，與截圖 `scratchpad/ic2/takeover-3.png` 的錯誤位置一致。
- 手動把 `"洞府:"` 改回 `洞府:` 後整個 `<Status_block>` 能解析，其餘行（在場 NPC、導航、行動選項等多行插入）都對。

## 改法（三方審查定案）

### A. 記錄式正文槽（只在非 YAML 區那條路徑，不寫進通用 fillRaw）
- `bodyRecordPrefix(skeleton, yamlTags)` 從替換前骨架判定前綴或 null：非 YAML 區某行只有一個佔位符、是正文槽（`isBody`，容許 `{{ 本回合.正文 }}`）、在行尾（後面只剩空白）、前綴 trimEnd 後以 `|` 結尾。組行用原前綴。
- 正文按 `\r?\n` 切、丟空白行，每行補前綴；某行已以前綴開頭就不再補；全空正文逐字照現狀。不合條件（多佔位、非行尾、`<maintext>…</maintext>`、`正文：{{…}}`）逐字照現狀。
- 前情提要：`floorMessage` 判定前綴後存進 recap（換回文字已照同一前綴展開），`restore` 只做 slot 換回；message、current.text、match.shell 三處用同一份，不看 restore 當下那行的行首。

### B. YAML 鍵位置的佔位符（`fillYamlLine`）
- 順手修：清單項裡的鍵（`- 鍵: {{x}}`）多行值的子層縮排改以鍵所在欄起算（原本以行首縮排起算，讀回空值）。
- 縮排與清單記號 lead 認連續多個 `- `（`- - {{k}}: v`）；區塊純量的父層欄有鍵時是鍵所在欄（lead 長度），沒鍵（`- |`）是行首縮排，有無佔位兩條路徑一致。
- 鍵是佔位、值段沒有佔位符時值原樣輸出（`{{k}}: 1` 仍是數字）。記錄式正文「已帶前綴」比對先 trimStart 該行。
- 鍵候選只認 plain 鍵（去縮排與 `- ` 後開頭不是 `"`、`'`、`[`、`{`（`{{` 除外）、`#`）或一整段引號純量；分隔冒號找「後接空白或行尾」的第一個，佔位符與引號內略過。
- plain 鍵 fillRaw 後先 trim 再 plainScalar；雙引號鍵只對內容 escapeDouble；單引號鍵照現有單引號規則。`- "{{k}}: {{v}}"` 維持現行輸出。
- 值部分只剩空白或純註解：鍵後原樣輸出、不補 `""`；值是區塊指示符（`|`、`>` 加修飾）時原樣輸出並設 blockParent。trim 後以 `#` 開頭的註解行直接 fillRaw，值裡換行換空白。
- `looksLikeYaml` 先把佔位符遮成普通字元，容器首行是 `{{k}}:` 也認成 YAML。

### C. 本回合 分支不被蓋掉
- `card-shell-route.ts` 兩處 `{ ...tree, 本回合: { 正文: … } }` 改成合併既有 本回合 物件再設 正文（Transfur 的 本回合.在場NPC／可互動物件／檢定 原本永遠填空）。

## 已知限制〔模型判斷·未裁決〕
- 兩個鍵填成同名、或與同層固定鍵同名 → js-yaml duplicated key，整塊解析失敗（原卡在酒館也一樣）。
- 正文行含 `|`（如 markdown 表格）時卡只取第二欄，與原卡行為相同。
- 正文槽在 YAML 容器裡時多行正文原樣插入，維持既有行為。
- 骨架有兩個正文槽、其中一個是記錄式時，前情提要取第一個前綴套到全部位置。
- `? {{k}}` 複雜鍵不支援（舊版也壞）。
- 只認 `|` 結尾前綴、只處理正文槽（三方 PASS）；記錄式格式下正文空白行被丟掉。

## 測試
- `refactor-shell.test.ts`：記錄式正文（多段、空行、CRLF、已帶前綴、`{{ 本回合.正文 }}`、縮排前綴）；逐字不變（多佔位、非行尾、全空正文、`<maintext>`、`正文：`）；YAML 鍵位（`{{k}}:` 下接多行清單讀回 `{洞府: [...]}`、容器首行動態鍵、`第{{n}}层:`、`NPC_{{k}}: v`、`"{{k}}":`、`'{{k}}':`、`- {{k}}: {{v}}`、`{{k}}: |` 區塊、行尾註解；特殊鍵值 `""`／換行／`true`／`<<`／前後空白斷言實際鍵名）；不誤判（`- "{{k}}: {{v}}"` 輸出不變、`"{{t}}: 現在"`、`{ {{k}}: v }`）。
- `card-shell-route.test.ts`：前情提要在記錄式骨架下 message、current.text、shell 三處每行帶前綴（含前綴嵌在 JS 模板字串的卡）；本回合 其他欄位保留。
- 回歸 fixture：兩張卡的最小骨架／狀態直接寫在上述兩支測試裡，不依賴 scratchpad。

## 驗證計畫
1. 零 AI 重放：用上述 RPGImmortal 產物與 Transfur-safe 產物（`scratchpad/rt/root/data/worlds/01M4NDE7TK1P3C9B57HF5BTGEM`）跑填值，RPGImmortal `<Status_block>` js-yaml 解析成功、Transfur-safe `<mainPage>` 每段都有 `mainText|`。
2. `npm run verify` 全綠。
3. GUI：`npm run harness:build` 後用測試通道帶獨立 `--root`，Transfur-safe、RPGImmortal 各開接管桌（重構 Sonnet、遊玩 Haiku）跑 1–2 回合，截圖確認 Transfur 正文多段全顯示、RPGImmortal 狀態欄與小地圖正常無錯誤對話窗。
