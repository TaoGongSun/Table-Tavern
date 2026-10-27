# 非 zod MVU 卡的 schema 檢查（可擴充、必填鍵、模板）

Status: todo

## Summary
上游 MVU 的舊式卡（`$meta`／`$__META_EXTENSIBLE__$`／`$arrayMeta`）由 `generateSchema` 推出 schema：陣列與物件不可擴充時 insert 拒收、必填鍵 remove 拒收、insert 套 template。zod 卡上游 schema 是 any，不查。本地兩種卡都不查。mvu-array-index-path 審查時拆出。

## 已查到的事實（2026-10-27）
- TestCards 沒有帶 `$meta` 的非 zod 實卡：唯一帶 `$meta` 的 DongeonMaster 註冊了 zod schema（本地也判成 zod），上游不查它。
- 上游預設是嚴格的：沒寫 `$meta` 的物件、陣列一律不可擴充，鍵預設必填。照搬會連 GM 常寫的 `/物品/-` 一起擋，和 mvu-array-index-path「頂層 insert 放行」的裁定衝突。
- 上游每批更新後以舊 schema 為底重新調和（`reconcileAndApplySchema`），insert 值自帶 `$meta` 會改 schema；schema 要跨回合保存，不能只從原始 initvar 重算（樹模式匯入時 `$meta` 已剝掉）。
- Marinara Engine 不支援 MVU，沒有可參考的做法。

## 開工時可考慮的方向〔模型判斷·未裁決〕
卡片 initvar 寫了 `$meta` 等標記才啟用 schema，啟用時整張卡照上游規則（含預設嚴格）；沒寫的卡維持現狀。測試卡可用 DongeonMaster 拿掉 zod 腳本的版本。

## Next action
等出現帶 `$meta` 的非 zod 實卡再開工〔作者裁決 2026-10-27〕。
