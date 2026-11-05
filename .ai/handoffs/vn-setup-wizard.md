# vn-setup-wizard — VN 開桌精靈：沒想法也能開桌、美術不等人、美術一覽

Status: 進行中（分支 `vn-setup-wizard`；設計見 [plans/vn-setup-wizard.md](../plans/vn-setup-wizard.md)）

## Summary
作者 2026-11-05 需求：只有一點概念的玩家也能靠題材勾選與選填讓 AI 產大綱與角色、改完再生成；立繪只生預設一張、願意可先挑、不願意直接開始背景補；劇情先生成再播；立繪與背景一覽可重生成。

## 案別與估計
大案，分三包：A 精靈（方案估 650）、B 時序（300，驗收）、C 美術一覽（300，背景上傳碰信任邊界，方案也送）。

## 作者裁決〔2026-11-05〕
維持串流照播；本案先做、合 main 後再試玩（實測佇列第 30 項順延）；同地點換時段用色調遮罩、專屬時段圖在一覽手動生成。

## 目前進度
- 包 A 精靈完成並通過驗收（Sol 兩輪＋常識審）：三步精靈在 `src/features/vn/wizard/`，後端 vn.json `preferences`／`setup`、偏好注入、`vn_setup_set`，大廳「設定未完成」與匯入 VN 作品鈕。
- 包 B 資產時序實作完成，待主線驗收：串流中寫完的 `[bg]` 行提早排背景、後端自動定稿、同地點換時段改基底＋色調遮罩（細節見下節）。通道實測截圖在 scratchpad `vn-b2/`。
- 下一步：主線驗收包 B → 送審 → 包 C 美術一覽。

## 包 B 已接／包 C 要接
- 暫定背景：`vn_backgrounds_prefetch(worldId, scene, source, text)`——`useVnCells` 串流中每多一行換行結尾的 `[bg]` 就送一次整段串流文字；後端只看換行結尾的行，接在正式格後補時段（`vn_backgrounds::streamed`），走同一個 `fill` 去重。
- 自動定稿：`vn_sprites::prepare`（命令 `vn_sprites_prepare(worldId, adopt)`，冪等、同桌一次一條）。③ 掛載時排一次；按開始時 `ArtStep` 先把「選了別張」的裝上，再骰中的角色放進 `adopt`（候選一生好就裝）。開工記定稿世代、裝前 `claim_base` 搶位、提交前再驗，玩家手動挑定／上傳／刪除就放棄。事件 `vn-sprite-status` 的 `expression: "base"`（queued／running／done／failed）；`vn_sprites_status` 併入排著、生成中與失敗的定稿（③ 重開、標題畫面靠它補讀）。跑的期間再被呼叫＝跑完再掃一輪，標題畫面「重試」就是再呼叫一次。設定已走完才接著補臉層。`sprite.json` 記 `candidate`（裝的是哪張候選；上傳＝無），③ 據此判斷「挑定這張」與按開始要不要裝，作品包匯出時拿掉。
- 舞台：立繪換新（卡圖／剪影→定稿、換定稿）延到換格才套用並交叉淡入 0.6s，臉層後到淡入 0.3s；夜晚對立繪疊半強度夜色（以身體圖為遮罩的 multiply 層）。
- 背景庫：每地點自動只生一張基底（時段清單第一項＝固定四時段的 day、卡自訂時段的第一個），`backgrounds.json` 每筆 `{name, base}`（舊桌只有地名照讀）。`BgSlot.mode`＝`Base`／`Regenerate`／`Dedicated`。
- 包 C 讀取：`vn_background_read(slug, period)` 回 `{src, period}`，period＝圖實際時段（≠要的＝基底，舞台疊遮罩）；`Library::base_period` 依「記下的基底 → 清單第一項 → 第一張有圖的」決定基底；`Library::shown` 是讀取順序。遮罩色在 `StageBackdrop.tsx` 的 `TINTS`／`tintOf`，一覽的遮罩預覽可直接重用。
- 包 C 生成專屬圖：`vn_background_generate_period(worldId, location, period)`（以同地點既有圖當參考改光線、覆寫該時段）；重生成 `vn_background_regenerate` 由前端帶「目前顯示那張」的實際時段。完成事件 `vn-background-status` done 後，`useBackgrounds` 會重讀同 slug 所有用過的時段。背景清單命令、上傳替換仍待包 C。

## 已知限制
- 撰寫劇本途中關掉精靈又立刻點回來：聖經還沒寫回，會再呼叫一次生成（額度花兩次）。機率低，輕防。
- 自動定稿只在記憶體：重開 app 時在途的那位角色不續生，進精靈 ③ 會再排（已開演的桌要到立繪分頁手動生）。
- 按開始的瞬間候選剛好寫完、前端還沒收到回覆時，那張不會被採用（維持先前的定稿）。機率低。
- 舊桌與作品包匯入的桌：只有某時段圖的地點以那張當基底（例如只有夜景），其他時段疊遮罩在夜景上；新生成的地點不會。
- 暫定 `[bg]` 只比落檔早「該行之後到回覆結束」這段時間（實測 haiku 約 3 秒）；模型把 `[bg]` 寫在開頭時省得多，寫在結尾時幾乎沒差。
