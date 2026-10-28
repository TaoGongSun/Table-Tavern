> 結案 2026-10-28：純文件結案，撤銷原施工方案（三方共識）。

# 無續聊路的快取到底有沒有真的抓到

Status: done

## Summary

原規格要讓 OpenRouter 的嚴格延續型端點（`stealth/ox-alpha`）吃到快取：把每輪尾巴重播進歷史、加 `StrictExtension` 策略與 chain epoch。2026-10-28 施工前核對後撤銷：該端點已下架、現行組裝已是一般前綴型的標準形狀、尾巴重播會讓多角色桌私設洩給別的角色。結論、證據、驗證範圍與 ox-alpha 歷史證據見 [plans/non-claude-real-cache.md](../../plans/non-claude-real-cache.md)。

與 [vendor-prefix-floor](vendor-prefix-floor.md) 分開：那案是帳本顯示層，本案是組裝機制層。

## 衍生（主線另立案）

- codex 改走 lane resume：單發我方內容零命中，resume 實測與累計 usage 的注意事項在規格檔「codex 數據」節。
- codex 技能清單混進跑團 prompt。
- no-cache-model-optout 前提修正：OpenRouter 對不快取的端點也回 `cached_tokens: 0`。

## Constraints

- 日後再遇嚴格延續型端點，要先重審私設隔離方式，不可直接復活尾巴重播。
- 驗收快取不能只看 `cached_tokens > 0`，要扣掉供應商自帶底線。
