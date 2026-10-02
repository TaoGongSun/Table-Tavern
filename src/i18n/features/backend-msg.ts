// 後端回傳的訊息代碼（src-tauri/src/ui_msg.rs 的 UiMsg）與需修復原因（commit.rs 的 RepairReason）。
// i18n/index.ts 的 t() 會把這份補充字典與主字典視為同一個 MsgKey 空間；翻譯入口是
// shared/ui/backend-text.ts。`npm run check:i18n` 會從 Rust 抽 code／欄位核對這裡的鍵、佔位符與參數表。
const COPY = {
  "zh-TW": {
    be_io_failed: "讀寫檔案失敗：{error}",
    needsRepair_outside: "這張桌的資料夾組合對不上，無法自動修復。原檔都還在，請打開資料夾查看。",
    needsRepair_rename:
      "修復時改名失敗（檔案可能被佔用）。這次沒有刪除原桌、備份或另存。下次開啟會再試。",
    needsRepair_convert: "格式轉換沒有完成，原桌未改動。下次開啟會再試。",
    needsRepair_missing: "找不到這張桌的主資料夾，只有備份或未完成的操作。無法自動修復。",
    needsRepair_io: "修復時讀寫失敗，原桌、備份與另存都還在：{error}",
  },
  "zh-CN": {
    be_io_failed: "读写文件失败：{error}",
    needsRepair_outside: "这张桌的文件夹组合对不上，无法自动修复。原文件都还在，请打开文件夹查看。",
    needsRepair_rename:
      "修复时重命名失败（文件可能被占用）。这次没有删除原桌、备份或另存内容。下次打开会再试。",
    needsRepair_convert: "格式转换没有完成，原桌未改动。下次打开会再试。",
    needsRepair_missing: "找不到这张桌的主文件夹，只有备份或未完成的操作。无法自动修复。",
    needsRepair_io: "修复时读写失败，原桌、备份与另存内容都还在：{error}",
  },
  en: {
    be_io_failed: "Couldn't read or write a file: {error}",
    needsRepair_outside:
      "This table's folders don't match any state we can repair automatically. All the original files are still there — open the folder to take a look.",
    needsRepair_rename:
      "Renaming failed during repair (a file may be in use). Nothing was deleted — the table, its backup and any saved-aside content are untouched. We'll try again next time you open it.",
    needsRepair_convert:
      "The format conversion didn't finish. The original table is unchanged. We'll try again next time you open it.",
    needsRepair_missing:
      "This table's main folder is missing; only a backup or an unfinished operation is left. It can't be repaired automatically.",
    needsRepair_io:
      "A file couldn't be read or written during repair. The table, its backup and any saved-aside content are all still there: {error}",
  },
  ja: {
    be_io_failed: "ファイルの読み書きに失敗しました：{error}",
    needsRepair_outside:
      "この卓のフォルダ構成が想定と合わないため、自動では修復できません。元のファイルはすべて残っています。フォルダを開いて確認してください。",
    needsRepair_rename:
      "修復中に名前の変更に失敗しました（ファイルが使用中の可能性があります）。卓・バックアップ・退避した内容は削除していません。次に開くときにもう一度試します。",
    needsRepair_convert:
      "形式の変換が完了しませんでした。元の卓は変更されていません。次に開くときにもう一度試します。",
    needsRepair_missing:
      "この卓のメインフォルダが見つかりません。バックアップか未完了の操作だけが残っています。自動では修復できません。",
    needsRepair_io:
      "修復中にファイルの読み書きに失敗しました。卓・バックアップ・退避した内容はすべて残っています：{error}",
  },
  ko: {
    be_io_failed: "파일을 읽거나 쓰지 못했습니다: {error}",
    needsRepair_outside:
      "이 테이블의 폴더 구성이 맞지 않아 자동으로 복구할 수 없습니다. 원본 파일은 모두 남아 있으니 폴더를 열어 확인해 주세요.",
    needsRepair_rename:
      "복구 중 이름 바꾸기에 실패했습니다(파일이 사용 중일 수 있습니다). 테이블, 백업, 따로 저장한 내용은 삭제하지 않았습니다. 다음에 열 때 다시 시도합니다.",
    needsRepair_convert:
      "형식 변환이 끝나지 않았습니다. 원래 테이블은 바뀌지 않았습니다. 다음에 열 때 다시 시도합니다.",
    needsRepair_missing:
      "이 테이블의 주 폴더를 찾을 수 없고 백업이나 끝나지 않은 작업만 남아 있습니다. 자동으로 복구할 수 없습니다.",
    needsRepair_io:
      "복구 중 파일을 읽거나 쓰지 못했습니다. 테이블, 백업, 따로 저장한 내용은 모두 남아 있습니다: {error}",
  },
  es: {
    be_io_failed: "No se pudo leer o escribir un archivo: {error}",
    needsRepair_outside:
      "Las carpetas de esta mesa no coinciden con ningún estado que se pueda reparar automáticamente. Los archivos originales siguen ahí: abre la carpeta para revisarlos.",
    needsRepair_rename:
      "Falló un cambio de nombre durante la reparación (puede que un archivo esté en uso). No se borró nada: la mesa, la copia y el contenido apartado siguen intactos. Se volverá a intentar la próxima vez que la abras.",
    needsRepair_convert:
      "La conversión de formato no terminó. La mesa original no cambió. Se volverá a intentar la próxima vez que la abras.",
    needsRepair_missing:
      "Falta la carpeta principal de esta mesa; solo quedan una copia o una operación sin terminar. No se puede reparar automáticamente.",
    needsRepair_io:
      "No se pudo leer o escribir un archivo durante la reparación. La mesa, la copia y el contenido apartado siguen ahí: {error}",
  },
  "pt-BR": {
    be_io_failed: "Não foi possível ler ou gravar um arquivo: {error}",
    needsRepair_outside:
      "As pastas desta mesa não batem com nenhum estado que dê para reparar automaticamente. Os arquivos originais continuam lá — abra a pasta para conferir.",
    needsRepair_rename:
      "Uma renomeação falhou durante o reparo (talvez um arquivo esteja em uso). Nada foi apagado: a mesa, o backup e o conteúdo guardado à parte continuam intactos. Vamos tentar de novo na próxima vez que você abrir.",
    needsRepair_convert:
      "A conversão de formato não terminou. A mesa original não foi alterada. Vamos tentar de novo na próxima vez que você abrir.",
    needsRepair_missing:
      "A pasta principal desta mesa sumiu; só restam um backup ou uma operação inacabada. Não dá para reparar automaticamente.",
    needsRepair_io:
      "Não foi possível ler ou gravar um arquivo durante o reparo. A mesa, o backup e o conteúdo guardado à parte continuam lá: {error}",
  },
  de: {
    be_io_failed: "Eine Datei konnte nicht gelesen oder geschrieben werden: {error}",
    needsRepair_outside:
      "Die Ordner dieses Tisches passen zu keinem Zustand, der sich automatisch reparieren lässt. Alle Originaldateien sind noch da – öffne den Ordner, um nachzusehen.",
    needsRepair_rename:
      "Beim Reparieren ist das Umbenennen fehlgeschlagen (eine Datei ist eventuell in Benutzung). Es wurde nichts gelöscht – Tisch, Sicherung und beiseitegelegter Inhalt sind unverändert. Beim nächsten Öffnen wird es erneut versucht.",
    needsRepair_convert:
      "Die Formatumwandlung wurde nicht abgeschlossen. Der ursprüngliche Tisch ist unverändert. Beim nächsten Öffnen wird es erneut versucht.",
    needsRepair_missing:
      "Der Hauptordner dieses Tisches fehlt; übrig sind nur eine Sicherung oder ein unvollständiger Vorgang. Eine automatische Reparatur ist nicht möglich.",
    needsRepair_io:
      "Beim Reparieren konnte eine Datei nicht gelesen oder geschrieben werden. Tisch, Sicherung und beiseitegelegter Inhalt sind alle noch da: {error}",
  },
  fr: {
    be_io_failed: "Impossible de lire ou d'écrire un fichier : {error}",
    needsRepair_outside:
      "Les dossiers de cette table ne correspondent à aucun état réparable automatiquement. Tous les fichiers d'origine sont toujours là : ouvrez le dossier pour les consulter.",
    needsRepair_rename:
      "Un renommage a échoué pendant la réparation (un fichier est peut-être utilisé). Rien n'a été supprimé : la table, la sauvegarde et le contenu mis de côté sont intacts. Nouvel essai à la prochaine ouverture.",
    needsRepair_convert:
      "La conversion de format ne s'est pas terminée. La table d'origine n'a pas été modifiée. Nouvel essai à la prochaine ouverture.",
    needsRepair_missing:
      "Le dossier principal de cette table est introuvable ; il ne reste qu'une sauvegarde ou une opération inachevée. Réparation automatique impossible.",
    needsRepair_io:
      "Impossible de lire ou d'écrire un fichier pendant la réparation. La table, la sauvegarde et le contenu mis de côté sont toujours là : {error}",
  },
  ru: {
    be_io_failed: "Не удалось прочитать или записать файл: {error}",
    needsRepair_outside:
      "Папки этого стола не соответствуют ни одному состоянию, которое можно исправить автоматически. Все исходные файлы на месте — откройте папку, чтобы посмотреть.",
    needsRepair_rename:
      "При восстановлении не удалось переименовать файл (возможно, он занят). Ничего не удалено: стол, резервная копия и отложенное содержимое не тронуты. Попробуем снова при следующем открытии.",
    needsRepair_convert:
      "Преобразование формата не завершилось. Исходный стол не изменён. Попробуем снова при следующем открытии.",
    needsRepair_missing:
      "Основная папка этого стола не найдена — остались только резервная копия или незавершённая операция. Автоматическое восстановление невозможно.",
    needsRepair_io:
      "При восстановлении не удалось прочитать или записать файл. Стол, резервная копия и отложенное содержимое на месте: {error}",
  },
} as const;

type BackendMsgLang = keyof typeof COPY;

export type BackendMsgKey = keyof (typeof COPY)["zh-TW"];
export const BACKEND_MSG_MESSAGE_KEYS = Object.keys(COPY["zh-TW"]) as BackendMsgKey[];

export type BackendParamType = "string" | "number" | "boolean";

/** 每個 UiMsg code 的必要參數與型別；跟 ui_msg.rs 的欄位一一對應（check:i18n 核對）。 */
export const BACKEND_MSG_PARAMS: Record<string, Record<string, BackendParamType>> = {
  io_failed: { error: "string" },
};

/** commit.rs 的 RepairReason；鍵是 `needsRepair_<reason>`。 */
export const NEEDS_REPAIR_REASONS = ["outside", "rename", "convert", "missing", "io"] as const;
export type RepairReason = (typeof NEEDS_REPAIR_REASONS)[number];

export function isBackendMsgKey(key: string): key is BackendMsgKey {
  return key in COPY["zh-TW"];
}

export function backendMsgMessage(lang: BackendMsgLang, key: BackendMsgKey): string {
  return COPY[lang][key];
}
