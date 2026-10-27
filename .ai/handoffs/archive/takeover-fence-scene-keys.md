> 結案 2026-10-28：Sol、Grok 驗收 DONE，squash 進 main。

# takeover-fence-scene-keys — 接管桌平欄的 time／place／present 凍住

分支 `takeover-fence-scene-keys`。

## 問題
先貼開場白（狀態區塊頂層 `时间／地点／在场` 經 `table_key` 落平欄）再介面重構：重構刻意不搬三鍵（`BASE_TABLE_KEYS`），接管回覆又不收圍欄，三鍵凍在開場值。後果（單元測試重現）：換幕照舊 present 結算、主角色卡被收進隱藏區；新面孔永遠不登場（present 有值就不退回正文比對）；增量桌狀態視角裁掉後進場角色；GM「目前狀態」凍住的地點與樹上同義欄位並列，對齊輪還標成帳上真值。卡片介面只讀樹、接管桌不顯示頂部狀態欄、換幕摘要不讀狀態，不受影響。TestCards 31 張有開場白的卡 0 張會寫進三鍵（罕見）。先重構再貼開場白時開場白不收，本來就沒有。

## 做法（方案 A，Sol、Grok 共識）
`refactor/apply.rs` 本次實際寫入非空接管殼（`new_shell` 有值）時，`flat_migration::clear_scene_keys` 刪掉平欄三鍵（刪鍵不寫空字串：present 缺席才是接管桌的「未知」，空字串會被當成沒人在場照樣結算），併入 `moved_table_keys` 讓 `sync_scene_state_tree` 一併從整幕事件快照移除。沒寫殼的套用（介面沒勾、characters、無殼仍收圍欄的增量桌）不清。撤銷重構由收據的 `removed_table` 還原。

## 已知限制
- 撤銷重構只還原檯面平欄，事件快照已無三鍵；撤銷後再收回，三鍵不回快照值（與其他已搬平欄鍵同路；此順序未另測）。
- 已存在的接管桌不遷移（發佈前舊桌不相容）。
- 卡欄位與 place／present 同義、名冊欄位接在場機制不在本案：見 card-field-overlaps-fixed-keys、character-presence。

## 驗證
verify 12 步綠；cargo `refactor::tests::takeover_fence_scene_keys` 5 項＋`takeover_flat_keys` 兩項改期望（place 一併清掉、收回不復活、撤銷還原）。
