# onboarding-person-unify — fr／ru 對玩家稱呼統一成 tu／ты

本案範圍原則〔作者裁決 2026-10-03〕：本案整理本地化的各種小缺漏，遇到小問題直接一起修、不停下來問；只有會讓範圍擴大好幾倍的大問題才停下回報。

〔作者裁決 2026-10-03〕主字典 fr／ru 對玩家用 tu／ты，`features/openrouter-onboarding.ts` 等處卻用 vous／вы；以主字典為準統一。入場說明用詞越好懂越好。

## 清點方法

- 用 esbuild 打包 `fr.ts`、`ru.ts` 與 `features/*.ts` 九支的匯出函式，展開全部 fr、ru 字串（各 808 條：主字典 607＋features 201）。
- fr 掃 `vous|votre|vos`、`-ez` 結尾詞（命令式／現在式第二人稱複數）、`faites|dites|êtes|soyez|ayez`。
- ru 掃 `вы|вас|вам|вами|ваш*`、`-те／-тесь` 結尾詞（複數命令式、第二人稱複數現在式）、`уверены|готовы|должны|можете|хотите|будете` 等。
- 命中逐條人工判讀。正則誤中（不是對玩家用敬稱）：fr 6 條（`chez`、`requêtes` 等字面巧合）、ru 19 條（`-те` 結尾的名詞前置格，如 акте／ответе／формате／контексте；`Использовать` 不定式）。這些字串本來就沒有 vous／вы，不列入改寫。
- 判斷不了的類別（泛指、非對玩家、法律／正式文字）：**沒有遇到**。risk 系列是無人稱敘述，`riskAccept` 是玩家第一人稱，都不含 vous／вы。

## 改寫原則

- 只換人稱與相應的動詞、所有格、反身代名詞；句子其他部分、標點、空白（含 fr 既有 NBSP／`\u00a0`）原樣保留。
- ru 過去式與短尾形容詞有性別（вы отменили → ты отменил／отменила；готовы → готов／готова）。改用無人稱或不帶性別的說法，不寫「(а)」。
- fr `connecté`、`averti` 等陽性預設是主字典既有寫法，不動。

## fr（28 條）

### 主字典 `src/i18n/fr.ts`（13）

| 鍵（行） | 原文 | 新文 |
|---|---|---|
| usageCostNote (43) | …, utilisez-les seulement pour comparer. | …, utilise-les seulement pour comparer. |
| refactorModeOptCharactersDesc (265) | L'interface de la carte disparaît ; vous jouez via la conversation multi-personnages de l'app. | L'interface de la carte disparaît ; tu joues via la conversation multi-personnages de l'app. |
| refactorCancelledNotice (277) | Vous avez annulé. Seul ce qui… | Tu as annulé. Seul ce qui… |
| refactorModeMismatch (311) | …(différent de votre choix). Ce résultat a été écarté — relancez la refonte. | …(différent de ton choix). Ce résultat a été écarté — relance la refonte. |
| refactorRerunPlayed (314) | …; réorganisez sur une nouvelle table. | …; réorganise sur une nouvelle table. |
| refactorRerunNoSource (315) | …Réorganisez sur une nouvelle table. | …Réorganise sur une nouvelle table. |
| lobbyNewTableHint (401) | Partir d'une table vide et ajouter vos personnages | Partir d'une table vide et ajouter tes personnages |
| lobbyGenTableHint (402) | Décrivez l'histoire, l'IA prépare la table | Décris l'histoire, l'IA prépare la table |
| lobbyYourTables (403) | Vos tables | Tes tables |
| genCharHintPlaceholder (426) | Quel personnage voulez-vous ? Laissez vide et l'IA improvisera | Quel personnage veux-tu ? Laisse vide et l'IA improvisera |
| genCharParseFail (428) | …Voici le texte original — relancez la génération. | …Voici le texte original — relance la génération. |
| stateJumpHint (442) | …Cliquez pour marquer comme compteur et ne plus être averti. | …Clique pour marquer comme compteur et ne plus être averti. |
| undoLastHint (515) | …; appuyez encore pour remonter, dans cette scène uniquement | …; appuie encore pour remonter, dans cette scène uniquement |

### `features/backend-msg.ts`（1）

| 鍵 | 原文 | 新文 |
|---|---|---|
| needsRepair_outside | …Tous les fichiers d'origine sont toujours là : ouvrez le dossier pour les consulter. | …: ouvre le dossier pour les consulter. |

### `features/openrouter-onboarding.ts`（14，fr 區塊 188–210 行）

| 鍵 | 原文 | 新文 |
|---|---|---|
| title | Connectez OpenRouter et commencez à jouer | Connecte OpenRouter et commence à jouer |
| intro | Connectez-vous une fois et commencez à discuter tout de suite. | Connecte-toi une fois et commence à discuter tout de suite. |
| manualIntro | Utilisez ceci seulement pour conserver une key existante. Le stockage local et le démarrage gratuit sont identiques à la connexion en un clic. | Utilise ceci seulement pour garder une key existante. Le stockage local et le démarrage gratuit sont les mêmes qu’avec la connexion en un clic. |
| cliHint | Vous préférez une CLI\u00a0? Changez dans Paramètres → Connexion AI. | Tu préfères une CLI\u00a0? Change de mode dans Paramètres → Connexion IA. |
| errBrowser | …Autorisez l’ouverture des liens externes puis réessayez. | …Autorise l’ouverture des liens externes puis réessaie. |
| errTimeout | L’autorisation a expiré. Revenez ici et cliquez de nouveau sur « Connecter OpenRouter ». | L’autorisation a expiré. Reviens ici et clique de nouveau sur « Connecter OpenRouter ». |
| errCallback | …Fermez la page et reconnectez-vous ; si le problème persiste, redémarrez Table Tavern. | …Ferme la page et reconnecte-toi ; si le problème persiste, redémarre Table Tavern. |
| errCancelled | Vous avez annulé l’autorisation OpenRouter. Aucun réglage n’a été modifié ; reconnectez-vous quand vous le souhaitez. | Tu as annulé l’autorisation OpenRouter. Aucun réglage n’a été modifié ; reconnecte-toi quand tu veux. |
| errNetwork | …Vérifiez le réseau ou le VPN puis réessayez. | …Vérifie le réseau ou le VPN puis réessaie. |
| errExchange | …Reconnectez-vous une fois ; si l’échec persiste, réessayez plus tard. | …Reconnecte-toi une fois ; si l’échec persiste, réessaie plus tard. |
| errSave | …Vérifiez les droits d’écriture du dossier de réglages puis réessayez. | …Vérifie les droits d’écriture du dossier de réglages puis réessaie. |
| errCrypto | …Redémarrez Table Tavern ; si l’échec persiste, utilisez l’option manuelle de key ci-dessous. | …Redémarre Table Tavern ; si l’échec persiste, utilise l’option manuelle de key ci-dessous. |
| errKeyEmpty | Collez l’API key OpenRouter complète. | Colle l’API key OpenRouter complète. |
| errUnknown | La connexion ne s’est pas terminée. Réessayez ; si l’échec persiste, utilisez l’option manuelle de key ci-dessous. | La connexion ne s’est pas terminée. Réessaie ; si l’échec persiste, utilise l’option manuelle de key ci-dessous. |

`manualIntro` 順手把 conserver→garder、identiques à→les mêmes qu’avec，較口語好懂。

## ru（28 條）

### 主字典 `src/i18n/ru.ts`（13）

| 鍵（行） | 原文 | 新文 |
|---|---|---|
| themePreviewHint (107) | Предпросмотр темы «{name}» — поддержите автора, чтобы сохранить ее. … | Предпросмотр темы «{name}» — поддержи автора, чтобы сохранить ее. … |
| refactorImportBtnHint (250) | …(JSON); выберите нужное (без расхода квоты) | …(JSON); выбери нужное (без расхода квоты) |
| refactorCancelledNotice (277) | Вы отменили запуск. Здесь только то, что… | Запуск отменён. Здесь только то, что… |
| refactorModeMismatch (311) | …(не совпадает с вашим выбором). Результат отброшен — запустите перестройку ещё раз. | …(не совпадает с твоим выбором). Результат отброшен — запусти перестройку ещё раз. |
| refactorRerunPlayed (314) | …— проведите разбор на новом столе. | …— проведи разбор на новом столе. |
| refactorRerunNoSource (315) | …Проведите разбор на новом столе. | …Проведи разбор на новом столе. |
| undoLastImportKept (381) | " (записей мировой книги сохранено: {n} — вы изменили их содержимое)" | " (записей мировой книги сохранено: {n} — в них есть твои правки)" |
| lobbyGenTableHint (402) | Опишите историю, а ИИ накроет стол | Опиши историю, а ИИ накроет стол |
| lobbyYourTables (403) | Ваши столы | Твои столы |
| genCharHintPlaceholder (426) | Какого персонажа хотите? Оставьте пустым, и ИИ придумает сам | Какого персонажа хочешь? Оставь пустым, и ИИ придумает сам |
| genCharParseFail (428) | …Вот исходный текст — просто сгенерируйте ещё раз. | …Вот исходный текст — просто сгенерируй ещё раз. |
| stateEditHint (441) | Нажмите, чтобы изменить | Нажми, чтобы изменить |
| stateJumpHint (442) | …Нажмите, чтобы пометить как счётчик и больше не предупреждать. | …Нажми, чтобы пометить как счётчик и больше не предупреждать. |

`refactorCancelledNotice`、`undoLastImportKept` 改無人稱，避開 ты 過去式的性別。

### `features/backend-msg.ts`（1）

| 鍵 | 原文 | 新文 |
|---|---|---|
| needsRepair_outside | …Все исходные файлы на месте — откройте папку, чтобы посмотреть. | …— открой папку, чтобы посмотреть. |

### `features/openrouter-onboarding.ts`（14，ru 區塊 211–233 行）

| 鍵 | 原文 | 新文 |
|---|---|---|
| title | Подключите OpenRouter и начинайте играть | Подключи OpenRouter и начинай играть |
| intro | Войдите один раз — и сразу начинайте общаться. | Войди один раз — и сразу начинай общаться. |
| manualIntro | Используйте только если хотите оставить существующий key. Сохранение на устройстве и бесплатная стартовая настройка такие же, как при подключении в один клик. | Этот вариант нужен, только если хочешь оставить существующий key. Сохранение на устройстве и бесплатная стартовая настройка — как при подключении в один клик. |
| cliHint | Хотите использовать CLI? Переключитесь в Настройки → AI-подключение. | Хочешь использовать CLI? Переключись в Настройки → Подключение к ИИ. |
| errBrowser | …Разрешите приложению открывать внешние ссылки и повторите попытку. | …Разреши приложению открывать внешние ссылки и повтори попытку. |
| errTimeout | …Вернитесь сюда и снова нажмите «Подключить OpenRouter». | …Вернись сюда и снова нажми «Подключить OpenRouter». |
| errCallback | …Закройте страницу авторизации и подключитесь снова; если это повторяется, перезапустите Table Tavern. | …Закрой страницу авторизации и подключись снова; если это повторяется, перезапусти Table Tavern. |
| errCancelled | Вы отменили авторизацию OpenRouter. Настройки не изменены; подключитесь снова, когда будете готовы. | Авторизация OpenRouter отменена. Настройки не изменены; подключись снова, когда захочешь. |
| errNetwork | …Проверьте сеть или VPN и повторите попытку. | …Проверь сеть или VPN и повтори попытку. |
| errExchange | …Подключитесь ещё раз; если ошибка повторится, попробуйте позже. | …Подключись ещё раз; если ошибка повторится, попробуй позже. |
| errSave | …Проверьте право Table Tavern на запись в папку настроек и повторите попытку. | …Проверь право Table Tavern на запись в папку настроек и повтори попытку. |
| errCrypto | …Перезапустите Table Tavern; если ошибка останется, используйте ручной ввод key ниже. | …Перезапусти Table Tavern; если ошибка останется, используй ручной ввод key ниже. |
| errKeyEmpty | Вставьте полный OpenRouter API key. | Вставь полный OpenRouter API key. |
| errUnknown | Подключение не завершилось. Повторите попытку; если ошибка останется, используйте ручной ввод key ниже. | Подключение не завершилось. Повтори попытку; если ошибка останется, используй ручной ввод key ниже. |

`errCancelled` 改無人稱並把「когда будете готовы」換成不帶性別的「когда захочешь」；`manualIntro` 開頭改「Этот вариант нужен, только если…」。

## 追加：fr.ts 原始 NBSP 改跳脫〔作者裁決 2026-10-03〕

`src/i18n/fr.ts` 字串裡直接寫入的 U+00A0 共 19 個（14 行），全部改寫成 `\u00a0` 跳脫，與上一案五個 confirm 鍵的寫法一致。執行期字串值不變。

| 行 | 鍵 | 位置（原文 NBSP 以 ⍽ 標示） | 處數 |
|---|---|---|---|
| 20 | usageScopeLabel | `afficher⍽?` | 1 |
| 328 | editCardSummary | `«⍽{name}⍽»` | 2 |
| 365 | importCardHint | `JSON)⍽;` | 1 |
| 439 | stateSummaryPresent | `Présents⍽:` | 1 |
| 456 | sceneAdvanceHint | `cet acte⍽:` | 1 |
| 458 | sceneRevertHint | `commencé⍽:` | 1 |
| 460 | sceneSummaryRetryHint | `convaincu⍽?` | 1 |
| 466 | sceneWithTitleVersioned | `({v})⍽:` | 1 |
| 470 | sceneWithTitle | `Acte {n}⍽:` | 1 |
| 476 | castHint | `«⍽{name}⍽»` | 2 |
| 477 | castHintClear | `«⍽{name}⍽»` | 2 |
| 486 | renameNote | `la suite⍽;` | 1 |
| 497 | gmCallOn | `«⍽{name}⍽»` | 2 |
| 499 | composerPlaceholder | `«⍽{name}⍽»` | 2 |

- 判斷為簡單：fr.ts 沒有其他特殊空白（U+202F、U+2009、U+2007、U+200B、U+FEFF 皆 0；U+2060 只以 `\u2060` 跳脫出現）；已有 20 處 `\u00a0` 跳脫是上一案寫的。`check-i18n.mjs` 用 esbuild 打包後讀執行期值，寬度與佔位符檢查不受原始碼寫法影響；沒有測試或腳本比對 fr.ts 原始碼裡的 NBSP。
- 做法：腳本把 fr.ts 中每個 U+00A0 字元換成六字元 `\u00a0`，只動這 14 行。
- 證明輸出不變：改前改後各用 esbuild 打包 fr.ts 匯出 607 個鍵的值存 JSON，逐鍵 `===` 比對須全同；另確認改後檔內 U+00A0 原始字元為 0。整案做完 fr.ts 的 `\u00a0` 跳脫為 155（原有 20＋原始轉跳脫 19＋標點前 90＋« » 內側 26）。

## 追加：fr 標點前空白統一成 NBSP〔作者裁決 2026-10-03〕

範圍：fr 全部字串（`fr.ts` 與 `features/*.ts` 九支的 fr 區塊）。`? ! : ;` 前的一般空白改成 `\u00a0` 跳脫；已是 NBSP 的照舊。

清點（同樣用 esbuild 展開執行期值，逐字元判讀前一字元與是否位於 `{…}` 內）：

| 類別 | 筆數 | 處理 |
|---|---|---|
| 前面是一般空白、不在 `{…}` 內 | 162 | 改成 `\u00a0` |
| 前面已是 NBSP | 16 | 不動 |
| 前面沒有空白 | 0 | 無需決定 |
| 在 `{…}` 內（plural 分支文字） | 2 | 改成 `\u00a0`（見下） |
| URL 的 `:`、時間格式、佔位符內部語法、plural 語法逗號 | 0 | fr 沒有 URL、`\d:\d`、`\w:\w`；plural 語法只用逗號與大括號，不含這四個標點 |

162 筆分布：`fr.ts` 88（`;` 40、`:` 30、`?` 16、`!` 2）；features 74——backend-msg-table 33、backend-msg-ai 15、backend-msg-notes 10、backend-msg 5、openrouter-onboarding 5、backend-msg-updater 3、api-compat 2、smart-free 1。人稱改寫後的新文沿用同樣位置（例如 onboarding 五處 ` ;`）。

plural 分支內 2 處：`fr.ts:327` `unsavedLeaveConfirm` one／other 分支的「Veux-tu vraiment quitter ?」。分支內容是顯示文字，`plural.ts` 只在選擇器與大括號邊界吃空白，分支文字原樣輸出；屬「fr 全部字串統一」範圍，一起改。標點前合計 **164** 處。

改法：
1. 先做人稱與 cliHint 改寫。
2. 腳本只在 fr 範圍的字串字面值內把 `[空白](?=[?!:;])` 換成 `\u00a0`，plural 分支文字照改；同一輪把 19 個原始 NBSP 換成跳脫，並做下節的 « » 內側空白。
3. fr 字串都是單一字面值、沒有 `+` 串接，替換不會跨字面值。

驗證「除了這些空白，執行期其他字元都沒變」（標點前與 « » 內側兩項合併驗）：
- 步驟 1 後匯出 fr 全部鍵值 A，步驟 2 後匯出 B。
- 把 A、B 的 U+00A0 都正規化成一般空白後，逐鍵 `===` 必須全同。
- 未正規化逐字元比對：差異只能是「空白→U+00A0」，處數須等於 **210**（標點前 164＋« » 內側 46）。
- B 中 `? ! : ;` 前為一般空白、`«` 後與 `»` 前為一般空白的處數皆為 0；fr 原始碼的原始 U+00A0 為 0。
- check-i18n 讀執行期值，NBSP 與空白同寬；`npm run verify` 全綠。

## 追加：fr « » 內側空白統一成 NBSP〔作者裁決 2026-10-03〕

範圍：fr 全部字串（含 `features/*.ts` 的 fr 區塊）中 `«` 後、`»` 前的一般空白，改成 `\u00a0`。上一案五個確認窗的 NBSP＋U+2060 寫法（`worldbookDeleteConfirm`、`undoLastImportConfirm`、`deleteCharacterConfirm`、`deleteTableConfirm`、`renameConfirm`）原樣不動。`»` 之後的空白不在範圍（後接 `? ; :` 的已算在上一節）。

清點：`«` 共 35 個、`»` 共 35 個，內側已是 NBSP 各 12（五個確認窗 7 對＋`editCardSummary`、`castHint`、`castHintClear`、`gmCallOn`、`composerPlaceholder`），內側無空白 0；內側為一般空白 **23 對＝46 處**：

| 檔案 | 鍵 |
|---|---|
| fr.ts（13 對） | themePreviewHint、risk2、cliPermissionNote、apiKeyLooksPasted、apiKeyNotOpenRouter、tierModelApiLabel、tierModelCliLabel、cliCatalogClaude、gmTierLabel、convertEntryDone、genInputPlaceholder、errAuthCli、importCardInterface |
| backend-msg-table（6 對） | be_refactor_shell_conflict、be_refactor_value_mismatch ×2、be_refactor_rule_mismatch ×2、be_refactor_path_conflict |
| backend-msg-notes（2 對） | be_refactor_span_leftover、be_refactor_person_span_invalid |
| backend-msg-ai（1 對） | be_tier_model_missing |
| openrouter-onboarding（1 對） | errTimeout |

驗證併入上一節的逐鍵比對（差異總數 210）。


## 已查不改：`be_refactor_span_leftover` 的「（餘段）」

各語系訊息裡的「（餘段）」引用的是後端實際產生的條目標題：`src-tauri/src/refactor_assemble.rs:448` 以 `format!("{}（餘段）", entry.title)` 產生兜底條目，玩家在條目列表看到的就是這個中文後綴（測試 `refactor_assemble/tests.rs:202、323` 依此標題查找）。訊息要跟實際標題對得上，只改字典會對不上；要在地化得連後端產生標題一起改（需把介面語言帶進重構組裝、改測試），超出本案，不改。

## 追加：cliHint 分頁名對齊實際分頁〔作者裁決 2026-10-03〕

`cliHint` 指路「設定 → AI 連線」，分頁名直接抄主字典 `settingsBtn` 與 `aiTab` 的值，不另譯。fr、ru 本案修（已併入上方表格的 cliHint 新文）：

- fr：`Connexion AI` → `Connexion IA`（`fr.ts:17` aiTab）
- ru：`AI-подключение` → `Подключение к ИИ`（`ru.ts:17` aiTab）

其他語系對照（`openrouter-onboarding.ts` 各語系 cliHint 行 vs 主字典 aiTab；`settingsBtn` 那段全部一致）。不一致的四個語系也在本案修〔作者裁決 2026-10-03〕，只換分頁名，句子其他部分不動：

| 語系 | cliHint 行 | 原文 | 新文（分頁名抄 aiTab） |
|---|---|---|---|
| ko | 107 | CLI를 쓰고 싶다면 설정 → AI 연결에서 전환하세요. | CLI를 쓰고 싶다면 설정 → AI 연동에서 전환하세요.（ko.ts:17） |
| es | 130 | ¿Quieres usar una CLI? Cámbialo en Ajustes → Conexión AI. | ¿Quieres usar una CLI? Cámbialo en Ajustes → Conexión de IA.（es.ts:17） |
| pt-BR | 153 | Quer usar uma CLI? Troque em Configurações → Conexão de AI. | Quer usar uma CLI? Troque em Configurações → Conexão de IA.（pt-BR.ts:17） |
| de | 176 | Lieber eine CLI nutzen? Wechsle unter Einstellungen → AI-Verbindung. | Lieber eine CLI nutzen? Wechsle unter Einstellungen → KI-Verbindung.（de.ts:17） |

zh-TW、zh-CN、en、ja 一致，不需處理。

## 驗收

全部以 esbuild 匯出十語系 × 808 鍵＝8080 個執行期值做快照比對（S0 原始、S1 人稱與分頁名後、S2a 原始 NBSP 轉跳脫後、S2b 空白統一後）：

- 人稱與分頁名（S0→S1）：鍵集合相同；變動的鍵恰為允許清單 60 個（fr 主字典 13、ru 主字典 13、backend-msg 兩語 2、onboarding fr／ru 各 14、ko／es／pt-BR／de cliHint 4），清單外 0、清單內未變 0。原始碼 diff 60 行成對，每行開引號前與結尾完全相同，只動字面值。
- 原始 NBSP 轉跳脫（S1→S2a）：8080 鍵逐鍵完全相等。
- 空白統一（S2a→S2b）：鍵集合相同；非 fr 語系 0 變動；fr 每鍵長度相同；逐字元差異只有 U+0020→U+00A0，共 **210**；改後 fr `? ! : ;` 前與 « 後、» 前的一般空白為 0。
- 上一案五個確認窗（worldbookDeleteConfirm、undoLastImportConfirm、deleteCharacterConfirm、deleteTableConfirm、renameConfirm）的 NBSP＋U+2060 序列保留，S0 與 S2b 值完全相等。
- `src/shared/ui/i18n-plural.test.ts` 補 fr `unsavedLeaveConfirm` 經 `t()`：n＝0、1 走 one、2 走 other、缺參數走 other 保留 `{n}`，分支內 NBSP 原樣輸出。
- 窄視窗：一次性 WebKit 測試（test:webkit 設定，不留檔）在 280、320px 容器套 App.css 的 `.settings.onboarding` 樣式，渲染 fr Onboarding（展開手動 key）、21 條 onboarding 字串，以及 12 條含長 `{name}`（47 字元連字號名）的引號文案，檢查沒有子元素超出容器，結果 0 溢出；偵測器自檢（200 字元不可斷字串）有抓到。原生確認窗字串本案未變，不另測。
- `npm run verify` 全綠：vitest 655、cargo 773、harness 28。
- Sol 驗收同意（2026-10-03）。
