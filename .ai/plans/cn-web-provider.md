# cn-web-provider 方案：網頁版接中國模型供應商（階段 1）

目標：網頁版加「供應商」設定，簡體版預設魔搭（ModelScope API-Inference）、玩家貼令牌、用寫死排序的模型玩；同一份產物之後由 Capacitor 包成 APK（另案）。立案說明與作者裁決見 [handoffs/archive/cn-web-provider.md](../handoffs/archive/cn-web-provider.md)。

案別：核心重估約 475 行＝**大案**（立案估 250 行），方案與驗收都送審；碰信任邊界（玩家貼的金鑰）。

## 作者裁決

1. 一鍵開聊的預設模型：設定裡寫死排序。〔作者裁決 2026-11-02〕
2. 失敗時絕不自動換到付費模型。〔作者裁決 2026-11-02〕
3. 簡體語系預設供應商＝魔搭，品質優先，接受玩家要先綁阿里雲並實名。〔作者裁決 2026-11-02〕
4. 展開路線讓玩家手選模型，含付費模型；一鍵路線只用寫死名單；手選的付費模型要標明付費。〔作者裁決 2026-11-02〕
5. 中國供應商模式下藏頂欄「下載桌面版」鈕；之後有 APK 再改成下載 APK（另案）。〔作者裁決 2026-11-02〕
6. 兩家都不註冊帳號、不拿真令牌：照官方文件寫，全部靠假端點測試，等中國玩家回饋再修。〔作者裁決 2026-11-02〕
- 矽基流動留作第二家可選供應商（註冊門檻最低：手機號或微信、不必綁阿里雲）〔模型判斷·未裁決〕。

## 已查證的事實（2026-11-02）

**跨站請求**
- 主線用 Chrome 156 從 example.com 帶 `Authorization` 打兩家 `/v1/models`：矽基流動回 401 本文、魔搭回 200 清單，都沒被 CORS 擋；WebKit 實測同結果。Chromium 關卡已過。
- 魔搭預檢的 `allow-headers` 是明列清單（含 Authorization、Content-Type，**不含 X-Title**）；矽基流動是 `*`。

**魔搭（官方文件：API推理介绍、API-Inference使用限制、魔粒体系说明、API-Provider介绍、访问令牌、阿里云账号绑定）**
- 前提：魔搭帳號要綁阿里雲帳號，且阿里雲帳號通過實名認證，才能用 API-Inference。
- 計費單位是「魔粒」，不是錢：註冊後每日登入送 200（短期，當日有效），綁阿里雲再每日送 50；另有一次性的長期魔粒（驗證郵箱 50 等）。用魔粒前要先完善個人資訊、驗證郵箱。
- API-Inference 每次呼叫扣魔粒：輕量 0.5、主流 1、旗艦 2（平均值）；系統原因失敗全額退還。文件沒有用錢買魔粒的管道。
- 會花錢的唯一路徑是 API-Provider：玩家自己在魔搭託管外部供應商（DashScope、DeepSeek 等）的金鑰後，模型 ID 帶 `:提供方名`（例 `Qwen/Qwen3-1.7B:DashScope`）才走外部計費；不帶後綴就是魔粒。
- 併發與額度會依平台壓力動態調整；舊模型會下架。
- `/v1/models` 不用金鑰，回 `{id, object, owned_by, created}`，**沒有上下文長度與價格**。2026-11-02 共 37 支（含生圖、SQL、醫療、評審等專用模型）。
- 錯誤本文是 OpenAI 形狀：`{"error":{"message":"Authentication failed, ...","request_id":...}}`。
- 令牌在 `https://modelscope.cn/my/myaccesstoken` 新建，可選長期或按月短期，最多 10 支。
- 魔搭有 OAuth（`api-inference` 授權項），但 token 端點只收 `client_secret_basic`／`client_secret_post`，純靜態網頁不能放 secret，本案不用（見「觀察」）。
- 模型頁的 API-Inference 區塊標每次扣多少魔粒：Qwen3.5-397B-A17B、GLM-5.2 都是 2 魔粒／次（旗艦）。
- **文件沒寫、照推定處理**：魔粒用完、未實名、內容審核時回什麼狀態碼與訊息。推定為 401／403／429 加 OpenAI 形狀本文，文案只說「可能」。

**矽基流動**
- 錯誤本文是根層 `{"code":30014,"message":"Token is invalid.","data":null}`。
- 官方 FAQ：收費版模型名前加 `Pro/`；免費模型限速固定；401＝金鑰錯、403＝餘額不足或權限不夠、429＝限速、503/504＝負載高。
- `/v1/models` 要金鑰，回傳沒有價格。
- 官方價格頁（siliconflow.cn/pricing，展開全部）2026-11-02 標「免费」的對話模型：`XingChenAGI/Xing4.0-29B`、`Qwen/Qwen3-8B`、`THUDM/GLM-4-9B-0414`、`THUDM/GLM-Z1-9B-0414`、`deepseek-ai/DeepSeek-R1-0528-Qwen3-8B`、`Qwen/Qwen2.5-7B-Instruct`、`Qwen/Qwen3.5-4B`、`tencent/Hunyuan-MT-7B`。同名模型另有 `Pro/`、`LoRA/` 前綴的收費版。

## 供應商抽象

傳輸層與換模本來就是 OpenAI 相容的通用碼，只把「金鑰從哪來、模型名單從哪來、額度怎麼查、要帶什麼請求頭」抽成介面；OpenRouter 的 OAuth、目錄、排行、上游、`/key` 原封不動留在 OpenRouter 專屬路徑。

- 共用（不改或小改）：`stream-chat.ts`、`sse.ts`、`runaway-guard.ts`、`failover.ts`（FailoverRuntime）、`smart-call.ts`（runSmartCall）、`api-failure.ts`、`key-store.ts`。
- OpenRouter 專屬（不動）：`oauth.ts`、`catalog.ts`、`free-pool.ts`、`openrouter-api.ts`、`ConnectPanel.tsx`、`useOpenRouterSession.ts`。
- 新介面 `AiSession`（`useChat`／`ChatView`／`App` 只認它，取代 `OpenRouterSession` 型別）：
  - `provider`、`apiKey`、`deps`（fetch＋apiBase）、`headers`、`runtime`、`quota`／`quotaEvent`／`refreshQuota`、`logout`；
  - `pool: ModelPool`＝`{ refresh(key, now), plan(holds, now), contextLength(id), tokenizer(id) }`，`FreePool` 已符合；
  - `dailyCheck(key): Promise<FreeDaily|null>`：OpenRouter 打 `/key`，其他家固定回 null；`useChat` 兩處直接 import 的 `fetchFreeDaily` 改走它。
- `streamChat` 改收 `headers`：只有 OpenRouter 帶 `X-Title`（魔搭預檢不放行它）。
- 中國供應商的選模＝`FixedPool`：
  - 自動：名單＝設定檔寫死的排序 ∩ 執行期 `/v1/models` 回得到的 ID（過濾下架；抓不到就用整份寫死名單）。換模、重送照用 FailoverRuntime／runSmartCall，並沿用既有的忙線紀錄與冷卻。
  - 手選：名單只有玩家選的那一支，`others` 為空——失敗就報錯，不換別支。
  - `contextLength`／`tokenizer`：寫死名單每支附值；手選的名單外模型一律用保守值 32k。
- 落點：新開 `web/src/features/provider/`（供應商設定檔、預設與選擇、FixedPool、金鑰型 session、模型選單、貼令牌面板，約 7 支＋測試）。`features/openrouter/` 的通用傳輸層本案不搬，記進「觀察」〔模型判斷·未裁決〕。

## 供應商設定與預設

- 設定檔 `providers.ts` 一份表，每家：`id`、顯示名、`apiBase`（經 `resolve.ts`，e2e 才可覆寫）、註冊與令牌頁網址、引導步驟的文案鍵、寫死名單（每支 `id`、顯示名、`contextLength`、`tokenizer`）、付費判定函式、`headers`。本案三家：`openrouter`（沿用現有路徑）、`modelscope`、`siliconflow`。
- 選擇存 `localStorage` 的 `tt-web:provider`。**沒選過**才看預設：已存有 OpenRouter 金鑰的舊玩家＝`openrouter`（不把既有簡體玩家踢出登入）；否則語系 zh-CN＝`modelscope`，其他語系＝`openrouter`。選過就以存的為準，換語系不改供應商。
- 一鍵路線：簡體玩家打開就是魔搭的貼令牌面板，貼完直接開聊，模型照寫死名單自動選。
- 展開路線（面板下方「更多设置」）：
  - 供應商單選（OpenRouter／魔搭／矽基流動），存 `tt-web:provider`；
  - 模型：「自动（推荐）」或從 `/v1/models` 清單手選一支，存 `tt-web:model:<provider>`。登入後在頂欄供應商名稱旁可以打開同一個展開面板改模型與供應商；換供應商不清任何一家的令牌。回合進行中（`busy`，含串流與等 MVU 處理回覆）這兩個選項停用，提示「回复完成或按停止后再切换」；切換規則見「切換供應商或模型時的回合」。
- 手選模型只給中國供應商；OpenRouter 網頁版維持只用免費模型（OpenRouter 付費要儲值，跟網頁版定位不同）〔模型判斷·未裁決〕。

## 預設模型排序（作者裁決 1、3）

魔搭寫死名單，挑選規則：
1. 只收通用對話大模型；排除生圖、視覺、SQL、醫療、評審、`early-access`、`Preview`、mini／Lite 與 1B 級以下的模型。
2. 排除帶日期快照的 ID（`-0731`、`-1017` 這類），快照下架得比主線名稱早。
3. 每家廠商最多一支，讓第一支掛掉時換到的是另一家的後端。
4. 角色扮演靠中文創作與長上下文：同家取最大的那支通用模型。

**魔搭定案名單**（2026-11-02 公開 `/v1/models` 37 支，依上述規則）：

| 順序 | 模型 | 官方模型頁的上下文 | 寫進設定的 `contextLength` |
|---|---|---|---|
| 1 | `deepseek-ai/DeepSeek-V4.1-Flash` | 1M | 131072 |
| 2 | `Qwen/Qwen3.5-397B-A17B` | 原生 262,144 | 131072 |
| 3 | `ZhipuAI/GLM-5.2` | 1M | 131072 |
| 4 | `MiniMax/MiniMax-M3` | 百萬級 | 131072 |

- 排除理由：DeepSeek 的 `-0731`／`-1017`／`-0813` 是快照（規則 2）；`DeepSeek-V4-Pro` 是上一代，同家取 V4.1（規則 3、4）；Qwen 系列取最大的 397B；`Qwen3.8-*` 只有 27B 與 Flash 版，比 397B 小；ERNIE、Intern、Step、LongCat、Nex 是次選廠商，列在名單外給手選；`Mistral-Large-2407` 中文創作較弱；其餘是專用或小模型（規則 1）。
- `contextLength` 一律取 131072：四支原生都在 262k 以上，但平台免費端點實際放多長文件沒寫，長提示也拖慢回覆；128k 對角色卡（含世界書）已足夠。上下文爆掉時既有的 `AI_CONTEXT_TOO_LONG` 路徑照常處理。
- `tokenizer` 填 null（照 ST 退回估算）。
- 四支都可能預設先思考：回覆會先串流思考內容，既有的思考失控上限照用；每次扣的魔粒是固定的，不因思考變多。不送 `enable_thinking` 這類非標準參數（各家寫法不同，沒實測）。

**矽基流動定案名單**（官方價格頁標「免费」的對話模型，同規則）：

| 順序 | 模型 | `contextLength` |
|---|---|---|
| 1 | `XingChenAGI/Xing4.0-29B` | 32768 |
| 2 | `Qwen/Qwen3-8B` | 32768 |
| 3 | `THUDM/GLM-4-9B-0414` | 32768 |

- 排除理由：`GLM-Z1-9B`、`DeepSeek-R1-0528-Qwen3-8B` 是推理模型，回覆慢（規則 1）；`Hunyuan-MT-7B` 是翻譯模型；`Qwen3.5-4B` 太小；`Qwen2.5-7B` 與 Qwen3-8B 同家，取較新的（規則 3）。
- `contextLength` 一律 32768：小模型原生上下文多半是 32k，官方頁沒逐支標；寧可短。
- 品質落差：最大的一支只有 29B，長篇角色卡和 MVU 格式的遵守度很可能比魔搭差，所以只當第二家可選，不當簡中預設。

## 失敗不升付費（作者裁決 2）與付費標示（作者裁決 4）

自動路線的可驗條件（各有單元測試）：
1. 寫死名單任何一支不得以 `Pro/`、`LoRA/` 開頭，不得含 `:`（魔搭的外部提供方後綴）。測試掃整份表。
2. FixedPool 的候選只從寫死名單取；`/v1/models` 只用來刪，不用來補。
3. `runSmartCall` 的 `others` 只含名單內其餘模型。
4. 名單全失敗就報錯停下，不另找模型。

手選路線：
- 魔搭：選單只列 `/v1/models` 回的 ID（都不帶 `:` 後綴），每次呼叫只扣魔粒、不扣錢，不標付費；ID 含 `:` 的一律不列。
- 矽基流動：API 不給價格，只能保守標示——寫死免費名單內標「免费」，其他全部標「可能收费，会扣你在硅基流动的余额」（`Pro/`、`LoRA/` 開頭的確定收費，標「收费」）。選到非免費那支時，選單下方常駐顯示這句。
- 手選的那支失敗就報錯，不自動換回免費名單，也不換別支。

殘餘風險：
- 魔搭：寫死模型不會「被改成收費而扣錢」，因為 API-Inference 只扣魔粒；最多是被平台改成旗艦檔、魔粒用得較快。不成立為金錢風險。
- 矽基流動：排除 `Pro/` 與 `:` 只保證不會自動選到這類模型，不保證名單內既有模型永遠免費；寫死的免費模型若被官方改成收費，在我們發版更新名單前會照送，帳上有餘額（含贈金）的玩家會被扣錢。緩解只有名單跟著發版更新，列已知限制〔模型判斷·未裁決〕。產品文案只說「本站不会自动改用付费模型」，不承諾「永远不扣余额」。

## 貼令牌畫面與引導

一律只說點哪裡，不出現命令列或程式碼；簡中為主，十語系都要有譯文。

- 魔搭（四步，每步一個按鈕開官方頁面，新分頁）：
  1. 注册魔搭（手机号即可，也能用 GitHub 登录）；
  2. 右上角头像 →「绑定阿里云账号」，按页面完成阿里云实名认证（官方要求，没实名用不了）；
  3. 在个人设置里验证邮箱（官方要求，验证后才能用每日赠送的魔粒）；
  4. 打开「访问令牌」页 → 新建（有效期选「长期」）→ 复制，回来粘贴。
  面板另一行說明：每天登录送的魔粒够玩一阵子，大模型每次回复扣 1–2 魔粒，次日刷新。
- 矽基流動（三步）：注册（手机号或微信）→「API 密钥」→「新建 API 密钥」→ 复制 → 粘贴。
- 存之前驗證：矽基流動打一次 `/v1/models`（要金鑰、不花錢），401 就當場說「这把密钥无效」不存；網路失敗照存。魔搭的 `/v1/models` 不驗令牌、驗證得送一次聊天（會扣魔粒），所以不預驗，第一次開口才知道；401 的文案要直接說「令牌无效或还没完成阿里云实名」。
- `checkApiKey(draft, baseUrl)` 改傳該供應商的 apiBase，不再對中國供應商的金鑰誤報「不是 OpenRouter 的」。

## 錯誤訊息

- 錯誤本文的兩種形狀：本案只保證 **HTTP 非 2xx 的錯誤本文**兩種都收——`detailFromBody` 有 `error` 物件照舊（OpenRouter、魔搭），沒有就讀根層 `code`／`message`（矽基流動）。分類仍以 HTTP 狀態為主。
- 串流中途的錯誤（`StreamOutcome.absorb()`）與上下文爆掉判定（`bodySaysTooLong()`）本案維持只認 `error` 物件，不宣稱全面相容；玩家回饋出現根層形狀的串流錯誤或上下文爆掉時，三處改成共用一個解析入口。
- 429 的分類依供應商（`classifyFailure` 與 `dailyEvidence` 多收一個供應商的限速規則，經 `CallEnv` 傳進 `runSmartCall`）：
  - OpenRouter：原行為不變（`x-ratelimit-remaining <= 0` 或 `free-models-per-*` 字樣＝帳號級 `account`，每日證據擋送出）。
  - 中國供應商：沒有明確的帳號級證據就一律 `unknown-rate-limit`（計入換模），`remaining=0` 標頭與每分鐘字樣都不算帳號級證據（各模型各自限速）；`daily` 恆為 false，額度維持 `unknown`、不擋送出。魔粒用完若日後從玩家回饋拿到可辨識的訊息，才加成帳號級證據。
- 文案裡寫死的「OpenRouter」改成 `{provider}` 佔位（`errAuth`、`errRateLimited` 等）。
- 中國供應商情境：
  - 401：魔搭「令牌无效，或还没完成阿里云绑定与实名」；矽基流動「密钥无效或账号被停用，请到 API 密钥页确认后重新粘贴」；
  - 403：「平台拒绝了这次请求（账号受限，或这个模型需要付费）。本站不会改用付费模型。」附官方訊息原文（遮金鑰）；魔搭另加「也可能是今天的魔粒不足」；
  - 429：魔搭「请求太频繁，或今天的魔粒用完了（每天登录会再送）」；矽基流動「请求太频繁，稍等一下再发」。自動路線計入換模（各模型各自限速，換一支有用）；中國供應商被包成 `FREE_MODEL_BUSY`／`ALL_BUSY` 的忙線訊息也帶「也可能是额度不足」；
  - 串流 `finish_reason: content_filter` 或本文明說內容審核 → 既有 `errFiltered`，中國供應商加一句「中国平台会审核内容，部分角色卡可能一直被拒」；
  - 其他 → 既有 `errGeneric` 帶原文。
- 拒答率、封號門檻、魔粒用完、未實名的確切回應都沒實測（作者裁決 6），文案不承諾原因，只說可能；玩家回饋拿到實際回應後再細分。
- `redactKeys` 從只遮 `sk-or-` 擴成：遮任何 `sk-` 開頭的長字串，另外直接把當前供應商的金鑰原字串換掉（魔搭令牌格式文件沒寫，靠原字串比對最穩）。

## 額度

- 魔搭有魔粒但沒有公開的查詢 API；矽基流動只有每分鐘限速。兩家 `quota` 固定 `unknown`、`dailyCheck` 回 null、頂欄不出額度徽章、`QuotaPanel`（導去下載桌面版）不出〔模型判斷·未裁決〕。
- 送出前不查額度（只有 OpenRouter 有 `/key`）。

## CSP 與 GitHub

- `connect-src` 列出所有供應商的 origin（OpenRouter、魔搭、矽基流動）＋ GitHub：`_headers` 是建置時寫出的靜態檔，執行時才選供應商，只能全列。來源去重（e2e 時都是同一個假端點）。
- `site-headers.ts` 從 `resolveEndpoints` 拿全部供應商端點組字串，不再手寫。
- 中國供應商模式：`useLatestRelease` 不發請求，頂欄「下載桌面版」鈕不顯示（作者裁決 5）；匯出時的桌面版導流等其他入口本案不動〔模型判斷·未裁決〕。

## 金鑰儲存

- 每家一格：OpenRouter 沿用 `tt-web:openrouter-key`（舊玩家不用重登），其他家 `tt-web:key:<provider>`。只在本瀏覽器 localStorage，不進網址、存檔、錯誤訊息、日誌；寫不進就只在本分頁有效（沿用現有 `key-store`）。
- 請求只送當前供應商那格金鑰到它自己的 apiBase；一個回合從頭到尾用回合開始時那個 session（金鑰、apiBase、pool、runtime 都在開頭取定，現行 `runTurn` 已是如此）。測試：A 家金鑰永遠不出現在打 B 家的請求頭裡。

## 切換供應商或模型時的回合

- `FailoverRuntime.invalidate()` 只作廢換模計數票，不作廢聊天回合；聊天回合的作廢靠 `useChat` 自己的 `generationRef`，兩者不連動。所以不靠 `invalidate` 保證回合安全。
- 機制：**回合進行中不能切換**。展開面板裡的供應商與模型選項在 `busy` 期間停用（`busy` 從送出前查詢一路涵蓋串流、換模第二發、MVU 處理回覆，到回合收尾的 `finally` 才放開）；所有供應商／模型選擇入口共用同一個 `busy`，按停止本身不立刻解鎖，要等回合收尾。自動存檔是 `busy=false` 之後由 effect 啟動，`finally` 不代表存檔已寫完；本案不新增「等存檔成功才能切換」的門檻。理由：從源頭消除「舊回合晚到結果寫回新供應商的桌」這個競態，比切換時回滾逐字稿、變數與世界書狀態少很多程式。
- 唯一能在回合中改變 session 的另一條路是退出：沿用現有行為（`setGame(null)` 卸載聊天畫面 → `generationRef` 推進並 abort，結果不寫回）。
- 切換只發生在回合之間：桌、逐字稿與已完成的回合原樣保留，下一句起用新的供應商或模型；FailoverRuntime 與 pool 照樣 `invalidate`（重新從名單第 1 支起算）。
- 切到一家**沒存金鑰**的供應商＝離開這張桌（同登出，回到那家的連線畫面；桌在存檔庫，從「接著玩」回來）〔模型判斷·未裁決〕。理由：聊天畫面一旦卸載再重掛，會從開桌時的快照起算，自動存檔又以 `updatedAt` 判新舊，舊快照會蓋掉較新的存檔。
- 離桌後那家的連線畫面加一句「剛才那張桌照常自動存檔了，連上之後到『接著玩』繼續」。
- 設定面板打開才掛（模型清單打開才抓 `/models`），不在每次開站多打一個請求。
- 貼令牌面板與模型選單都依供應商重掛：貼到一半的草稿不會送去另一家，上一家的模型清單不留到新清單抓回來之前。
- 驗收（e2e）：
  - 串流中想切到另一家已存金鑰的供應商：選項是停用的，按停止也不立刻解鎖。停止前已收到的正文依既有規則保留；停止後不再接受增量；收尾後切換，舊請求不得再修改逐字稿、變數與存檔。下一句打的是新供應商、帶新供應商的金鑰。
  - 等 MVU 處理回覆時想切模型：選項仍停用；回合收尾（MVU 更新完）後才能切，切完下一句用新模型，這一回合的變數更新照常保留。
- 退出只清當前供應商那格。

## 測試與 e2e 假端點

- 單元：
  - 供應商預設（語系、舊 OpenRouter 金鑰、已選過）；
  - FixedPool：交集過濾、抓不到用全表、手選單支無換模；
  - 寫死名單掃描（無 `Pro/`、無 `:`）、`others` 不出名單；
  - 矽基流動付費標示；
  - 根層錯誤本文解析、文案依供應商；
  - 金鑰分格與跨家不外洩、`redactKeys`；
  - 429 分類：帶 `x-ratelimit-remaining: 0` 的 429，OpenRouter 仍是 `account`（原行為），中國供應商是 `unknown-rate-limit` 並計入換模、`daily` 為 false；
  - 根層錯誤本文只在 HTTP 錯誤路徑解析。
- 既有 chat 測試的 session 假物件改成 `AiSession`（多 `dailyCheck`、`headers`），屬型別連帶改動。
- e2e：
  - `resolve.ts` 加 `modelscopeApi`、`siliconflowApi`，只在 e2e 模式吃 `VITE_TT_MODELSCOPE_API`、`VITE_TT_SILICONFLOW_API`；
  - `fake-endpoints.mjs` 同一台伺服器加 `/ms/v1/*`（OpenAI 形狀錯誤）與 `/sf/v1/*`（根層形狀錯誤），聊天沿用現有串流假回覆，可切 401／403／429；
  - `run.mjs` 加簡中情境：語系 zh-CN → 看到魔搭面板 → 貼令牌開聊 → 手選一支模型後失敗只報錯不換 → 切到矽基流動、貼錯金鑰被當場擋 → 斷言整段沒有請求帶 `Pro/` 或 `:` 模型、沒打 GitHub、頂欄沒有下載鈕、A 家金鑰沒送去 B 家；
  - `run.mjs:45` 的 connect-src 斷言改成比對去重後的來源集合。
- 測試通道（test-harness）不用：本案只動網頁版。

## 施工順序

1. 抽 `AiSession` 介面、`dailyCheck`／`headers`、429 的供應商規則，OpenRouter 改接（行為不變、既有測試全綠）。
2. 供應商設定、預設、FixedPool、金鑰型 session、錯誤解析與文案。
3. 畫面（貼令牌面板、展開設定、模型選單、頂欄、回合中停用切換）：先拍現況截圖給作者看，再改。
4. CSP、GitHub 請求、下載鈕、e2e。

## 核心行數估計與結構檢查

估計（核心，不含測試與譯文）：

| 項目 | 行數 |
|---|---|
| `providers.ts`（三家設定、名單、付費判定） | 65 |
| 預設與選擇（供應商、模型） | 40 |
| FixedPool（自動＋手選） | 55 |
| 金鑰型 session＋選擇 hook | 80 |
| `AiSession` 抽介面與 useChat／App 接線 | 30 |
| 錯誤解析與文案代入 | 35 |
| 429 的供應商規則（`classifyFailure`／`dailyEvidence` 經 `CallEnv`） | 15 |
| 回合中停用切換（聊天畫面回報 `busy` 給頂欄） | 15 |
| 貼令牌面板＋展開設定＋模型選單 | 105 |
| 矽基流動存前驗金鑰 | 15 |
| CSP／resolve／GitHub 跳過／藏下載鈕 | 20 |
| **合計** | **約 475** |

動到的檔案現行行數（2026-11-02 main）：

| 檔案 | 行數 |
|---|---|
| web/e2e/run.mjs | 550 |
| web/src/features/chat/useChat.ts | 535 |
| web/src/features/chat/ChatView.tsx | 287 |
| web/src/features/openrouter/api-failure.ts | 209 |
| web/src/features/openrouter/stream-chat.ts | 190 |
| web/e2e/fake-endpoints.mjs | 158 |
| web/src/App.tsx | 124 |
| web/src/features/openrouter/useOpenRouterSession.ts | 123 |
| web/site-headers.ts | 116 |
| web/src/features/openrouter/ConnectPanel.tsx | 57 |
| web/src/features/chat/error-text.ts | 26 |
| web/src/shared/endpoints/resolve.ts | 24 |
| web/src/features/funnel/useLatestRelease.ts | 15 |
| web/src/shared/endpoints/origins.ts | 12 |
| web/src/i18n/*.ts（十語系） | 209–224 |

結構檢查：`node scripts/check-structure.mjs` 綠。`features/openrouter/` 20 檔已到分類門檻（見「觀察」）；新 `features/provider/` 約 7 支＋測試；useChat 淨增預計 <10 行；`run.mjs` 會逼近 700 行，加的簡中情境若超過 120 行就拆成 `e2e/cn-provider.mjs`。

## 已知限制

- 兩家都沒做真實端對端驗證（作者裁決 6）：沒用真令牌打過聊天，名單、上下文長度、錯誤分類、文案都照官方文件與公開清單推定，只用假端點測；上線後依中國玩家回饋修。
- 魔粒用完、未實名、內容審核、封號的實際回應未知，文案只說「可能」。
- 魔粒用完若回 429：照暫時限速處理，會在多次送出之間累積失敗、逐步換遍名單，最後落到既有的忙線紀錄與冷卻；不擋送出。若回 401／403：照帳號錯誤處理，直接停下報錯。
- 魔搭四支可能預設先思考，回覆較慢；沒實測、不送關閉思考的參數。
- 魔搭免費併發與額度由平台動態調整，名單模型可能隨時下架（`/v1/models` 過濾能避開已下架的）。
- 矽基流動：只保證不會自動選到 `Pro/`、`LoRA/`、帶 `:` 的模型，不保證名單內既有模型永遠免費（見「殘餘風險」）。
- 拒答率沒實測（作者裁決 2026-11-02 跳過實驗）。

## 觀察

- `features/openrouter/` 一半是通用傳輸層（stream-chat、sse、api-failure、failover、smart-call、key-store），之後該搬到通用位置；本案不做。
- 魔搭 OAuth 有 `api-inference` 授權項，能做到「一鍵登入」不必貼令牌；但 token 端點要 client secret，得有後端（例如 Cloudflare Worker）代換。另有 `registration_endpoint` 與 `client_id_metadata_supported`，是否支援公開客戶端未查。值得之後單獨評估。
- 找卡頁連的外站（角色卡網站）在中國可能連不上，本案不處理。
- `web/src/App.css` 本案後 997 行，逼近 1000 行門檻；下次動樣式時依區塊（頂欄、連線、聊天、下載頁）拆檔。
- 簡中的 `metaDescription`（分享卡片描述）仍寫「用 OpenRouter 免費模型」，本案沒改。
- 施工順序第 3 步寫「先拍現況截圖給作者看再改」，實際依主線交辦只拍改後三張（簡中貼令牌面板、中國供應商聊天畫面、回合中停用的切換選項）。
- 「等 MVU 時切模型」沒有獨立 e2e：MVU 處理在 `runTurn` 的 `finally` 之前，`busy` 涵蓋它；e2e 驗的是串流中停用與停止後才解鎖。
