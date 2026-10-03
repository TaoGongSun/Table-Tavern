// AI 連線面（ai_transport、transport/client、responses_transport、smart_free、cli/、lanes）的後端錯誤代碼譯文；鍵是 `be_<code>`。
// 參數表在 backend-msg.ts 的 BACKEND_MSG_PARAMS；index.ts 的 t() 把這份併進同一個 MsgKey 空間。
// 多半顯示在 ErrorNote 的主文字或小字；CLI 原話、路徑、版本號、結束狀態照原文代入。
const COPY = {
  "zh-TW": {
    be_cli_workspace_failed: "無法準備 CLI 工作目錄：{error}",
    be_grok_profile_failed: "無法準備 grok 設定目錄：{error}",
    be_cli_risk_not_accepted: "尚未確認 CLI 訂閱模式的風險告知，請到設定完成確認",
    be_cli_not_found: "找不到 {cli} CLI，請確認已安裝並登入",
    be_agy_too_old: "Gemini CLI {version} 太舊，本 app 需要 1.1.8 以上。請執行 `agy update` 後重新驗證。",
    be_unknown_transport: "未知的連線方式：{transport}",
    be_tier_model_missing: "尚未設定「{tier}」檔位對應的模型，請先到設定填寫",
    be_openrouter_api_key_missing: "尚未設定 OpenRouter API key，請先到設定貼上",
    be_no_free_models: "目前沒有可用的免費模型",
    be_no_stable_free_model: "目前沒有可用的穩定免費模型，請到設定改用其他免費模型或手動選模",
    be_smart_free_daily_exhausted: "OpenRouter 免費模型今日可用次數已用完，請等額度重置後再試",
    be_responses_api_failed: "Responses API 回傳失敗",
    be_cli_reply_error: "CLI 回覆錯誤：{error}",
    be_cli_stdin_timeout: "CLI 60 秒收不進提示詞，已中止",
    be_cli_stalled: "CLI 120 秒沒有任何輸出（網路或程序卡死），已中止",
    be_cli_crashed: "CLI 異常結束（{status}）：{tail}",
    be_cli_no_reply: "CLI 沒有產出回覆（{status}）：{tail}",
    be_cli_turn_failed: "{cli} CLI 回合失敗",
    be_cli_turn_failed_status: "{cli} CLI 回合失敗（{status}）",
    be_cli_unexpected_abort: "內部錯誤：沒有取消訊號卻回報中止",
    be_agy_conversation_mismatch: "Agy 續聊回到不同的對話：預期 {expected}，實際 {actual}",
    be_agy_lock_poisoned: "Agy 回填鎖已損壞",
    be_lane_state_write_failed: "無法寫入續聊線狀態檔 {path}：{error}",
    be_session_abandon_failed: "中止後刪不掉 session 檔 {path}：{error}。這條線已清掉，請再送一次。",
    be_lane_rewrite_unsupported: "{provider} 的續聊線無法在回合後抹掉私設，私設必須放進該角色固定的系統提示詞",
  },
  "zh-CN": {
    be_cli_workspace_failed: "无法准备 CLI 工作目录：{error}",
    be_grok_profile_failed: "无法准备 grok 设定目录：{error}",
    be_cli_risk_not_accepted: "尚未确认 CLI 订阅模式的风险告知，请到设定完成确认",
    be_cli_not_found: "找不到 {cli} CLI，请确认已安装并登录",
    be_agy_too_old: "Gemini CLI {version} 太旧，本 app 需要 1.1.8 以上。请执行 `agy update` 后重新验证。",
    be_unknown_transport: "未知的连接方式：{transport}",
    be_tier_model_missing: "尚未设定“{tier}”档位对应的模型，请先到设定填写",
    be_openrouter_api_key_missing: "尚未设定 OpenRouter API key，请先到设定粘贴",
    be_no_free_models: "目前没有可用的免费模型",
    be_no_stable_free_model: "目前没有可用的稳定免费模型，请到设定改用其他免费模型或手动选模",
    be_smart_free_daily_exhausted: "OpenRouter 免费模型今日可用次数已用完，请等额度重置后再试",
    be_responses_api_failed: "Responses API 返回失败",
    be_cli_reply_error: "CLI 回复错误：{error}",
    be_cli_stdin_timeout: "CLI 60 秒内收不进提示词，已中止",
    be_cli_stalled: "CLI 120 秒没有任何输出（网络或进程卡死），已中止",
    be_cli_crashed: "CLI 异常结束（{status}）：{tail}",
    be_cli_no_reply: "CLI 没有产出回复（{status}）：{tail}",
    be_cli_turn_failed: "{cli} CLI 回合失败",
    be_cli_turn_failed_status: "{cli} CLI 回合失败（{status}）",
    be_cli_unexpected_abort: "内部错误：没有取消信号却回报中止",
    be_agy_conversation_mismatch: "Agy 续聊回到了不同的对话：预期 {expected}，实际 {actual}",
    be_agy_lock_poisoned: "Agy 回填锁已损坏",
    be_lane_state_write_failed: "无法写入续聊线状态文件 {path}：{error}",
    be_session_abandon_failed: "中止后删不掉 session 文件 {path}：{error}。这条线已清掉，请再发送一次。",
    be_lane_rewrite_unsupported: "{provider} 的续聊线无法在回合后抹掉私设，私设必须放进该角色固定的系统提示词",
  },
  en: {
    be_cli_workspace_failed: "Couldn't prepare the CLI working folder: {error}",
    be_grok_profile_failed: "Couldn't prepare the grok settings folder: {error}",
    be_cli_risk_not_accepted:
      "The risk notice for CLI subscription mode hasn't been confirmed yet. Confirm it in Settings.",
    be_cli_not_found: "Couldn't find the {cli} CLI. Make sure it's installed and signed in.",
    be_agy_too_old:
      "Gemini CLI {version} is too old; this app needs 1.1.8 or later. Run `agy update`, then re-verify.",
    be_unknown_transport: "Unknown connection type: {transport}",
    be_tier_model_missing: "No model is set for the “{tier}” tier yet. Fill it in under Settings.",
    be_openrouter_api_key_missing: "No OpenRouter API key yet. Paste one in Settings.",
    be_no_free_models: "No free models are available right now.",
    be_no_stable_free_model:
      "No stable free model is available right now. Pick another free model in Settings, or choose a model manually.",
    be_smart_free_daily_exhausted:
      "Today's OpenRouter free-model requests are used up. Try again after the allowance resets.",
    be_responses_api_failed: "The Responses API reported a failure.",
    be_cli_reply_error: "CLI error: {error}",
    be_cli_stdin_timeout: "The CLI didn't accept the prompt within 60 seconds, so it was stopped.",
    be_cli_stalled:
      "The CLI produced no output for 120 seconds (network or process hung), so it was stopped.",
    be_cli_crashed: "The CLI exited unexpectedly ({status}): {tail}",
    be_cli_no_reply: "The CLI produced no reply ({status}): {tail}",
    be_cli_turn_failed: "{cli} CLI turn failed",
    be_cli_turn_failed_status: "{cli} CLI turn failed ({status})",
    be_cli_unexpected_abort: "Internal error: reported as stopped without a stop signal.",
    be_agy_conversation_mismatch:
      "Agy resumed a different conversation: expected {expected}, got {actual}",
    be_agy_lock_poisoned: "Agy's write-back lock is broken.",
    be_lane_state_write_failed: "Couldn't write the conversation-thread state file {path}: {error}",
    be_session_abandon_failed:
      "After stopping, the session file {path} couldn't be deleted: {error}. This thread has been cleared; please send again.",
    be_lane_rewrite_unsupported:
      "{provider} conversation threads can't erase private settings after a turn; private settings must go into that character's fixed system prompt.",
  },
  ja: {
    be_cli_workspace_failed: "CLI の作業フォルダを準備できませんでした：{error}",
    be_grok_profile_failed: "grok の設定フォルダを準備できませんでした：{error}",
    be_cli_risk_not_accepted:
      "CLIサブスクリプションモードのリスク確認がまだです。設定で確認を済ませてください。",
    be_cli_not_found: "{cli} CLI が見つかりません。インストールとログインを確認してください。",
    be_agy_too_old:
      "Gemini CLI {version} は古すぎます。このアプリには 1.1.8 以上が必要です。`agy update` を実行してから再認証してください。",
    be_unknown_transport: "不明な接続方式：{transport}",
    be_tier_model_missing: "「{tier}」ランクのモデルが未設定です。先に設定で入力してください。",
    be_openrouter_api_key_missing: "OpenRouter の APIキーが未設定です。先に設定で貼り付けてください。",
    be_no_free_models: "現在利用できる無料モデルがありません。",
    be_no_stable_free_model:
      "現在利用できる安定無料モデルがありません。設定で別の無料モデルに切り替えるか、モデルを手動で選んでください。",
    be_smart_free_daily_exhausted:
      "今日の OpenRouter 無料モデルの利用回数を使い切りました。枠がリセットされてから再試行してください。",
    be_responses_api_failed: "Responses API が失敗を返しました",
    be_cli_reply_error: "CLI がエラーを返しました：{error}",
    be_cli_stdin_timeout: "CLI が 60 秒以内にプロンプトを受け取らなかったため中止しました",
    be_cli_stalled:
      "CLI から 120 秒間出力がありませんでした（ネットワークかプロセスが停止）。中止しました",
    be_cli_crashed: "CLI が異常終了しました（{status}）：{tail}",
    be_cli_no_reply: "CLI が返答を出しませんでした（{status}）：{tail}",
    be_cli_turn_failed: "{cli} CLI のターンが失敗しました",
    be_cli_turn_failed_status: "{cli} CLI のターンが失敗しました（{status}）",
    be_cli_unexpected_abort: "内部エラー：中止の合図がないのに中止が報告されました",
    be_agy_conversation_mismatch:
      "Agy の続き会話が別の会話に戻りました：想定 {expected}、実際 {actual}",
    be_agy_lock_poisoned: "Agy の書き戻しロックが壊れています",
    be_lane_state_write_failed: "会話スレッドの状態ファイル {path} に書き込めませんでした：{error}",
    be_session_abandon_failed:
      "中止後に session ファイル {path} を削除できませんでした：{error}。このスレッドはクリア済みです。もう一度送信してください。",
    be_lane_rewrite_unsupported:
      "{provider} の会話スレッドはターン後に非公開設定を消せません。非公開設定はそのキャラクターの固定システムプロンプトに入れる必要があります。",
  },
  ko: {
    be_cli_workspace_failed: "CLI 작업 폴더를 준비하지 못했습니다: {error}",
    be_grok_profile_failed: "grok 설정 폴더를 준비하지 못했습니다: {error}",
    be_cli_risk_not_accepted:
      "CLI 구독 모드의 리스크 고지를 아직 확인하지 않았습니다. 설정에서 확인을 마쳐 주세요.",
    be_cli_not_found: "{cli} CLI를 찾을 수 없습니다. 설치와 로그인 상태를 확인해 주세요.",
    be_agy_too_old:
      "Gemini CLI {version}은(는) 너무 오래되었습니다. 이 앱은 1.1.8 이상이 필요합니다. `agy update`를 실행한 뒤 재인증해 주세요.",
    be_unknown_transport: "알 수 없는 연결 방식: {transport}",
    be_tier_model_missing: "“{tier}” 등급에 연결된 모델이 없습니다. 먼저 설정에서 입력해 주세요.",
    be_openrouter_api_key_missing: "OpenRouter API 키가 아직 없습니다. 먼저 설정에 붙여 넣어 주세요.",
    be_no_free_models: "지금 사용할 수 있는 무료 모델이 없습니다.",
    be_no_stable_free_model:
      "지금 사용할 수 있는 안정 무료 모델이 없습니다. 설정에서 다른 무료 모델로 바꾸거나 모델을 직접 선택해 주세요.",
    be_smart_free_daily_exhausted:
      "오늘의 OpenRouter 무료 모델 사용 횟수를 모두 썼습니다. 한도가 초기화된 뒤 다시 시도해 주세요.",
    be_responses_api_failed: "Responses API가 실패를 반환했습니다",
    be_cli_reply_error: "CLI 오류: {error}",
    be_cli_stdin_timeout: "CLI가 60초 안에 프롬프트를 받지 못해 중단했습니다",
    be_cli_stalled: "CLI가 120초 동안 아무것도 출력하지 않아(네트워크 또는 프로세스 멈춤) 중단했습니다",
    be_cli_crashed: "CLI가 비정상 종료되었습니다({status}): {tail}",
    be_cli_no_reply: "CLI가 답변을 내지 않았습니다({status}): {tail}",
    be_cli_turn_failed: "{cli} CLI 턴이 실패했습니다",
    be_cli_turn_failed_status: "{cli} CLI 턴이 실패했습니다({status})",
    be_cli_unexpected_abort: "내부 오류: 중단 신호 없이 중단이 보고되었습니다",
    be_agy_conversation_mismatch:
      "Agy 이어 하기가 다른 대화로 돌아갔습니다: 예상 {expected}, 실제 {actual}",
    be_agy_lock_poisoned: "Agy 기록 잠금이 손상되었습니다",
    be_lane_state_write_failed: "대화 스레드 상태 파일 {path}에 쓰지 못했습니다: {error}",
    be_session_abandon_failed:
      "중단 후 session 파일 {path}을(를) 지우지 못했습니다: {error}. 이 스레드는 정리했으니 다시 보내 주세요.",
    be_lane_rewrite_unsupported:
      "{provider} 대화 스레드는 턴이 끝난 뒤 비공개 설정을 지울 수 없습니다. 비공개 설정은 해당 캐릭터의 고정 시스템 프롬프트에 넣어야 합니다.",
  },
  es: {
    be_cli_workspace_failed: "No se pudo preparar la carpeta de trabajo de la CLI: {error}",
    be_grok_profile_failed: "No se pudo preparar la carpeta de ajustes de grok: {error}",
    be_cli_risk_not_accepted:
      "Aún no has confirmado el aviso de riesgos del modo de suscripción por CLI. Confírmalo en los ajustes.",
    be_cli_not_found: "No se encontró la CLI de {cli}. Comprueba que esté instalada y con la sesión iniciada.",
    be_agy_too_old:
      "Gemini CLI {version} es demasiado antigua; esta app necesita la 1.1.8 o posterior. Ejecuta `agy update` y vuelve a verificar.",
    be_unknown_transport: "Tipo de conexión desconocido: {transport}",
    be_tier_model_missing: "Todavía no hay modelo para el nivel «{tier}». Rellénalo en los ajustes.",
    be_openrouter_api_key_missing: "Aún no hay API key de OpenRouter. Pégala en los ajustes.",
    be_no_free_models: "Ahora mismo no hay modelos gratuitos disponibles.",
    be_no_stable_free_model:
      "Ahora mismo no hay un modelo gratuito estable disponible. Elige otro modelo gratuito en los ajustes o selecciona un modelo a mano.",
    be_smart_free_daily_exhausted:
      "Se agotaron las solicitudes de hoy a los modelos gratuitos de OpenRouter. Vuelve a intentarlo cuando se renueve el cupo.",
    be_responses_api_failed: "La Responses API devolvió un fallo",
    be_cli_reply_error: "Error de la CLI: {error}",
    be_cli_stdin_timeout: "La CLI no aceptó el prompt en 60 segundos; se detuvo",
    be_cli_stalled: "La CLI pasó 120 segundos sin ninguna salida (red o proceso colgados); se detuvo",
    be_cli_crashed: "La CLI terminó de forma inesperada ({status}): {tail}",
    be_cli_no_reply: "La CLI no produjo ninguna respuesta ({status}): {tail}",
    be_cli_turn_failed: "Falló el turno de la CLI de {cli}",
    be_cli_turn_failed_status: "Falló el turno de la CLI de {cli} ({status})",
    be_cli_unexpected_abort: "Error interno: se informó una detención sin señal de detener",
    be_agy_conversation_mismatch:
      "Agy retomó otra conversación: se esperaba {expected}, llegó {actual}",
    be_agy_lock_poisoned: "El bloqueo de escritura de Agy está dañado",
    be_lane_state_write_failed: "No se pudo escribir el archivo de estado del hilo {path}: {error}",
    be_session_abandon_failed:
      "Tras detener, no se pudo borrar el archivo de sesión {path}: {error}. El hilo ya se limpió; vuelve a enviar.",
    be_lane_rewrite_unsupported:
      "Los hilos de {provider} no pueden borrar los ajustes privados tras el turno; los ajustes privados deben ir en el prompt de sistema fijo de ese personaje.",
  },
  "pt-BR": {
    be_cli_workspace_failed: "Não foi possível preparar a pasta de trabalho da CLI: {error}",
    be_grok_profile_failed: "Não foi possível preparar a pasta de configurações do grok: {error}",
    be_cli_risk_not_accepted:
      "O aviso de risco do modo de assinatura da CLI ainda não foi confirmado. Confirme nas configurações.",
    be_cli_not_found: "A CLI do {cli} não foi encontrada. Confira se ela está instalada e logada.",
    be_agy_too_old:
      "Gemini CLI {version} é antiga demais; este app precisa da 1.1.8 ou mais recente. Rode `agy update` e reverifique.",
    be_unknown_transport: "Tipo de conexão desconhecido: {transport}",
    be_tier_model_missing: "Ainda não há modelo para o nível “{tier}”. Preencha nas configurações.",
    be_openrouter_api_key_missing: "Ainda não há API key do OpenRouter. Cole uma nas configurações.",
    be_no_free_models: "Não há modelos grátis disponíveis agora.",
    be_no_stable_free_model:
      "Não há um modelo grátis estável disponível agora. Escolha outro modelo grátis nas configurações ou selecione um modelo manualmente.",
    be_smart_free_daily_exhausted:
      "As requisições de hoje aos modelos grátis do OpenRouter acabaram. Tente de novo depois que a cota for renovada.",
    be_responses_api_failed: "A Responses API retornou uma falha",
    be_cli_reply_error: "Erro da CLI: {error}",
    be_cli_stdin_timeout: "A CLI não aceitou o prompt em 60 segundos; foi interrompida",
    be_cli_stalled: "A CLI ficou 120 segundos sem nenhuma saída (rede ou processo travado); foi interrompida",
    be_cli_crashed: "A CLI terminou de forma inesperada ({status}): {tail}",
    be_cli_no_reply: "A CLI não produziu resposta ({status}): {tail}",
    be_cli_turn_failed: "O turno da CLI do {cli} falhou",
    be_cli_turn_failed_status: "O turno da CLI do {cli} falhou ({status})",
    be_cli_unexpected_abort: "Erro interno: interrupção informada sem sinal de interromper",
    be_agy_conversation_mismatch:
      "O Agy retomou outra conversa: esperado {expected}, veio {actual}",
    be_agy_lock_poisoned: "A trava de gravação do Agy está corrompida",
    be_lane_state_write_failed: "Não foi possível gravar o arquivo de estado da conversa {path}: {error}",
    be_session_abandon_failed:
      "Depois de interromper, não foi possível apagar o arquivo de sessão {path}: {error}. A conversa já foi limpa; envie de novo.",
    be_lane_rewrite_unsupported:
      "As conversas contínuas do {provider} não conseguem apagar configurações privadas após o turno; elas precisam ficar no prompt de sistema fixo desse personagem.",
  },
  de: {
    be_cli_workspace_failed: "Der CLI-Arbeitsordner konnte nicht vorbereitet werden: {error}",
    be_grok_profile_failed: "Der grok-Einstellungsordner konnte nicht vorbereitet werden: {error}",
    be_cli_risk_not_accepted:
      "Der Risikohinweis für den CLI-Abonnementmodus ist noch nicht bestätigt. Bestätige ihn in den Einstellungen.",
    be_cli_not_found: "Die {cli}-CLI wurde nicht gefunden. Prüfe, ob sie installiert und angemeldet ist.",
    be_agy_too_old:
      "Gemini CLI {version} ist zu alt; diese App braucht 1.1.8 oder neuer. Führe `agy update` aus und verifiziere dann neu.",
    be_unknown_transport: "Unbekannte Verbindungsart: {transport}",
    be_tier_model_missing: "Für die Stufe „{tier}“ ist noch kein Modell eingestellt. Trage es in den Einstellungen ein.",
    be_openrouter_api_key_missing: "Noch kein OpenRouter-API-Schlüssel. Füge ihn in den Einstellungen ein.",
    be_no_free_models: "Derzeit ist kein kostenloses Modell verfügbar.",
    be_no_stable_free_model:
      "Derzeit ist kein stabiles Gratis-Modell verfügbar. Wähle in den Einstellungen ein anderes Gratis-Modell oder ein Modell von Hand.",
    be_smart_free_daily_exhausted:
      "Die heutigen Anfragen für kostenlose OpenRouter-Modelle sind aufgebraucht. Versuche es nach dem Zurücksetzen des Kontingents erneut.",
    be_responses_api_failed: "Die Responses API hat einen Fehler gemeldet",
    be_cli_reply_error: "CLI-Fehler: {error}",
    be_cli_stdin_timeout: "Die CLI hat den Prompt nicht innerhalb von 60 Sekunden angenommen; abgebrochen",
    be_cli_stalled:
      "Die CLI hat 120 Sekunden lang nichts ausgegeben (Netzwerk oder Prozess hängt); abgebrochen",
    be_cli_crashed: "Die CLI wurde unerwartet beendet ({status}): {tail}",
    be_cli_no_reply: "Die CLI hat keine Antwort geliefert ({status}): {tail}",
    be_cli_turn_failed: "Runde der {cli}-CLI fehlgeschlagen",
    be_cli_turn_failed_status: "Runde der {cli}-CLI fehlgeschlagen ({status})",
    be_cli_unexpected_abort: "Interner Fehler: Abbruch gemeldet, obwohl kein Abbruchsignal kam",
    be_agy_conversation_mismatch:
      "Agy hat eine andere Unterhaltung fortgesetzt: erwartet {expected}, erhalten {actual}",
    be_agy_lock_poisoned: "Die Rückschreibsperre von Agy ist beschädigt",
    be_lane_state_write_failed:
      "Die Statusdatei des Gesprächsstrangs {path} konnte nicht geschrieben werden: {error}",
    be_session_abandon_failed:
      "Nach dem Abbruch ließ sich die Session-Datei {path} nicht löschen: {error}. Der Strang wurde bereits geleert; bitte sende erneut.",
    be_lane_rewrite_unsupported:
      "Gesprächsstränge von {provider} können private Einstellungen nach der Runde nicht löschen; private Einstellungen gehören in den festen System-Prompt dieser Figur.",
  },
  fr: {
    be_cli_workspace_failed: "Impossible de préparer le dossier de travail de la CLI\u00a0: {error}",
    be_grok_profile_failed: "Impossible de préparer le dossier de réglages de grok\u00a0: {error}",
    be_cli_risk_not_accepted:
      "L'avertissement sur les risques du mode abonnement CLI n'est pas encore confirmé. Confirme-le dans les réglages.",
    be_cli_not_found: "CLI {cli} introuvable. Vérifie qu'elle est installée et connectée.",
    be_agy_too_old:
      "Gemini CLI {version} est trop ancienne\u00a0; cette app nécessite la 1.1.8 ou plus récente. Lance `agy update`, puis revalide.",
    be_unknown_transport: "Type de connexion inconnu\u00a0: {transport}",
    be_tier_model_missing: "Aucun modèle n'est défini pour la catégorie «\u00a0{tier}\u00a0». Renseigne-le dans les réglages.",
    be_openrouter_api_key_missing: "Pas encore de clé API OpenRouter. Colle-la dans les réglages.",
    be_no_free_models: "Aucun modèle gratuit n'est disponible pour le moment.",
    be_no_stable_free_model:
      "Aucun modèle gratuit stable n'est disponible pour le moment. Choisis un autre modèle gratuit dans les réglages ou sélectionne un modèle à la main.",
    be_smart_free_daily_exhausted:
      "Les requêtes du jour vers les modèles gratuits d'OpenRouter sont épuisées. Réessaie après la remise à zéro du quota.",
    be_responses_api_failed: "La Responses API a renvoyé un échec",
    be_cli_reply_error: "Erreur de la CLI\u00a0: {error}",
    be_cli_stdin_timeout: "La CLI n'a pas accepté le prompt en 60 secondes\u00a0; arrêtée",
    be_cli_stalled: "La CLI n'a rien produit pendant 120 secondes (réseau ou processus bloqué)\u00a0; arrêtée",
    be_cli_crashed: "La CLI s'est arrêtée de façon inattendue ({status})\u00a0: {tail}",
    be_cli_no_reply: "La CLI n'a produit aucune réponse ({status})\u00a0: {tail}",
    be_cli_turn_failed: "Échec du tour de la CLI {cli}",
    be_cli_turn_failed_status: "Échec du tour de la CLI {cli} ({status})",
    be_cli_unexpected_abort: "Erreur interne\u00a0: arrêt signalé sans signal d'arrêt",
    be_agy_conversation_mismatch:
      "Agy a repris une autre conversation\u00a0: attendu {expected}, reçu {actual}",
    be_agy_lock_poisoned: "Le verrou d'écriture d'Agy est corrompu",
    be_lane_state_write_failed: "Impossible d'écrire le fichier d'état du fil {path}\u00a0: {error}",
    be_session_abandon_failed:
      "Après l'arrêt, impossible de supprimer le fichier de session {path}\u00a0: {error}. Le fil a été vidé\u00a0; renvoie ton message.",
    be_lane_rewrite_unsupported:
      "Les fils de {provider} ne peuvent pas effacer les réglages privés après le tour\u00a0; ils doivent aller dans le prompt système fixe de ce personnage.",
  },
  ru: {
    be_cli_workspace_failed: "Не удалось подготовить рабочую папку CLI: {error}",
    be_grok_profile_failed: "Не удалось подготовить папку настроек grok: {error}",
    be_cli_risk_not_accepted:
      "Согласие с рисками режима подписки CLI ещё не подтверждено. Подтверди его в настройках.",
    be_cli_not_found: "CLI {cli} не найден. Проверь, что он установлен и в нём выполнен вход.",
    be_agy_too_old:
      "Gemini CLI {version} слишком старый: приложению нужна версия 1.1.8 или новее. Выполни `agy update` и проверь заново.",
    be_unknown_transport: "Неизвестный способ подключения: {transport}",
    be_tier_model_missing: "Для уровня «{tier}» ещё не задана модель. Укажи её в настройках.",
    be_openrouter_api_key_missing: "API key OpenRouter ещё не задан. Вставь его в настройках.",
    be_no_free_models: "Сейчас нет доступных бесплатных моделей.",
    be_no_stable_free_model:
      "Сейчас нет доступной стабильной бесплатной модели. Выбери в настройках другую бесплатную модель или укажи модель вручную.",
    be_smart_free_daily_exhausted:
      "Сегодняшние запросы к бесплатным моделям OpenRouter закончились. Попробуй снова после сброса лимита.",
    be_responses_api_failed: "Responses API вернул ошибку",
    be_cli_reply_error: "Ошибка CLI: {error}",
    be_cli_stdin_timeout: "CLI не принял промпт за 60 секунд, выполнение остановлено",
    be_cli_stalled: "CLI ничего не выводил 120 секунд (зависла сеть или процесс), выполнение остановлено",
    be_cli_crashed: "CLI неожиданно завершился ({status}): {tail}",
    be_cli_no_reply: "CLI не выдал ответа ({status}): {tail}",
    be_cli_turn_failed: "Ход {cli} CLI завершился ошибкой",
    be_cli_turn_failed_status: "Ход {cli} CLI завершился ошибкой ({status})",
    be_cli_unexpected_abort: "Внутренняя ошибка: сообщено об остановке без сигнала остановки",
    be_agy_conversation_mismatch:
      "Agy продолжил другой разговор: ожидался {expected}, получен {actual}",
    be_agy_lock_poisoned: "Блокировка записи Agy повреждена",
    be_lane_state_write_failed: "Не удалось записать файл состояния ветки диалога {path}: {error}",
    be_session_abandon_failed:
      "После остановки не удалось удалить файл сессии {path}: {error}. Ветка уже очищена — отправь ещё раз.",
    be_lane_rewrite_unsupported:
      "Ветки диалога {provider} не умеют стирать приватные настройки после хода; приватные настройки нужно поместить в постоянный системный промпт этого персонажа.",
  },
} as const;

type AiMsgLang = keyof typeof COPY;

export type AiMsgKey = keyof (typeof COPY)["zh-TW"];
export const AI_MSG_MESSAGE_KEYS = Object.keys(COPY["zh-TW"]) as AiMsgKey[];

export function isAiMsgKey(key: string): key is AiMsgKey {
  return key in COPY["zh-TW"];
}

export function aiMsgMessage(lang: AiMsgLang, key: AiMsgKey): string {
  return COPY[lang][key];
}
