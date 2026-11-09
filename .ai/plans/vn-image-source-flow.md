# vn-image-source-flow — VN 開桌先選來源再生成、額度用完不卡死、失敗講人話、立繪不再縮成一團

案別：大案，核心重估約 595 行（原估 400 行；採主線簡化機制，依下方技術細節；方案與驗收都送審；工程審 Grok，施工 Sol〔作者裁決 2026-11-09〕）。Fable 常識審開工前已做，必須改保留；定稿合併佇列為建議，依主線回應不採用。

## 已查實（0.5.2 與現行碼生圖相關程式無差異）
- **一進美術步就生成**：`ArtStep` 掛載即 `vn_sprites_prepare`，`useArtProgress` 標題畫面「缺又沒在排」就重排，一鍵路線建桌後排首幕背景。預設來源沒設＝codex（`vn_sprites.rs` `sprite_source`、`SpriteBasePicker.tsx`），不管玩家有沒有裝。
- **額度用完變 `NO_IMAGE`**：agy 的 `generate_image` 工具步驟回 `TOOL_ERROR`（429 RESOURCE_EXHAUSTED、reason QUOTA_EXHAUSTED，message「Your quota will reset after 4h39m1s.」），模型接著回 `NO_IMAGE`，`result.status` 仍是 SUCCESS，沒有圖。程式只看回覆文字（`generated_image/cli_fetch.rs` `missing_reason`）。既有本機 CLI log 與串流錄音皆已只讀確認；精確欄位與去識別化限制見「技術細節」§2，不提交原始路徑或錄音。
- **額度很少**：作者帳號約 18 張用完（gemini-3.1-flash-image）；6 立繪＋1 背景＋每位約 4 臉層約 31 張，一桌就超額。重置時間會往後移，只能當大約值。
- **失敗留到舞台**：佇列失敗只記在記憶體、不自動重排（背景單次工作內另有一次自動重試）；`useBackgrounds.ts` 已保存 `failed`，但舞台未呈現失敗狀態（done 才重讀圖片），第一張背景失敗＝永遠黑底、沒提示。
- **拒絕訊息變旁白**：`cli/stream.rs` `parse_agy_line` 只有 status 非 SUCCESS 才算錯。agy 把 Google 拒絕句當正文回且 SUCCESS，就存成 GM 回合。全 repo 沒辨識這個句型。
- **立繪一團紅**：CLI 來源生圖沒傳比例、不驗比例、不裁透明邊（`vn_sprites.rs` `generate_png`）。把正常立繪補透明邊成 16:9，舞台畫面和朋友截圖幾乎一樣；裁掉透明邊就恢復正常。實際 agy 立繪上緣留白 1.3%～21.2%。朋友的圖實際比例未驗證。
- 去背本身：測試通道實測 agy 立繪去背正常。作者截圖裡成功的拉克是 Codex 生成，不算證據。作者 0.5.2 測 Gemini CLI 全失敗是查因測試先用完同帳號額度造成的。

## 要做
1. **先選來源、再按開始生成**
   - 美術步頂列「立繪來源 [Gemini CLI ▾]　[開始生成]」，這是此步主鈕；下方列當桌實際待生立繪＋首幕缺背景數，說明進遊戲後按上台角色補缺的表情（固定四鍵＋卡宣告的額外表情／已有變體獨立組）；已上傳／已有圖不計。按下顯示該批實際完成數／總數。
   - 只裝一家也要按：這一下是同意花額度。下拉預選已連線且能生圖的那家，不列沒裝的 CLI。
   - 底部「進入遊戲」在按生成前降為一般鈕，文字就是「進入遊戲」，讓想自己上傳的人能直接進。
   - 用每桌旗標（如 `art_started`）擋掉三個既有自動觸發；補「還沒開始」狀態的文字（`vnStartPreparing`、`vnArtPreparing`、`vnArtQuota` 現在都假設已在生）。
   - 一鍵「直接開始」不停下〔模型判斷·未裁決；常識審查員〕：用記住的來源，沒記住用已連線能生圖那家；兩家都能生圖時在主鈕下露一行「美術用 Codex CLI 額度（改）」。一家都沒有＝跳過美術，標題畫面寫「還沒有立繪 → 到美術一覽選來源」。
2. **額度用完整條佇列暫停**
   - 抓 agy 工具錯誤，撞額度就暫停佇列，不逐張失敗。一條訊息「Gemini 的生圖額度用完了，約 4 小時後恢復；剩 13 張，酒館開著時會自動續生」（數字由現況計算；未知恢復時間只提供重試）＋「換來源」「立刻重試」，在美術一覽與標題畫面同一位置。
   - 其他 `NO_IMAGE`／`REFUSED` 換白話，十語系；玩家畫面不出現內部代碼。工具錯誤用既有 app log 診斷（遮蔽家目錄與帳號，不給玩家看），不新增桌 log。
   - 換來源後失敗工作重新排回，不必重開 app。
3. **臉層上台才補**：角色第一次在舞台出現才排他缺的表情（通常 4 張，含卡宣告擴充），依出場順序；沒臉層用定稿。背景維持插在臉層前。`prepare.rs` 定稿生成與佇列 worker 目前是兩條任務，會交錯排程（已有 source_gate 串行化單次生圖），保留兩條任務，單一 gate 控制來源暫停；各自保留待續狀態，恢復後重啟。
4. **背景失敗看得到**：舞台用時段色調深色漸層＋地點名小字代替純黑，右上角小標「背景沒生出來 · 重試」，點了才花額度；額度用完時小標寫原因。不彈窗、不擋對話。有前一張照現況模糊頂著。
5. **故事回合的拒絕不當旁白**：收尾時辨識已知 Google 拒絕句型（累積正文與 `result.response` 都比對；不暫存串流），走既有 CLI 回覆錯誤流程，訊息白話，丟掉該 lane 的對話線。
6. **立繪裁透明邊＋比例防呆**：去背後依 alpha 裁透明邊（下緣貼齊），在算臉框前做；舊桌既有立繪不動。裁完寬大於高才算失敗，顯示「生出來是橫圖，再骰一張？」由玩家按，不自動重骰。CLI 提示詞補「2:3 直式七分身、人物置中、淺灰純色底」（先對照現有 `BASE_SPEC`，已有的不重複）。
7. **換章提示**：`ChapterCardView` 的 `opening` 階段有章名時沒有進度字。標題下加「正在寫這一章的開場……」，背景在排就接「，背景準備中」，十語系。

## 技術細節

2026-11-09 簡化版；採納主線回應 **1–12 全部**，取代前版機制，不刪七項功能或 Fable 必須改。核心重估 **約 595 行（約 520–680）**；不含測試、十語系譯文、純型別連帶與 structure-only 搬檔。新名稱為施工落點；方案審已通過，依下列必改施工，不呼叫付費模型。

### 0. 已查核的地基與簡化界線

- `vn_sprites.rs::generate_png` 的全 app `source_gate()` 已涵蓋背景、臉層、定稿、手動候選、變體，單張呼叫已串行化；保留 `prepare.rs` 與 queue worker，**不增 Base 工作、不統一兩個 worker**。沿用 queue 背景先於臉層，定稿與 queue 之間不保證先後。
- `useBackgrounds.ts` 已保存 failed，舞台沒露出；不重寫 hook。`vn_backgrounds.rs::job` 現有失敗後再試一次，額度／暫停要直接返回，不走第二次。`render_face` 只在對齊不過時再骰，這部分保留。
- 額度欄位已由 repo 的 agy 成功／badmodel 夾具、stream／runner 測試，以及前輪只讀的五份本機既有失敗串流核對；**錯誤在 stream-json，不必掃 CLI log**。本輪未重跑 CLI，也不提交原始錄音、家目錄、帳號或卡片內容。
- 主線已提供故事拒絕完整原文（§5）；不再列為待取得證據。只實作這個已知句型，其他句型未知。

### 1. 先選來源、按開始才自動補圖（核心約 170 行）

- **每桌只存 started 與 appeared**：在 `data/vn.rs::VnFile` 加 `art: ArtFlow`，`ArtFlow { started: bool, appeared: Vec<String> }`；沿用 `worlds/<id>/vn/vn.json` 的 `read_vn/edit_vn`，不另開 art-flow 檔或 data 模組。serde default 讓新桌／舊桌缺欄位均為 false／空陣列；不根據已有圖或 setup 結束推斷同意花額度。壞檔沿用 read_vn 回錯，不以預設覆蓋；重播不排圖。
- 新小函式 `data::vn::{art_flow,set_art_started,note_appeared}` 只讀改這兩欄；寫入沿用 `WorldWritePermit` → `state_commit::with_commit` → `edit_vn`，不持鎖等模型。appearance 去重、只存角色 id 與首次出場順序。
- **來源維持全域** `AppConfig.preferences.vn_sprite_source`，沿用 `sprite_source/read_config`；不加每桌來源。開始生成先 await `updateConfig` 保存下拉值，再呼叫開始命令；保存失敗不啟動。每張工作沿用其已讀 config，已在途／已備好 prompt 的一張可能用舊來源，後續讀新設定；不為跨桌切換加 snapshot／revision 協定。
- 在既有 `commands/vn_sprites.rs` 加薄邊界 `vn_art_start/status/retry`；start 寫 started=true 後呼叫既有 `prepare::start` 與首幕背景 fill；status 合併 ArtFlow、既有兩種 snapshot 與來源 pause；retry 見§2。登記 `lib.rs` handler／harness 名單，更新 `shared/contracts/backend-contracts.ts`。不新建 flow domain 或第二套佇列模型。
- `SpriteSourceSelect` 沿用 `detectClis`＋`cliConnectedKey`，只列已偵測、已連線的 codex／agy；更新／偵測中不給開始；API 沿用已有生圖設定的可用判定。預選「記住且可用」→可用第一家，不把未安裝的記住值加回選項、不發模型探測。
- `wizard/ArtStep.tsx::ArtStep/FirstBackdrop/finalize`、`VnWizard.tsx::begin/startAuto`、`useArtProgress.ts::read/retry`：移除未授權的掛載 prepare、finishRef 無條件 prepare；標題只在 started 且沒有 pending／failed 的首次進桌補缺圖，刷新事件不自動重試失敗。`prepare::start`、`queue::fill`、`vn_backgrounds::fill` 後端都檢查 started，覆蓋 `VnPlayer`／`useVnCells` 背景 fill、prefetch 等全部自動入口。
- 美術步頂部來源＋「開始生成」為主鈕；底部「進入遊戲」在尚未開始時為一般鈕，只 finalize 已選／上傳資產與清 setup，不偷偷 start。一鍵在 `IdeaStep` 顯示實際來源與「改」，`startAuto` 建桌後、首次 fill 前呼叫 start；無來源保持 started=false，繼續故事並提示到美術一覽選來源。
- 單張生成／再骰是明確動作，可以只做該張；不把 started 改 true，不因此開全桌自動臉層。上傳、挑定、看圖均不啟動自動生成。明確單格臉層再骰沿用原入口，只排那格；共同 pause 檢查仍在 generate_png。
- **併入已同意文案**：顯示當桌實際「待生立繪數＋首幕缺背景數」，進度分母在本次開始時取該批，不用預設 6＋1；表情按 `face_list/missing_faces`，額外表情／已有變體獨立組納入估計，既有資產不算待生。說明臉層上台後才補，不承諾所有卡都恰四張。
- 測試：未開始各入口零生成；按一次後冪等、來源保存順序／失敗、僅 agy／無來源／舊記住 codex、一鍵不中斷、上傳後直接進桌、舊 vn.json 預設值；延伸既有 wizard／progress 測試，不另建持久化工作測試。

### 2. 工具錯誤、記憶體暫停與恢復（核心約 180 行）

- **精確欄位**：`event=step_update`、`step_update.step_type=tool`、`tool_name=generate_image`、`state=ERROR`；`tool_info.error.type=TOOL_ERROR`，`tool_info.error.message` 的 `body:` 後為 JSON，含 `/error/code=429`、status=RESOURCE_EXHAUSTED、`details/*/reason=QUOTA_EXHAUSTED`；metadata 有 quotaResetTimeStamp／quotaResetDelay，RetryInfo 有 retryDelay。之後仍有 result SUCCESS＋NO_IMAGE，現有 `parse_agy_line` 把工具事件當 Other。
- `cli/stream.rs::parse_agy_line` 內重用 serde_json 解工具錯誤，抽小型 `cli/stream/agy_errors.rs` 管欄位／穩定碼／§5句型；只認 generate_image 的工具錯誤，不搜 prompt 或正文。精確 QUOTA_EXHAUSTED 映穩定額度碼＋可選 reset；裸429不宣稱數小時後恢復。UTC 優先，其次 delay，沒有可靠時間就 None。
- 額度碼走既有 `CliLine::Done { is_error:true }`，agy 啟用既有 sticky_failed_done，使後來 SUCCESS 不蓋錯；沿用 `runner/verdict.rs::decide`／CLI 錯誤流程。已知穩定碼在 runner 錯誤出口原樣保留（比照既有 AI_INCOMPLETE_RESPONSE），讓共同生圖入口不解析玩家譯文；其他工具錯誤用既有 SoftError 診斷；進入任何 warn／SoftError 前先做同一份路徑與帳號遮蔽，不能讓既有 stderr 診斷繞過遮蔽。`cli_fetch::run_cli_image` 錯誤返回也收尾清本次 agy 對話，避免 `.await?` 漏清；取圖與 NO_IMAGE／REFUSED 還是原邏輯。
- 新 `vn_sprites/pause.rs` **只管理記憶體**的來源→pause(reset) 與 timer；不存待辦、failed、pause、revision、完成世代，不開機掃桌。唯一執行前檢查在 **`generate_png` 取得 source_gate 後、呼叫來源前**：暫停回穩定碼；本次撞額度也在放開 gate 前記 pause。其他入口只辨識返回碼、做停工收尾，不各加預檢。source_gate 不必換鎖，queue／Book 的 Mutex 不跨 await。
- `queue::start_worker/run_face` 對兩種暫停碼統一收尾：running 工作放回 waiting、清 running／worker、停迴圈，不標 failed、不逐張 emit 失敗；背景同樣走這段。保留其餘待辦與 BgMode。enqueue 在停工交界仍按既有 queue 鎖去重／啟動；即使有一次額外排程也只在 gate 返回暫停，不呼叫模型。
- `prepare::run/start` 遇暫停把本桌留在既有 pending（queued／待續），清 running、結束這輪；外層即使有 again 也不再立刻重掃。只增加 `resume_pending` 從現有 pending 推導需續的桌，不另存角色工作描述。run 組 todo 時跳過既有普通 failed，只有明確重試清掉該桌 failed 才再生，避免timer為一個待續角色順便重試整桌失敗。保留 generating/adopt、claim_base、install_at 與現有世代重驗；不合併 worker、不改手動採納優先序。
- **恢復只有一般重啟**：每來源最多一個 tokio timer，到 reset 清 pause，呼叫 `queue::start_worker` 與 `prepare::resume_pending`，兩邊照常推導／跑缺圖；第一張若仍超額，gate 再暫停。無單張探路、無 revision。重試／換來源／新額度期限時取消或替換該來源舊 timer，維持「一個」；timer 清除前在同一 gate 核對目前期限確實已到，避免正常連按重試時舊 timer 把新 pause 提早解掉。未知 reset 不排 timer，玩家按重試。
- **換來源／重試**：保存全域來源（換來源時）後，清新來源 pause、取消它的 timer，清該桌 queue.failed 與 prepare.pending 的 failed，再重新推導定稿／appeared 臉層／目前幕背景，重啟兩邊。清背景 failed 前保留既有 BgSlot，以原 BgMode 重新排入明確強制重生／專屬時段，不因已有舊圖而吃掉這次按鈕。其他桌普通失敗不清；同來源已暫停待辦可隨共用 pause 解除續跑。
- **重開 app**：只讀 started／appeared，重新推導沒定稿的角色、曾上台角色缺的臉、目前幕缺背景；既有記憶體待辦／failed／pause 都消失。先開進的桌第一張若還超額，會再收到一次供應商額度錯誤並重新暫停，可接受；此項不承諾供應商計費政策。未開始桌仍不自動生成；舊桌現有圖不動。未到訪桌不在背景自動跑。
- **診斷沿用 app log**：工具錯誤 `log::warn!` 記 type/message，家目錄與帳號先遮蔽；不記整條帶 parameters／prompt 的 stream。穩定碼給玩家白話、原診斷留既有 app log，不新建桌 log，不混 transcript／用量帳本。背景 job 對額度／暫停直接返回，不再試第二次；其他原有重試行為不擴改。
- 測試：最小合成工具 ERROR→SUCCESS 夾具、reset／裸429／普通工具錯誤、兩個 worker 加手動候選共用 gate 只首張真的呼叫、工作留 waiting／pending、timer 到期／替換／未知期限、換來源及重試清本桌 failed、重開後重新推導。fake_cli 驗收尾，**不執行 ignored capture_cli_fixtures**。

### 審查必改（已對照程式確認）

- 明確重骰：`vn_sprite_reroll` 先drop_face，原前端再fill；新增後端明確單格／整組排入入口，繞過 started＋appeared 自動閘，只在generate_png受pause限制；不得刪完卻無法排入。`vnSpriteTabIntro` 十語系改成上台才自動補，手動仍可補整組。
- 變體共用：`shared_body` 比例公差2%，`place_variant` 必須先對變體做與定稿同一套裁切再比；可共享時輸出仍是定稿尺寸、原框，獨立組才重定臉框，取代前版「未裁比對」。

### 3. 臉層上台才補（核心約 60 行）

- `VnPlayer.tsx` 的 `stageAt` 已有目前 `stage.slots`；`useSprites(worldId, visibleIds, active)` 僅在播放器實際可見、無標題／精靈／章卡覆蓋時，依首次上台順序呼叫 `vn_sprites_fill`。後端 `note_appeared` 驗現存／非 archived、stage／非 replay 角色，只在新 id 出現時寫 vn.json。
- `commands/vn_sprites.rs::vn_sprites_fill` 接可選角色子集；省略只補 appeared，不再 list 全桌。`queue::fill` 自動補圖限 started＋appeared，沿用 `faces_of`／`SpriteDir::missing_faces`，保留額外表情、已有變體獨立組與 queue 背景插臉前順序。
- `VnWizard::begin` 移除開演全桌 fill；`useSprites` 移除掛載全桌 fill；`prepare::install` 改為這角色已 appeared 才補。無定稿先記上台需求，定稿完成再推導；`useSpriteSlot::run` 上傳／換框／換變體後只補已出場的本角色。精靈 `ArtRow` 保持 fill=false，不在美術步先補全臉。
- 保留 `queue::{forget,forget_slot}`、`store::Snapshot`、base／variant epochs；stale requeue 仍按當前 appeared 與資產重驗，不加完成世代。沒有臉層直接沿用現有 SpriteFigure 定稿退路。
- 測試：兩角色依次上台、未上台／標題後方零臉、定稿晚到、退場／回退不重排、額外表情與變體獨立組、改框丟過期結果；主要放 `useSprites.test.ts`／現有 queue、prepare 測試。

### 4. 背景退路與共用訊息（核心約 55 行）

- `useBackgrounds.ts::statusOf` 繼續用現有 failed／queued／running；共用 pause 由美術 status snapshot／事件提供。`VnPlayer`→`Stage`→`StageBackdrop` 傳目前地點、狀態與重試。沒 src／last 用時段深色漸層＋地點小字，有 last 維持模糊；failed 顯示「背景沒生出來 · 重試」，paused 優先顯示來源原因，未開始／排隊不誤報失敗。
- 重試用既有 `vn_background_regenerate`，不因 render 自動重排；paused 時走§2明確重試再重排本張，仍只在 generate_png 檢查執行權。沿用上傳取消待辦、`Library::wants` 寫回重驗，不蓋剛上傳圖。
- 共用 `ArtFlowNotice` 放美術一覽與標題相同位置；預設未開始提示與開始按鈕、實際剩餘數、約略恢復時間、換來源／立刻重試。`shared/ui/ai-error.ts`、`useSpriteSlot.ts::spriteError` 對 quota／NO_IMAGE／REFUSED／橫圖用穩定碼＋十語系；故事拒絕用故事文案，不寫生圖來源。自動恢復說「酒館開著時」，無 reset 不承諾定時續生。
- 測試：無圖／舊圖、四時段、failed／paused／queued／未開始、重試一次、晚到舊桌事件；延伸 Stage／gallery 測試與後續 harness 截圖。

### 5. 故事拒絕：Done 比對、重用撤線（核心約 40 行）

主線提供、允許進夾具的實樣（本輪不另呼叫供應商驗證）：

> The prompt could not be submitted. The prompt contains sensitive words that violate Google's [Generative AI Prohibited Use policy](https://policies.google.com/terms/generative-ai/use-policy). Try rephrasing the prompt. If you think this was an error, [send feedback](https://aistudio.google.com/docs/troubleshooting).

- 純函式放 `cli/stream/agy_errors.rs`：**trim 後**以 `The prompt could not be submitted.` 開頭、含 `Prohibited Use policy`、UTF-8 全長 **≤1,024 bytes**，三條全滿足才認。只套 agy，不做其他白名單／寬鬆 regex，不重用前端 `REFUSAL_ERROR` 掃故事。一般拒絕台詞、引用中間出現、過長故事都不匹配；極短故事若恰好滿足三條仍可能誤判，列限制。
- `parse_agy_line` 的 result.response 與 `runner` 收到 Done 時的累積 full_text 各比一次，命中就把這輪 Done 轉成 `AI_CONTENT_FILTERED: agy` 錯誤；沿用 CLI 回覆錯誤出口。**不碰 Delta 分支、不暫存、不改串流釋放**；短暫看見拒絕句可接受，最終 Err 清臨時串流、不落 GM 成功事件。測試同時核對現有前端 finally 清除與 `chat_turn/gm.rs` 成功才落檔，不新增另一套串流生命週期。
- **lane 最小改法已確認**：`lanes/mod.rs` 在一般 `Err(_) if !opening` 之前加一個 agy＋精確拒絕碼 guard（開線／續聊共用）；依序重用 `discard_new` 清本次額外 id、`retire_agy_conversation` 移除並保存 lanes.json 後刪原對話，原錯誤返回。這樣不會走現有 resume 自動 reopen 再花一次額度；`settle_abort` 是中止／抹寫，不拿來代替撤線。
- 這個 guard 約 6–10 行，沒有新錯誤子系統，不抽 `failure.rs`；規約允許責任單一的小增量略超千行。若後續確有第二個共用責任才另做完整 structure-only 抽檔，本案不預先搬家。
- 測試：給定實樣、前後空白、僅增量／僅result命中、正常內容＋result拒絕、兩條必要文字各缺一、引用／過長／非agy；fake_cli 驗開線與續聊均只呼叫一次、丟線、下一次才新開、無正式 GM 拒絕旁白。保留玩家已送訊息，不刪卡內容。

### 6. 裁透明邊、臉框、變體（核心約 75 行）

- 新 `vn_sprites/trim.rs::trim_body` 重用 image crate：alpha 非零最小包圍盒裁四邊，內容下緣貼新畫布底，保留半透明耳髮；全透明失敗，裁後 width>height 才報橫圖，正方形允許。橫圖由玩家按再骰，不自動重試。
- `generate_candidate` 在 ensure_alpha 後／add_candidate 前裁；`install_at` 在 ensure_alpha 後／locate_face 前裁（冪等），涵蓋新上傳定稿。尺寸、fallback臉框、sprite.json、Snapshot 全用裁後座標。`SpriteDir::read` 不裁，既有定稿／臉層不改、不做遷移；玩家明確換定稿才沿用現有差分失效規則。
- `generate_png` 共用背景／臉層，不能在那裡統一裁或禁橫圖；`render_face/upload_face` 保持固定座標。`place_variant` 先用相同trim_body裁變體，再走 `compose::shared_body`；可共享保留定稿畫布與原框，不能自行裁到共享臉飄走；不共享／上傳變體才獨立裁、驗比例、重定臉框。舊定稿當共享參考也不重裁它。
- **併入已同意構圖**：主規格改為「直式七分身、人物置中、淺灰純色底」；`BASE_SPEC` 與 `background(false)` 已有的措辭不重複，只補 CLI 的 **2:3 portrait canvas**。背景16:9、臉層方形照舊。
- 測試用合成RGBA涵蓋透明補邊／半透明／空圖／真橫圖／冪等／裁後框；各一例共享變體畫布不改、獨立變體重定框與舊資產讀取不改，重用現有 compose／store測試，不為每條舊路徑複製一套。

### 7. 換章提示（核心約 15 行）

- `ChapterCardView.tsx` opening 有 heading 也顯示「正在寫這一章的開場……」；`VnPlayer` 傳 `backgrounds.statusOf` 的 queued／running，才加背景準備中。paused／failed 說原因，不冒充準備中；replay 不顯示生成文字。十語系用完整訊息，不硬接中文逗號；`vn-story.css` 只補副文字間距。
- 測試：有／無章名、背景排隊／暫停／失敗、replay與停止／重試；重用現有章卡測試。

### 分檔、驗證與已知限制

- 估算合計：170＋180＋60＋55＋40＋75＋15＝**595 核心行**；測試上限 **1,190 行**，不超核心2倍。每包延伸最接近的既有測試，先相關 Rust／Vitest，最後 `npm run verify`；不跑 ignored capture，不呼叫任何真模型，GUI只用harness與模擬錯誤／既有圖。
- 現況 `data/vn.rs`856、`cli/runner.rs`905、`transport/dispatch.rs`916、`lanes/mod.rs`1,022、`vn_sprites.rs`613、queue489、`VnPlayer.tsx`750、`VnPlayer.test.tsx`1,191行。前兩者只補欄位／Done接線；dispatch不需新資料通道；lane只加上述guard，無理由抽整套failure。新來源暫停與裁邊各由 `vn_sprites/pause.rs`、`trim.rs` 承擔，不往大檔堆。
- VN根層已超過20檔，來源 selector、共用 notice、現有 `wizard/useArtProgress.ts` 與其測試依美術狀態責任同批放 `vn/art-flow/`，呼叫端改import；不再加 `useArtFlow` 第二套hook。VnPlayer只接線；新臉層hook測試放同stem，不再擴大千行VnPlayer測試。
- **明列接受的限制**：重開後重新推導，未完成的明確強制重生／專屬時段意圖、普通失敗紀錄與來源暫停不保存；再撞一次額度可接受。全域來源切換影響其他桌後續圖，已備好的一張仍可能用舊來源。手動候選／變體被暫停後由玩家重試，不自動再骰。未知reset不自動恢復；app關閉不生成。兩個worker無整體順序保證。其他拒絕句型未知；短故事若符合既定三條可能誤判。朋友原圖實際比例仍未驗證，後續主線安排真機驗收。

## 施工方建議

- **同意主線1–12，無反對項**。撤回前版的待辦／失敗／pause持久化、每桌來源、工作revision／完成世代、定稿合併worker、單張探路、拒絕串流暫存、新桌log與預先抽failure；各自的正常功能由上述現有路徑承擔，罕見重開／跨桌情境列已知限制。
- 第5點的「一個timer」以取消／替換舊timer維持；不是增加revision。若完全不收掉舊timer，玩家正常按「立刻重試」後再撞額度，舊期限到時會提早清掉新的暫停。這是落實單一timer必要的幾行收尾，已計入180行。
- 已同意的實際待生數、七分身＋2:3比例已併入主規格與技術細節。核心估595行，施工若超估仍以完整功能為準，測試維持核心2倍內。
- 權限／Connector／登入設定不在本案改動；沿用現有設定與使用者明示規約，本輪只改方案、commit本分支、不push。

## 不做
- 不內建繞過審查或改寫玩家卡片內容的機制。〔模型判斷·未裁決〕
- 不做 SFW 版 sunsmell 卡：原卡在作者帳號全程沒被擋，SFW 卡驗證不了拒絕路徑。〔模型判斷·未裁決〕

## 驗收
- 程式測試模擬：額度錯誤暫停佇列與恢復、拒絕句走錯誤、裁邊與比例、旗標擋三個自動觸發、一鍵路線不停下。
- 測試通道截圖（Sonnet 拍、主線親看）：美術步未開始、生成中、額度用完、背景失敗、換章提示、紅虎上台。
- 真實 agy 出圖等額度恢復後由主線安排；期間任何測試不得呼叫 agy 生圖。

## 觀察
- 審查拒絕的觸發內容未重現：先前作者帳號探測未被擋，累積上下文或帳號差異仍屬推論；主線已提供朋友截圖與作者 agy metadata 的共同拒絕原文，列於技術細節§5，可作夾具，但本輪未重跑供應商。
