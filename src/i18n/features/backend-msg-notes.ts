// 畫面說明（非錯誤）的後端代碼譯文；鍵是 `be_<code>`。代碼落在重構結果（稽核 detail、未接管 note）、
// 機制帳本 detail、匯入收據名稱、模型清單快取，舊檔裡的中文原樣顯示。
// 參數表在 backend-msg.ts 的 BACKEND_MSG_PARAMS；index.ts 的 t() 把這份併進同一個 MsgKey 空間。
// 「（餘段）」是寫進世界書的條目標題字尾（不翻），各語系照抄，玩家才找得到那條。
const COPY = {
  "zh-TW": {
    be_refactor_drop_rule_carried: "淘汰缺編號或編號不在 1–4，自動退回照搬。",
    be_refactor_drop_rule_leftover: "淘汰缺編號或編號不在 1–4，此段改併入餘段照搬。",
    be_refactor_span_leftover: "此段未獲有效路由，已併入「（餘段）」條目照搬。",
    be_refactor_person_span_invalid: "人物「{name}」mode=clean 但段落引用無效，退回展開佇列。",
    be_refactor_coverage_carried: "此條目未出現在人物／介面／條目分類任何一處，自動補列照搬。",
    be_refactor_signal_no_reason: "結構預掃訊號（{pattern}）落在照搬條目，未附 reason 說明。",
    be_refactor_signal_on_carry: "預掃訊號落在照搬條目",
    be_refactor_carry_reason: "照搬理由：{reason}",
    be_ledger_refactor_mechanism:
      "AI 卡重構產生的機制條目：欄位規則／觸發表由 App 本地執行，說明文留在世界書（唯讀）照常可讀。",
    be_ledger_script_unrecognized: "卡片腳本認不出來，沒轉成觸發表，預設不送模型。",
    be_ledger_scaffold_absorbed: "機制鷹架條目，已由本地機制接管，不再送入提示詞。",
    be_receipt_refactor_apply: "AI 卡重構",
    be_cli_model_alias: "{alias}（官方別名）",
  },
  "zh-CN": {
    be_refactor_drop_rule_carried: "淘汰缺编号或编号不在 1–4，自动退回照搬。",
    be_refactor_drop_rule_leftover: "淘汰缺编号或编号不在 1–4，此段改并入余段照搬。",
    be_refactor_span_leftover: "此段未获有效路由，已并入“（餘段）”条目照搬。",
    be_refactor_person_span_invalid: "人物“{name}”mode=clean 但段落引用无效，退回展开队列。",
    be_refactor_coverage_carried: "此条目未出现在人物／界面／条目分类任何一处，自动补列照搬。",
    be_refactor_signal_no_reason: "结构预扫信号（{pattern}）落在照搬条目，未附 reason 说明。",
    be_refactor_signal_on_carry: "预扫信号落在照搬条目",
    be_refactor_carry_reason: "照搬理由：{reason}",
    be_ledger_refactor_mechanism:
      "AI 卡重构产生的机制条目：字段规则／触发表由 App 本地执行，说明文留在世界书（只读）照常可读。",
    be_ledger_script_unrecognized: "卡片脚本认不出来，没转成触发表，默认不发给模型。",
    be_ledger_scaffold_absorbed: "机制脚手架条目，已由本地机制接管，不再放进提示词。",
    be_receipt_refactor_apply: "AI 卡重构",
    be_cli_model_alias: "{alias}（官方别名）",
  },
  en: {
    be_refactor_drop_rule_carried:
      "The drop has no rule number, or it isn't 1–4, so the entry was carried over as-is.",
    be_refactor_drop_rule_leftover:
      "The drop has no rule number, or it isn't 1–4, so this span was merged into the leftover entry as-is.",
    be_refactor_span_leftover:
      "This span had no valid route, so it was merged into the “…（餘段）” entry as-is.",
    be_refactor_person_span_invalid:
      "“{name}” uses mode=clean but references invalid spans; sent back to the expand queue.",
    be_refactor_coverage_carried:
      "This entry wasn't classified anywhere (characters, interface or entries), so it was added back as-is.",
    be_refactor_signal_no_reason:
      "A structure prescan signal ({pattern}) landed in a carried entry with no reason given.",
    be_refactor_signal_on_carry: "Prescan signal landed in a carried entry",
    be_refactor_carry_reason: "Reason for carrying: {reason}",
    be_ledger_refactor_mechanism:
      "Mechanism entry made by the AI card refactor: field rules and trigger tables run locally in the app; the description stays readable in the World Book (read-only).",
    be_ledger_script_unrecognized:
      "The card script wasn't recognized, so it wasn't turned into a trigger table and isn't sent to the model by default.",
    be_ledger_scaffold_absorbed:
      "Mechanism scaffold entry, now handled by the app's local mechanism and no longer sent in the prompt.",
    be_receipt_refactor_apply: "AI card refactor",
    be_cli_model_alias: "{alias} (official alias)",
  },
  ja: {
    be_refactor_drop_rule_carried:
      "除外ルールの番号がないか 1〜4 以外のため、そのまま引き継ぎました。",
    be_refactor_drop_rule_leftover:
      "除外ルールの番号がないか 1〜4 以外のため、この段落は残り段落の項目にまとめてそのまま引き継ぎました。",
    be_refactor_span_leftover:
      "この段落は有効な振り分け先がないため、「…（餘段）」項目にまとめてそのまま引き継ぎました。",
    be_refactor_person_span_invalid:
      "人物「{name}」は mode=clean ですが段落の参照が無効なため、展開待ちに戻しました。",
    be_refactor_coverage_carried:
      "この項目は人物・インターフェース・項目のどの分類にも含まれていないため、自動でそのまま追加しました。",
    be_refactor_signal_no_reason:
      "構造の事前スキャン信号（{pattern}）がそのまま引き継ぐ項目にあり、理由が付いていません。",
    be_refactor_signal_on_carry: "事前スキャン信号がそのまま引き継ぐ項目にあります",
    be_refactor_carry_reason: "引き継ぎの理由：{reason}",
    be_ledger_refactor_mechanism:
      "AI カード再構成で作られたメカニクス項目です。項目ルールとトリガー表はアプリ内で処理し、説明文はロアブックに（読み取り専用で）残っています。",
    be_ledger_script_unrecognized:
      "カードのスクリプトを認識できなかったため、トリガー表に変換せず、既定ではモデルに送りません。",
    be_ledger_scaffold_absorbed:
      "メカニクスの足場項目です。アプリ内のメカニクスが引き継いだため、プロンプトには入れません。",
    be_receipt_refactor_apply: "AI カード再構成",
    be_cli_model_alias: "{alias}（公式エイリアス）",
  },
  ko: {
    be_refactor_drop_rule_carried:
      "제외 규칙 번호가 없거나 1–4가 아니어서 그대로 옮겼습니다.",
    be_refactor_drop_rule_leftover:
      "제외 규칙 번호가 없거나 1–4가 아니어서 이 단락은 남은 단락 항목에 합쳐 그대로 옮겼습니다.",
    be_refactor_span_leftover:
      "이 단락은 유효한 배정처가 없어 “…（餘段）” 항목에 합쳐 그대로 옮겼습니다.",
    be_refactor_person_span_invalid:
      "인물 “{name}”은(는) mode=clean이지만 단락 참조가 잘못되어 전개 대기열로 되돌렸습니다.",
    be_refactor_coverage_carried:
      "이 항목은 인물·인터페이스·항목 어느 분류에도 없어서 자동으로 그대로 추가했습니다.",
    be_refactor_signal_no_reason:
      "구조 사전 검사 신호({pattern})가 그대로 옮길 항목에 있는데 이유가 붙어 있지 않습니다.",
    be_refactor_signal_on_carry: "사전 검사 신호가 그대로 옮길 항목에 있음",
    be_refactor_carry_reason: "그대로 옮긴 이유: {reason}",
    be_ledger_refactor_mechanism:
      "AI 카드 재구성으로 만든 메커니즘 항목입니다. 필드 규칙과 트리거 표는 앱에서 직접 처리하고, 설명은 월드북에 (읽기 전용으로) 남아 있습니다.",
    be_ledger_script_unrecognized:
      "카드 스크립트를 인식하지 못해 트리거 표로 바꾸지 않았으며, 기본적으로 모델에 보내지 않습니다.",
    be_ledger_scaffold_absorbed:
      "메커니즘 뼈대 항목입니다. 앱의 메커니즘이 넘겨받아 더 이상 프롬프트에 넣지 않습니다.",
    be_receipt_refactor_apply: "AI 카드 재구성",
    be_cli_model_alias: "{alias} (공식 별칭)",
  },
  es: {
    be_refactor_drop_rule_carried:
      "El descarte no tiene número de regla o no está entre 1 y 4, así que la entrada se conservó tal cual.",
    be_refactor_drop_rule_leftover:
      "El descarte no tiene número de regla o no está entre 1 y 4, así que este fragmento se unió tal cual a la entrada de sobrantes.",
    be_refactor_span_leftover:
      "Este fragmento no tenía un destino válido, así que se unió tal cual a la entrada «…（餘段）».",
    be_refactor_person_span_invalid:
      "«{name}» usa mode=clean pero hace referencia a fragmentos no válidos; vuelve a la cola de desarrollo.",
    be_refactor_coverage_carried:
      "Esta entrada no aparecía en ninguna clasificación (personajes, interfaz o entradas), así que se añadió tal cual.",
    be_refactor_signal_no_reason:
      "Una señal del análisis previo de estructura ({pattern}) cayó en una entrada conservada sin motivo indicado.",
    be_refactor_signal_on_carry: "Señal del análisis previo en una entrada conservada",
    be_refactor_carry_reason: "Motivo para conservarla: {reason}",
    be_ledger_refactor_mechanism:
      "Entrada de mecánica creada por la reorganización de carta con IA: las reglas de campos y las tablas de disparadores las ejecuta la app; la descripción sigue legible en el Libro del Mundo (solo lectura).",
    be_ledger_script_unrecognized:
      "No se reconoció el script de la carta: no se convirtió en tabla de disparadores y, por defecto, no se envía al modelo.",
    be_ledger_scaffold_absorbed:
      "Entrada de andamiaje de mecánica: ahora la gestiona la mecánica local de la app y ya no se envía en el prompt.",
    be_receipt_refactor_apply: "Reorganización de carta con IA",
    be_cli_model_alias: "{alias} (alias oficial)",
  },
  "pt-BR": {
    be_refactor_drop_rule_carried:
      "O descarte não tem número de regra ou não está entre 1 e 4, então a entrada foi mantida como está.",
    be_refactor_drop_rule_leftover:
      "O descarte não tem número de regra ou não está entre 1 e 4, então este trecho foi juntado como está à entrada de sobras.",
    be_refactor_span_leftover:
      "Este trecho não tinha um destino válido, então foi juntado como está à entrada “…（餘段）”.",
    be_refactor_person_span_invalid:
      "“{name}” usa mode=clean, mas faz referência a trechos inválidos; voltou para a fila de expansão.",
    be_refactor_coverage_carried:
      "Esta entrada não apareceu em nenhuma classificação (personagens, interface ou entradas), então foi adicionada como está.",
    be_refactor_signal_no_reason:
      "Um sinal da pré-análise de estrutura ({pattern}) caiu numa entrada mantida sem motivo informado.",
    be_refactor_signal_on_carry: "Sinal da pré-análise numa entrada mantida",
    be_refactor_carry_reason: "Motivo para manter: {reason}",
    be_ledger_refactor_mechanism:
      "Entrada de mecânica criada pela reorganização do card com IA: as regras de campos e as tabelas de gatilhos rodam no app; a descrição continua legível no Livro do Mundo (somente leitura).",
    be_ledger_script_unrecognized:
      "O script do card não foi reconhecido: não virou tabela de gatilhos e, por padrão, não é enviado ao modelo.",
    be_ledger_scaffold_absorbed:
      "Entrada de estrutura de mecânica: agora é tratada pela mecânica local do app e não vai mais no prompt.",
    be_receipt_refactor_apply: "Reorganização do card com IA",
    be_cli_model_alias: "{alias} (alias oficial)",
  },
  de: {
    be_refactor_drop_rule_carried:
      "Beim Verwerfen fehlt die Regelnummer oder sie liegt nicht zwischen 1 und 4, daher wurde der Eintrag unverändert übernommen.",
    be_refactor_drop_rule_leftover:
      "Beim Verwerfen fehlt die Regelnummer oder sie liegt nicht zwischen 1 und 4, daher wurde dieser Abschnitt unverändert in den Rest-Eintrag übernommen.",
    be_refactor_span_leftover:
      "Dieser Abschnitt hatte kein gültiges Ziel und wurde unverändert in den Eintrag „…（餘段）“ übernommen.",
    be_refactor_person_span_invalid:
      "„{name}“ nutzt mode=clean, verweist aber auf ungültige Abschnitte; zurück in die Ausarbeitungs-Warteschlange.",
    be_refactor_coverage_carried:
      "Dieser Eintrag kam in keiner Einordnung vor (Charaktere, Oberfläche, Einträge) und wurde unverändert ergänzt.",
    be_refactor_signal_no_reason:
      "Ein Signal aus der Strukturvorprüfung ({pattern}) liegt in einem unverändert übernommenen Eintrag, ohne Begründung.",
    be_refactor_signal_on_carry: "Vorprüfungssignal in einem unverändert übernommenen Eintrag",
    be_refactor_carry_reason: "Grund für die Übernahme: {reason}",
    be_ledger_refactor_mechanism:
      "Mechanik-Eintrag aus dem KI-Kartenumbau: Feldregeln und Auslösertabellen laufen lokal in der App; die Beschreibung bleibt im Weltbuch lesbar (schreibgeschützt).",
    be_ledger_script_unrecognized:
      "Das Kartenskript wurde nicht erkannt, nicht in eine Auslösertabelle umgewandelt und standardmäßig nicht an das Modell gesendet.",
    be_ledger_scaffold_absorbed:
      "Mechanik-Gerüsteintrag: wird jetzt von der lokalen Mechanik der App übernommen und nicht mehr im Prompt gesendet.",
    be_receipt_refactor_apply: "KI-Kartenumbau",
    be_cli_model_alias: "{alias} (offizieller Alias)",
  },
  fr: {
    be_refactor_drop_rule_carried:
      "L'exclusion n'a pas de numéro de règle, ou il n'est pas entre 1 et 4\u00a0: l'entrée a été reprise telle quelle.",
    be_refactor_drop_rule_leftover:
      "L'exclusion n'a pas de numéro de règle, ou il n'est pas entre 1 et 4\u00a0: ce passage a été versé tel quel dans l'entrée des restes.",
    be_refactor_span_leftover:
      "Ce passage n'avait pas de destination valide\u00a0: il a été versé tel quel dans l'entrée «\u00a0…（餘段）\u00a0».",
    be_refactor_person_span_invalid:
      "«\u00a0{name}\u00a0» utilise mode=clean mais renvoie à des passages invalides\u00a0; renvoyé dans la file de développement.",
    be_refactor_coverage_carried:
      "Cette entrée n'apparaissait dans aucun classement (personnages, interface, entrées)\u00a0: elle a été ajoutée telle quelle.",
    be_refactor_signal_no_reason:
      "Un signal de la pré-analyse de structure ({pattern}) se trouve dans une entrée reprise telle quelle, sans motif indiqué.",
    be_refactor_signal_on_carry: "Signal de pré-analyse dans une entrée reprise telle quelle",
    be_refactor_carry_reason: "Motif de la reprise\u00a0: {reason}",
    be_ledger_refactor_mechanism:
      "Entrée de mécanique créée par la réorganisation de carte par IA\u00a0: les règles de champs et les tables de déclencheurs tournent dans l'app\u00a0; la description reste lisible dans l'encyclopédie (lecture seule).",
    be_ledger_script_unrecognized:
      "Le script de la carte n'a pas été reconnu\u00a0: il n'a pas été converti en table de déclencheurs et n'est pas envoyé au modèle par défaut.",
    be_ledger_scaffold_absorbed:
      "Entrée d'échafaudage de mécanique\u00a0: désormais gérée par la mécanique locale de l'app et plus envoyée dans le prompt.",
    be_receipt_refactor_apply: "Réorganisation de carte par IA",
    be_cli_model_alias: "{alias} (alias officiel)",
  },
  ru: {
    be_refactor_drop_rule_carried:
      "У отбрасывания нет номера правила или он не от 1 до 4, поэтому запись перенесена как есть.",
    be_refactor_drop_rule_leftover:
      "У отбрасывания нет номера правила или он не от 1 до 4, поэтому этот фрагмент перенесён как есть в запись с остатками.",
    be_refactor_span_leftover:
      "У этого фрагмента нет подходящего назначения, поэтому он перенесён как есть в запись «…（餘段）».",
    be_refactor_person_span_invalid:
      "«{name}» использует mode=clean, но ссылается на неверные фрагменты; возвращено в очередь развёртывания.",
    be_refactor_coverage_carried:
      "Эта запись не попала ни в одну категорию (персонажи, интерфейс, записи), поэтому добавлена как есть.",
    be_refactor_signal_no_reason:
      "Сигнал предварительного анализа структуры ({pattern}) попал в запись, перенесённую как есть, без указанной причины.",
    be_refactor_signal_on_carry: "Сигнал предварительного анализа в записи, перенесённой как есть",
    be_refactor_carry_reason: "Причина переноса: {reason}",
    be_ledger_refactor_mechanism:
      "Запись механики, созданная ИИ-разбором карточки: правила полей и таблицы триггеров работают локально в приложении; описание по-прежнему доступно в книге мира (только чтение).",
    be_ledger_script_unrecognized:
      "Скрипт карточки не распознан: он не превращён в таблицу триггеров и по умолчанию не отправляется модели.",
    be_ledger_scaffold_absorbed:
      "Служебная запись механики: теперь её обрабатывает локальная механика приложения, в промпт она больше не попадает.",
    be_receipt_refactor_apply: "ИИ-разбор карточки",
    be_cli_model_alias: "{alias} (официальный алиас)",
  },
} as const;

type NoteMsgLang = keyof typeof COPY;

export type NoteMsgKey = keyof (typeof COPY)["zh-TW"];
export const NOTE_MSG_MESSAGE_KEYS = Object.keys(COPY["zh-TW"]) as NoteMsgKey[];

export function isNoteMsgKey(key: string): key is NoteMsgKey {
  return key in COPY["zh-TW"];
}

export function noteMsgMessage(lang: NoteMsgLang, key: NoteMsgKey): string {
  return COPY[lang][key];
}
