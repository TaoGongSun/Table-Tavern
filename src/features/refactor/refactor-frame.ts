// 外框條目（refactor-statusbar-skeleton 待問 3〔作者裁決 2026-10-03〕）：只規定輸出容器排法、容器裡只有佔位
// 說明的介面條目，例如「正文放 <maintext>、狀態欄放 <Status_block>」。後端盤點時用程式判出候選
// （frame.rs）；這裡在定義條目展開完後決定誰是真外框，並照外框的容器順序組殼。純函式。
import { BODY_PLACEHOLDER } from "./refactor-shell";

/** 外框候選：條目 uid 與容器標籤（出現順序、不重複），後端 RefactorFrameCandidate */
export interface RefactorFrameCandidate {
  uid: string;
  tags: string[];
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function containerRegex(tag: string): RegExp {
  const name = escapeRegExp(tag);
  return new RegExp(`<${name}>[\\s\\S]*?</${name}>`);
}

/**
 * 候選要當外框，得有同一次重構裡另一條定義條目的骨架含相同容器標籤——找不到就照一般條目展開，
 * 寧可多花一次展開，也不能把唯一的定義條目誤判成外框而丟掉欄位。
 */
export function confirmFrames(
  candidates: RefactorFrameCandidate[],
  definingShells: string[],
): { frames: RefactorFrameCandidate[]; expand: RefactorFrameCandidate[] } {
  const frames: RefactorFrameCandidate[] = [];
  const expand: RefactorFrameCandidate[] = [];
  for (const candidate of candidates) {
    const matched = candidate.tags.some((tag) =>
      definingShells.some((shell) => containerRegex(tag).test(shell)),
    );
    (matched ? frames : expand).push(candidate);
  }
  return { frames, expand };
}

// 一個外框套到骨架上：外框的每個容器依外框順序排；骨架裡有的容器原樣搬過來，唯一一個骨架沒有的容器放
// 正文槽；骨架其餘內容接在後面。組不起來回 null：一個容器都沒對上、沒對上的超過一個、或骨架已經有正文槽
// 又多出一個沒對上的容器。
function composeOne(skeleton: string, tags: string[]): string | null {
  let rest = skeleton;
  const blocks: (string | null)[] = tags.map((tag) => {
    const match = containerRegex(tag).exec(rest);
    if (match === null) return null;
    rest = rest.slice(0, match.index) + rest.slice(match.index + match[0].length);
    return match[0];
  });
  const missing = tags.filter((_, index) => blocks[index] === null);
  if (missing.length === tags.length || missing.length > 1) return null;
  if (missing.length === 1 && skeleton.includes(`{{${BODY_PLACEHOLDER}}}`)) return null;
  const ordered = blocks.map(
    (block, index) => block ?? `<${tags[index]}>\n{{${BODY_PLACEHOLDER}}}\n</${tags[index]}>`,
  );
  const remainder = rest.replace(/\n{3,}/g, "\n\n").trim();
  return [...ordered, ...(remainder === "" ? [] : [remainder])].join("\n");
}

/** 依 uid 順序把外框逐一套到合併後的骨架；組不起來的外框列在 failed（呼叫端保留來源、記成失敗）。 */
export function composeFrames(
  skeleton: string,
  frames: RefactorFrameCandidate[],
): { shell: string; applied: string[]; failed: string[] } {
  const ordered = [...frames].sort((a, b) => Number(a.uid) - Number(b.uid));
  let shell = skeleton;
  const applied: string[] = [];
  const failed: string[] = [];
  for (const frame of ordered) {
    const composed = composeOne(shell, frame.tags);
    if (composed === null) {
      failed.push(frame.uid);
    } else {
      shell = composed;
      applied.push(frame.uid);
    }
  }
  return { shell, applied, failed };
}
