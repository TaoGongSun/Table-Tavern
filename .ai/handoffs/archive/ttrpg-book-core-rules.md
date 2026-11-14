> 結案 2026-11-14：已合 main，Sol 驗收 1 輪 PASS（無必改）。

# ttrpg-book-core-rules — 加了規則書，第一回合就照該套裁決

- 案別：小案（只改書資料旗標與提示詞；核心約 5 行、測試約 35 行）。
- 起因：[ttrpg-builtin-books](../ttrpg-builtin-books.md) 真模型實測（2026-11-14，claude-sonnet-5-5，16 次派送）。四本新書在繁中遊玩時，玩家第一句口語行動都觸發 0 條（4/4），GM 回到預設的 d20 對難度；條目進了提示之後才照該套裁決。另外每場開戰都擲 d20 排先攻，因為戰鬥欄提示寫「先攻從骰池擲」。作者 2026-11-14：「測試結果都由你決定是否符合玩家的使用直覺…要做的話可直接立案做。」

## 做法
- 四本新書各把核心裁決條目標成常駐（`constant: true`，內文一字不動）：Dungeon World「Moves and Dice」（647 字元）；Cairn「Saves」「Attacking & Damage」「Turns (Cairn)」（890）；BRP「Percentile Rolls」「Combat Round Phases」（783）；PF2「Rolling Checks」「Step 4: Degree of Success」（1,515）。常駐條目進 GM 的凍結 system（有快取），不進角色線（`rulebook_tests.rs` 有測試）。
- 戰鬥欄提示（`transport/panel_prompt.rs` 中英兩段）：行動順序怎麼定照本桌規則書，規則書沒寫才各擲 d20；仍要求開打時排出順序（戰鬥層靠它收敵人）。
- 5e 不動〔模型判斷·未裁決〕：模型預設裁決本來就像它；已發佈版加過書的桌也拿不到新旗標。

## 驗證（2026-11-14）
- Sol 驗收 1 輪 PASS、無必改；建議「測試四本參數化並驗 system 全文」未採（不讓程式更簡單）。
- `npm run verify` 17 步全過。
- 真模型重測（claude-sonnet-5-5，新包新桌、與修正前同卡同第一句行動，10 次派送；逐字稿與截圖主線親看，在 `<scratchpad>/ttrpg-books/retest/`）：四本第 1 回合常駐條目都在 GM 的 system 裡（修正前 0 條）。Dungeon World 請玩家擲 2d6+1 並列出 10+／7–9／6- 三檔，開戰不擲先攻、直接排順序；Cairn 擲 d20 敏捷豁免決定誰先、攻擊直接擲傷害；BRP 兩次都是 1d100 對技能百分比，開戰沒擲 d20；PF2 用察覺擲 d20 先攻（照書）。四本戰鬥面板順序非空、敵人列有進來、拒收 0。
- 沒驗到：每本只一桌一場；BRP「依 DEX 排序」GM 沒明說依據；開戰後下一回合輪轉、房間模式沒重測；PF2 差 10 的四級成功度仍沒遇到。

## 已知限制
- 已經加過這四本書的桌（只有未發佈的測試桌）要移除再加一次才有常駐旗標。
- PF2「差 10 以上大成功／大失敗」真模型仍沒遇到觸發機會，未驗。
