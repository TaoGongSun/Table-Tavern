// 擲果一行（桌面與房間共用）：總和加粗。切最後一個「 = 」，卡名裡的等號不會切錯
export function DiceRollText({ text }: { text: string }) {
  const at = text.lastIndexOf(" = ");
  if (at < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, at + 1)}
      <strong>{text.slice(at + 1)}</strong>
    </>
  );
}
