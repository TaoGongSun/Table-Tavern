# 未重構桌狀態欄 YAML 清單攤平只剩最後一筆

Status: todo

## Summary
未重構桌從原卡狀態區塊解析狀態樹時，`transport/response.rs` 的 `parse_indented_fields`（約 513-516 行）把 `- ` 清單項當成縮排加兩格的普通欄位，同名鍵逐項覆蓋，整個清單塌成最後一筆（實例：人物列表只剩「周掌柜」）。要讓 YAML 清單項各自保留。

## Next action
未排程。
