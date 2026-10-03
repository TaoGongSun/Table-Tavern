# settings-tabs-tab-order — 設定視窗 Tab／Shift+Tab 導航順序

## 現象（2026-10-03 settings-tabs-focus-visible 實測，Playwright WebKit 26.0，Option+Tab＝按鈕參與 Tab）
1. 方向鍵移到未選分頁後按 Shift+Tab（一般與 Option 皆然），會先在選中分頁（tabIndex 0）多停一次，再到 dialog 本身、再跑到 body。
2. 從選中分頁按 Option+Shift+Tab，焦點連三次停在原處。
3. 關閉鈕在 Tab 順序裡夾在分頁列與內容區之間。

## 分類（Sol）
- 第 2 項優先：先對照 main、真 WKWebView、最小 dialog，確認是產品缺陷還是測試輸入／引擎行為；一併查背景控制項能不能拿到焦點。
- 第 1、3 項屬導航改善。

## 下一步
照上述順序查第 2 項，再決定 1、3 怎麼改。
