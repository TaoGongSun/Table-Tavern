# Task
Task-ID: refactor-review-detail
Title: 人審面板展開詳細：角色／介面／機制行可展開看內容
Created: 2026-08-10T23:55:00+08:00
Updated: 2026-08-11T09:40:21+08:00
Status: done

## Summary
2026-08-10 orc-cave 真卡實測（B1）發現：展開細看每行只有名字＋出處條目標題，機制區更是只剩標題一行，玩家沒有任何內容可看，無從判斷要不要勾。拍板：三區每行都能展開看詳細內容。

## 規格要點
- 角色行：展開顯示公開設定（public_md）＋私密設定（private_md）全文。
- 機制行：展開顯示欄位規則與觸發表的可讀摘要；一時做不到可讀化的部分至少給 raw。
- 介面行：展開顯示狀態樹欄位概要。
- 順手修樣式：出處灰字與名字目前零間隔黏在一起（實測截圖「巴古克 (Baguk)Baguk」「遊戲介面搬進 appDynamic Day Counter…」），展開 UI 重排時一併處理。

## Next action
2026-08-11 結案：併入 refactor-output-redesign 批次實作（四區 details 展開全文＋出處灰字獨立行），實測通過。


## Constraints
- 純顯示增強：不改勾選語意、不改套用流程、不改契約型別。
