> 結案 2026-10-11：已進 main，三方驗收通過。

# solo-private-dynamic-reopen — 單人在場 Claude 線 system 一變就重開

規格與結果：[plans/solo-private-dynamic-reopen.md](../../plans/solo-private-dynamic-reopen.md)。

- 單人在場 Claude 角色線比照 GM 線：凍結 system（含私設）一變就整線重開、不疊補丁；私設含每輪會變的巨集時每輪重開〔作者裁決 2026-10-11〕。
- 單人線任何原因重開與續聊失敗降級都刪被取代的舊 Claude session 檔；共線範圍不變。
- 實機：haiku 單人線五輪，動態私設每輪 `system-changed`、拿掉巨集後續聊、舊檔已刪。
