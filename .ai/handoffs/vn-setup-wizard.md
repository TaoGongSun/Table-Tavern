# vn-setup-wizard — VN 開桌精靈：沒想法也能開桌、美術不等人、美術一覽

Status: 進行中（分支 `vn-setup-wizard`；設計見 [plans/vn-setup-wizard.md](../plans/vn-setup-wizard.md)）

## Summary
作者 2026-11-05 需求：只有一點概念的玩家也能靠題材勾選與選填讓 AI 產大綱與角色、改完再生成；立繪只生預設一張、願意可先挑、不願意直接開始背景補；劇情先生成再播；立繪與背景一覽可重生成。

## 案別與估計
大案，分三包：A 精靈（方案估 650）、B 時序（300，驗收）、C 美術一覽（300，背景上傳碰信任邊界，方案也送）。

## 作者裁決〔2026-11-05〕
維持串流照播；本案先做、合 main 後再試玩（實測佇列第 30 項順延）；同地點換時段用色調遮罩、專屬時段圖在一覽手動生成。

## 目前進度
- 包 A 精靈實作完成，待主線驗收：三步精靈在 `src/features/vn/wizard/`（VnWizard 殼、IdeaStep、StoryBuild＋StorySpec、ArtStep、vn-preferences、useArtProgress），大綱預覽與草稿 hook 抽成 `features/lobby/OutlinePreview.tsx`、`useTableGenesis.ts`，聊天桌 `GenerateTableDialog` 只剩組裝。
- 後端：vn.json 加 `preferences`、`setup`（`story`／`art`，按開始遊戲成功清掉）；`generate_table_outline`／`generate_table_expand` 帶 `preferences`（有值＝VN，空白點子放行；聊天桌照舊拒空）；展開建桌時寫偏好、劇透鎖、`setup=story`，名字有填建只有名字的玩家卡並設 `player_card_id`；聖經提示詞讀 vn.json 偏好（首次與重寫劇本共用），生成後章數夾進長度區間、分支照玩家選的；`list_worlds` 帶 `setup`；新命令 `vn_setup_set(worldId, stage, spoilerLock)`。
- 大廳：「你的牌桌」旁加「匯入 VN 作品」鈕（選 .ttvn）；`setup` 有值的桌標「設定未完成」，點進去回精靈對應步驟；按「開始遊戲」先到標題畫面（新遊戲的等待室），露「立繪準備中 n/N」。② 劇本規格層的選擇程度可改（`vn_bible_save` 收 `branching`，改成線性時正規化補章門檻，vn.json 偏好跟著改）。
- 下一步：主線驗收（含通道截圖）→ 送審 → 包 B。

## 包 B 要接的介面
- 設定階段旗標：`vn.json` `setup`（`story`／`art`／缺）＝設定沒走完；精靈換步驟呼叫 `vn_setup_set`，按「開始遊戲」成功傳 `stage: null` 清掉。美術進度不看它。
- 候選排程（包 A 暫用前端）：`wizard/ArtStep.tsx` 在建桌完成、③ 隱藏掛載時對「沒定稿也沒候選」的角色依序呼叫 `vn_sprite_generate`（一次一張），③ 只接續不重排。精靈關掉（含按開始）後這個迴圈不再處理結果：B 要把它換成後端自動定稿工作（候選 → 安裝定稿 → 發 `vn-sprite-status` 的 `queued`／`done` → 補臉層），前端改成只讀狀態。
- 定稿規則（包 A 已做的部分）：按開始時 `ArtStep` 的 `finishRef` 對沒定稿、有目前顯示候選的角色呼叫 `vn_sprite_pick`；失敗就留在 ③、該列顯示原因，再按開始＝重試。B 要補：再骰中按開始＝再骰完用新圖、還沒出圖的離開精靈後續生；自動工作開工與提交都檢查既有定稿與手動操作世代，手動挑定或上傳優先。
- ② 劇本規格層：精靈換步、開始、關閉前自動存未存改動（`BibleTab` 的 `flushRef`）；`vn_bible_save` 在未遊玩桌會清掉凍結副本，首幕照新規格。設定階段寫入後 App 重讀大廳清單。
- 進度顯示：標題畫面 `wizard/useArtProgress.ts` 以 `list_characters`＋`vn_sprites_read_all` 計數，聽 `vn-sprite-status` 的 `queued`／`done` 重算；B 的自動定稿發同一事件即可讓小字跟著動。

## 已知限制（包 A）
- 撰寫劇本途中關掉精靈又立刻點回來：聖經還沒寫回，會再呼叫一次生成（後寫的蓋掉先寫的，額度花兩次）；同理候選生成中關掉再開，那位角色可能多生一張候選。機率低，輕防，包 B 改成後端工作後自然消失。
- ③「挑定這張」在已有定稿且只有一張候選時變灰（視為就是裝上去那張）；上傳定稿後若只有一張 AI 候選，要先再骰才能改用候選。
- 立繪進度（③ 主鈕與標題畫面）只算已安裝定稿；精靈內候選要到按開始時才裝，所以剛建好就開始會顯示 0/N，等包 B 的後端自動定稿補上。
