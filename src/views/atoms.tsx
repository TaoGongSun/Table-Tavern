import { t } from "../i18n";
import { explainAiError } from "../ai-error";

// 錯誤列：命中分流就顯示人話，原始字串一律保留在小字（玩家與協助者仍看得到真相）
export function ErrorNote({ text }: { text: string }) {
  const key = explainAiError(text);
  if (!key) return <p role="alert">{text}</p>;
  return (
    <p role="alert">
      {t(key)}
      <br />
      <small>{text}</small>
    </p>
  );
}
