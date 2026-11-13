# Task
Task-ID: rulebook-5e-single-char-keys
Title: 內建 5e 規則書的單字中日韓關鍵字改成兩字以上
Status: todo
Created: 2026-11-14
案別：小案（核心 0 行，只改 `src/features/dice/rules/srd-5.1.json` 的 key 與 keys_by_lang）。

## Summary
世界書掃描對中日韓是子字串比對，5e 書裡 10 個單字關鍵字（弓、弩、箭、矢、활、拳、踢、掴、毒、턴）會在不相干的句子誤觸發規則條目（例如「毒舌」「一箭之地」）。其餘四本已是兩字以上。改成兩字以上的常見說法即可；標題與內文不動（已加入的桌靠它判定）。來自 ttrpg-builtin-books 施工觀察。
