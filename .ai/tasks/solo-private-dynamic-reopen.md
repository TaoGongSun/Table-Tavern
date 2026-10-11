# 單人在場私設動態巨集沒算進重開指紋

Status: todo

## Summary
單人在場的 Claude 線（Hoist::All、不抹尾段）把 `own.private` 提進凍結 system（`transport/turns.rs:271-276`），但重開指紋沒算私設（`turns.rs:298-307`、`chat_assembly.rs:377-383`）。私設每輪以本人視角求值，含 `{{random}}` 等動態巨集時 system 每輪變、補丁疊在不抹的歷史裡。與 gm-line-system-change-restart 同類。

## Next action
未排程。比照 GM 線裁決（system 變動就重開）把私設納入指紋或直接以 system 變動判重開。
