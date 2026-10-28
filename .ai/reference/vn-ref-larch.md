# VN 模式參考：larch.ink 與 Marinara 對照

VN 模式（[vn-mode](../tasks/vn-mode.md)）開工前的參考資料。調查日 2026-10-28；Larch 只讀公開頁（首頁、wiki、changelog）與社群 repo `yazelin/larch-vn-skill`（MIT，`4e725f4`，2026-09-09；作者讀平台前端 bundle 與市集作品整理的欄位實測，Larch 開發者 yayapipi 帳號有 fork；**Larch 引擎本身未找到公開原始碼**），Marinara 讀 `e5ead06`（2026-10-23）的 docs 與原始碼，只記做法、不抄碼（AGPL，照 [marinara_reference_notes §21](../plans/marinara_reference_notes.md)）。

## 結論摘要

1. Larch 是**無程式 VN 編輯器**（作者預寫卡片＋白板連線分支），AI 是輔助；Marinara 是**全自動 AI GM**（GM 行內標記＋另一個場景模型挑演出）。我們的目標（任何角色卡都能全自動 VN 玩）站在 Marinara 這邊，Larch 適合借「邊界怎麼框」，不適合借整體架構。
2. 最值得借的是 Larch 的 **AI 對話卡四件套**：隱藏目標、素材範圍、多個離開條件（各接一條後續）、最多幾幕。這等於全自動模式每一幕的邊界契約，也是成本上限。
3. 三方（Larch、Marinara、Ren'Py）一致採「**只報變動**」：背景沒指定就沿用、換地點那一拍才寫新背景。和拍板 6 與「重構目的＝省模型輸出」吻合，劇本格式照這樣定。
4. Marinara 的 GM 行內格式 `[名字] [類型] [表情]: "台詞"` 加 `[choices: …]` 與拍板 4、5、7 幾乎同構，可當劇本格式的現成對照組。
5. Marinara 每回合另打一次場景模型挑背景／音樂／音效／運鏡，不建議照抄〔模型判斷·未裁決〕：多一次呼叫，與省輸出原則相反。
6. 資產一律「**從枚舉清單挑**」（Marinara 背景標籤、音樂 `state/genre/intensity` 三層資料夾；Ren'Py／Tyrano 的具名資產）是業界共識，AI 不准自創名稱。音訊案（vn-audio）可直接借 Marinara 的三層分類。
7. Larch 的玩家側 UI 清單（對話框、名牌、選項、快捷鈕、選單、歷史對話、CG 鑑賞）可當 v1 外框的驗收清單。
8. Larch 資料模型（社群 repo）有三個坑值得避開：立繪舞台跨卡延續、要明寫空舞台才會清；`emotion` 只是名牌字、不會換圖；BGM 給空值不會停。我們的格式要讓「換幕清場」「表情直接對圖」「明確停樂」都是預設行為。
9. 我們和業界最大的差距：還沒有**回溯（rollback）＋存檔槽＋已讀判定**三件玩家標配，這些在 Ren'Py 是引擎內建，VN v1 要排進去。

## 三方定位

| | Larch（larch.ink） | Marinara Engine（Game Mode） | Table Tavern（vn-mode 拍板） |
|---|---|---|---|
| 是什麼 | 網頁版無程式 VN／AVG 編輯器，2026-08-12 發 1.0，幾乎每日更新 | 自架 AI RP 引擎的「遊戲模式」，AI GM 跑單人 RPG，有 VN 版面 | 桌面 app，匯入 ST 角色卡，VN 桌＝同一引擎的另一種桌型 |
| 劇情誰寫 | 作者預寫卡片；AI 只在 AI 卡／AI 模式白板即興 | GM 模型每回合即時寫 | GM 模型每回合即時寫 |
| 玩家輸入 | 選項＋作者設的輸入卡（文字／數值／語音） | 自由輸入＋GM 給的選項卡 | 純選項制（拍板 5） |
| AI 費用誰付 | 作者付松果幣（AI 對話卡每幕 2 枚） | 玩家自接 API | 玩家自備（BYOK） |
| 商業模式 | 免費版 100 卡／10 白板、每月 20 松果幣；Pro NT$299／月（NT$2,990／年）無限卡、60 幣、AI 除影片外不扣幣；資產市集；匯出可商用 | 開源 AGPL | 未定 |

## 逐面向比較

| 面向 | Larch | Marinara | 業界 VN 引擎（Ren'Py／TyranoBuilder） |
|---|---|---|---|
| 劇本格式 | 卡片欄位：角色→表情→台詞，進階區設配音、背景、BGM、音效、轉場、特效 | GM 行內：`[名字] [main\|side\|whisper\|thought] [表情]: "台詞"`；選項 `[choices: "A"\|"B"]` | 宣告式指令流：Ren'Py `scene`／`show 角色 表情 at left`／`hide`／`with dissolve`；Tyrano `[bg]`、`[chara_show]`、`[chara_mod face=]` |
| 立繪與表情 | 角色工坊：去背立繪＋表情組；支援 Live2D、Spine；Pro 可 AI 從一張定稿生表情／服裝／姿勢差分；一張對話卡可裝多句，每句可帶自己的舞台、沒帶就沿用卡片層（社群 repo） | 角色的 Facial／Full-body 兩組 sprite，檔名即表情名（`happy_01` 算 happy 變體）；GM 從角色可用表情清單挑，缺就退預設；Game Mode 自動顯示說話者全身像 | 表情是具名枚舉；Ren'Py `layeredimage` 疊合差分（底圖＋眉眼嘴分層） |
| 站位 | 五個具名槽位 `farLeft`／`left`／`center`／`right`／`farRight`，另有縮放、偏移、進場動畫（fade、slide 等 14 種）與待機循環（`breathe`、`sway` 等）；舞台跨卡延續（社群 repo） | 自動排版＋玩家拖曳（Arrange）、預設偏左或偏右、大小與透明度滑桿 | 具名槽位（`left`／`center`／`right`）＋自訂 transform |
| 背景 | 沒指定就沿用前一卡 | 場景模型挑：頂層預設背景＋逐拍覆寫，只在換地點那拍寫；沒合適的可要求生成並存回可重用背景庫；1280×720 | `scene` 換背景同時清場，作者手寫 |
| 運鏡／特效 | 卡片進階區「轉場、特效」 | 場景模型挑固定特效清單（閃白、震動、聚焦、調色等），每回合最多 2 個、3 拍內最多 1 個 | `with` 轉場、ATL 動畫，作者手寫 |
| 音訊 | BGM（含淡出）、音效、配音（上傳／錄音／AI 預錄／即時 AI） | 音樂按 `state/genre/intensity` 三層資料夾，模型只給氛圍提示，引擎確定性挑檔；另有環境音、音效；可接 ElevenLabs 生成並快取 | music／sound／voice 分聲道，作者指定檔 |
| 選項與分支 | 白板連線；連線可設條件（成立的先走，第一條無條件線當預設）；wiki 說選項不符條件時可見但不可點，社群 repo 2026-09-04 讀播放器碼說選項渲染沒有條件判斷，兩者矛盾，以哪個為準未查證 | `[choices]` 標記渲染成選項卡，選了就當玩家輸入送出 | Ren'Py `menu:` 帶條件；跳轉 label |
| 變數與旗標 | 全域／單白板變數；設為、增減、反轉、隨機；台詞用 `{{變數}}` 插值 | 引擎追蹤地圖、隊伍、NPC、物品、任務、時間、天氣 | 一般變數＋跨周目 `persistent` |
| AI 即興邊界 | AI 對話卡：隱藏目標、素材範圍、離開條件（各接一條線）、最多幾幕；或整話設 AI 模式。執行時伺服器每回合收 `{messages, prompt, exitCondition, turn, maxTurns}`，回 `{reply, done}`（社群 repo） | 無幕級上限；靠 session 結束時摘要、新 session 寫銜接 | 無（非 AI） |
| 玩家側標配 | 標題畫面、對話框、名牌、選項、快捷鈕、選單、歷史對話、CG 鑑賞；存檔／auto／skip 未查證 | 重試回合、Checkpoints 回到先前時點、打字機、自動播放、session 歷史回放 | rollback、存讀檔槽、auto-forward、skip 只跳已讀、history 畫面 |
| 畫面比例 | 16:9（預設 1920×1080）或 9:16，比例不合可全顯或裁切填滿 | 隨視窗 | 固定設計解析度縮放 |

## 對照 Table Tavern：值得參考

每條寫：做法 → 採用後會怎樣 → 理由。編號對 [vn-mode](../tasks/vn-mode.md) 拍板。

1. **幕的邊界契約（借 Larch AI 對話卡）**：全自動 VN 每一幕帶「隱藏目標＋離開條件＋最多幾幕」。→ 劇情不會無限原地打轉，到上限就由引擎推去換幕；成本與時長都有天花板。理由：Larch 把這四件當必填，正是全自動最怕的失控點。對應現有換幕與 `scene_budget`（目前管的是 context 容量，不管劇情幕數，要另加一條劇情上限）。
2. **只報變動的劇本語意（三方共識）**：背景、BGM、站位沒寫就沿用，換地點那一拍才寫新值。→ 模型每格輸出更短、格式負擔更小，壞格式時退回沿用不會黑屏。理由：拍板 6 每幕一張背景本來就是 delta，再加上「重構目的＝省模型輸出」。
3. **劇本格式以 Marinara 行內標記當對照組**：一行＝說話者＋類型（台詞／旁白／內心）＋表情枚舉，選項放尾端單獨一行。→ 拍板 4、5、7 的格式可以直接定稿，不必從零設計；寬容解析失敗整段退旁白（拍板 1）照做。理由：Marinara 已在真實模型上跑過這種格式，證明模型穩定寫得出。
4. **表情只能從清單挑，缺圖退中性（三方共識）**：提示詞給「這個角色有哪些表情」，模型寫清單外的名字就退中性。→ 不會有破圖或空位，拍板 7 的預生 4–5 張直接當清單。理由：Marinara 與 Ren'Py 都這樣，AI 自創表情名是已知翻車點。
5. **資產分類用固定三層（借 Marinara 音樂資料夾）**：BGM 依「情境／類型／強度」歸檔，模型只給這三個值，引擎確定性挑檔。→ vn-audio 不必讓模型認得檔名，換曲庫不用改提示詞。理由：模型挑枚舉值比挑檔名可靠得多。
6. **玩家側 UI 驗收清單（借 Larch 標題與對話框設定頁）**：對話框、名牌、選項、快捷鈕（auto／skip／log）、遊戲選單、歷史對話、CG 鑑賞。→ v1 外框有明確完成定義，不漏標配。理由：這份清單和 Ren'Py 預設 screens 一致，是 VN 玩家的基本期待。
7. **選項卡選了就等於送出（借 Marinara）**：點選項直接成為玩家這一輪的輸入。→ 純選項制（拍板 5）不需要另做輸入框流程，「換一組」重骰鈕照拍板放旁邊。理由：最少的 UI 與狀態。
8. **背景生成後存成可重用的地點背景（借 Marinara）**：同一地點第二次回來直接用舊圖。→ 拍板 6 每幕生一張的額度可省下回訪的部分，畫面也前後一致。理由：VN 場景大量回訪同一地點。
9. **運鏡特效設頻率上限（借 Marinara）**：拍板 4 的 `特寫`、`震動` 若由模型下，引擎再擋「每輪最多 N 個」。→ 模型愛濫用特效時畫面不會一直閃。理由：Marinara 在提示詞裡明訂上限，代表沒限就會濫用。
10. **立繪待機微動（借 Larch `breathe` 循環）**：站著的立繪加極小的呼吸縮放。→ 靜態 AI 立繪看起來不像貼圖，純 CSS、零生成成本。理由：社群 repo 觀察到市集作品幾乎每個角色都掛，且「沒掛就是一張不會動的貼圖」。
11. **換幕與 CG 預設清場（避開 Larch 坑）**：換背景或出 CG 時舞台自動清空，要留人才明寫。→ 不會發生 CG 被前一幕立繪擋住。理由：Larch 預設延續，社群 repo 記錄過 CG 被三個立繪蓋掉。
12. **表情枚舉直接對到圖（避開 Larch 坑）**：劇本裡的表情值就是取圖鍵，不另設「標籤」與「圖」兩欄。→ 模型寫一次表情就換圖，不會出現表情字變了圖沒變。理由：Larch 的 `emotion` 只是名牌字，要另寫每句舞台才換圖，作者踩過。
13. **音樂要有明確「停」指令（避開 Larch 坑）**：沿用是預設，靜音用專門指令而非空值。→ 模型想安靜時真的會安靜。理由：Larch 空值被當「沒設」，只能掛一段無聲音軌。
14. **CG 提示詞把「畫風」與「身分要素」分開講（借社群 repo 實測）**：帶定稿立繪當參考圖，明講臉、髮型、服裝、道具必須跟參考圖一致，畫風可另調。→ 補強拍板 8 的一致性三道保險。理由：社群 repo 實測只說畫風時模型會連臉與服裝一起重設計。
15. **條件選項「可見但不可點」（借 Larch）**：留給 [vn-story-flags](../tasks/vn-story-flags.md)，不進 v1。→ 有旗標系統後，玩家能看到錯過的路線、提高重玩動機。理由：v1 沒有旗標，先做會是空殼。

## 對照 Table Tavern：不建議參考

1. **每回合另打一次場景模型挑演出（Marinara）**〔模型判斷·未裁決〕：不採用 → 每輪少一次呼叫、少一處失敗點；代價是背景與音樂提示要由 GM 同一發順手帶。理由：與「不為 AI 感濫用 AI call」「省模型輸出」相反，且拍板已把演出放在劇本格式裡。
2. **作者預寫卡片＋白板編輯器（Larch 本體）**〔模型判斷·未裁決〕：不採用 → VN 桌不提供劇情編輯器，任何角色卡開桌即可玩。理由：我們的差異化是「普通角色卡直接 VN 玩」，做編輯器等於換產品。
3. **作者預付 AI 幣（Larch 松果幣）**〔模型判斷·未裁決〕：不採用 → 維持 BYOK，app 不碰計費。理由：BYOK 已是既定原則，只借「幕數上限即成本上限」的觀念。
4. **玩家自由拖曳立繪站位（Marinara Arrange）**〔模型判斷·未裁決〕：v1 不做 → 站位全照拍板 4 規則，玩家不用調。理由：玩家不想動腦，規則站位已足夠；有需求再加。
5. **Live2D／Spine 動態立繪（Larch）**〔模型判斷·未裁決〕：不做 → 立繪維持靜態圖＋表情差分。理由：需要專用模型檔與授權 SDK，AI 生成管線產不出來。
6. **一次生成整個世界 JSON（Marinara 開局世界生成）**〔模型判斷·未裁決〕：不採用 → 開局沿用角色卡與現有重構流程。理由：Marinara 自己文件承認這步最常失敗、要旗艦模型，與玩家自選模型衝突。
7. **9:16 直式版面（Larch）**〔模型判斷·未裁決〕：v1 不做 → 只做桌面橫式。理由：桌面 app，直式是手機需求。

## 業界這一步怎麼做、我們差在哪

| 業界慣例（Ren'Py／TyranoBuilder；AI VN 通用做法） | 我們現況 | 差距 |
|---|---|---|
| 腳本＝宣告式指令流，每條指令一個動作（換背景、上下場、換表情、播音樂、選單） | 拍板 1、4 定了劇本格式方向，未定稿 | 要把指令集定成固定小表，解析器寬容但只認表內指令 |
| 站位＝具名槽位，表情＝具名枚舉，資產＝具名檔 | 拍板 4 規則站位、拍板 7 表情枚舉 | 方向一致；缺「資產名單由引擎注入提示詞」這一步 |
| rollback：任何一格可往回捲，選項可重選 | 有「收回」與 scene-fork | 收回是整輪粒度，VN 需要逐格回捲顯示（不必重生成） |
| 存讀檔槽＋縮圖 | 桌即存檔，無多槽 | 要決定「存檔槽」是否等於 scene-fork 分支點 |
| skip 只跳已讀、auto-forward 依字數等待 | 無 | 需要逐格已讀標記 |
| history 畫面（回看台詞） | 聊天視圖可看紀錄（拍板 1） | 方向一致，VN 內要有疊加式 log |
| music／sound／voice 分聲道與各自音量 | 無音訊 | vn-audio 案處理 |
| AI VN 產品通用做法：LLM 輸出帶標籤的結構化腳本、資產只能從清單挑、圖片預生快取 | 拍板 7 預生表情、拍板 6 每幕背景 | 方向一致；個別 AI VN 產品的實際做法未查證 |

## 未查證／沒查到

- Larch「AI 模式白板」（整話交給 AI 導演）實際控制哪些演出、記憶與收尾方式：wiki 只有一句說明。
- Larch 玩家側存讀檔、auto、skip、立繪站位：只確認有「歷史對話」與快捷鈕可自訂。
- Larch AI 對話卡用哪個模型、離開條件由誰判定（模型自判或引擎判）。
- 個別 AI VN 產品的做法：沒有實際查任何一家。
- Larch 選項能否依條件鎖住：wiki 與社群 repo 矛盾（見比較表）。
- 社群 repo 是第三方實測，Larch 每日更新，欄位可能已變。
- 頁面與 repo 內容沒有發現要我照做的指示文字（repo 本身是給 AI 用的 skill，只當資料讀）。

## 來源

- Larch：<https://larch.ink/zh>、<https://larch.ink/wiki>（ai-dialogue-and-ai-director、characters-and-dialogue-staging、choices-conditions-variables、story-board-basics、title-screen-and-dialogue-ui、ai-creative-tools、faq-plans-saving-copyright、ai-agent-mcp-skills）、<https://larch.ink/changelog>
- Larch 社群 repo：<https://github.com/yazelin/larch-vn-skill>（`SKILL.md` 卡片、舞台、角色、平台 AI、BGM、條件分支各節）
- Marinara：`docs/game/getting-started.md`、`docs/game/game-assets.md`、`docs/game/storyboard.md`、`docs/characters/sprites.md`、`docs/media/scene-backgrounds.md`、`docs/media/animated-expressions.md`、`packages/server/src/services/sidecar/scene-analyzer.ts`、`packages/server/src/services/game/gm-prompts.ts`、`packages/client/src/components/game/GameChoiceCards.tsx`
