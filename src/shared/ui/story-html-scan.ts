// 故事 HTML 進消毒器前的整理（marked 產出之後）。只會讓東西多顯示成字、不會生出新標記；消毒器仍是最後一道。

// 中文標籤的結尾（</核心记忆>）在解析器眼中是「假註解」，會被整個吃掉，配對就找不到結尾。
// 先改成字面文字：在屬性值與 textarea 裡 &lt; 解碼回來一模一樣，在文字裡就成了可配對的字
const CJK_CLOSING = /<\/((?![\x00-\x7F])\p{L}[^\s<>/&"'=]{0,40}\s*)>/gu;

// 沒收尾就把之後整則回覆收進去、而消毒器剝它們時連內容一起丟（textarea 則把之後的標籤全變成字）的元素。
// 有沒有收尾不自己模仿解析規格判斷，直接問瀏覽器：在尾端接一段哨兵字，解析後看它落在哪
const SWALLOWING = new Set([
  "script",
  "style",
  "title",
  "textarea",
  "xmp",
  "noembed",
  "noframes",
  "iframe",
  "plaintext",
  "template",
  // 內容會留下，但沒收尾時之後整段被當成 svg／math 解析、排版全失，所以也拆開
  "svg",
  "math",
]);
// 內容是純文字的（解析器不往裡面建元素）：修復時把文字當 HTML 重新解析
const RAW_TEXT = new Set(["script", "style", "title", "textarea", "xmp", "noembed", "noframes", "iframe", "plaintext"]);

// 每次呼叫重新產生：輸入猜不到，也就冒充不了（實體編碼過的固定字串解析後會變成同一串字）
function newSentinel(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return `tt-end-${Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
const SHOW_TEXT_AND_COMMENTS = 0x4 | 0x80;
// 罕見狀況輕防：一則訊息裡要修的未收尾結構超過這個數就不再細修
const MAX_REPAIRS = 30;

function parseBody(html: string): HTMLElement {
  return new DOMParser().parseFromString(`<!DOCTYPE html><body>${html}`, "text/html").body;
}

/** 哨兵所在的文字或註解節點；在 template 內容裡時一併回傳那個 template */
function findSentinel(root: Node, sentinel: string): { node: Node; template: Element | null } | null {
  const document = root.ownerDocument ?? (root as Document);
  const walker = document.createTreeWalker(root, SHOW_TEXT_AND_COMMENTS);
  while (walker.nextNode()) {
    if ((walker.currentNode as CharacterData).data.includes(sentinel)) return { node: walker.currentNode, template: null };
  }
  const scope = root as ParentNode;
  for (const template of [...scope.querySelectorAll("template")]) {
    const found = findSentinel((template as HTMLTemplateElement).content, sentinel);
    if (found) return { node: found.node, template: found.template ?? template };
  }
  return null;
}

/** 吞掉哨兵的最外層結構（未收尾的註解或吞文元素）；哨兵在正常位置時回 null */
function swallower(found: { node: Node; template: Element | null }): Node | null {
  let culprit: Node | null = found.node.nodeType === 8 ? found.node : null;
  for (let node: Node | null = found.template ?? found.node.parentNode; node; node = node.parentNode) {
    if (node.nodeType === 1 && SWALLOWING.has((node as Element).localName)) culprit = node;
  }
  return culprit;
}

/** 把吞文結構拆開：開頭改成字，吞進去的內容放回原處（純文字內容重新當 HTML 解析） */
function release(culprit: Node): void {
  const document = culprit.ownerDocument!;
  if (culprit.nodeType === 8) {
    (culprit as ChildNode).replaceWith(document.createTextNode("<!--"), ...parseBody((culprit as Comment).data).childNodes);
    return;
  }
  const element = culprit as Element;
  const inner =
    element.localName === "template"
      ? [...(element as HTMLTemplateElement).content.childNodes]
      : RAW_TEXT.has(element.localName)
        ? [...parseBody(element.textContent ?? "").childNodes]
        : [...element.childNodes];
  element.replaceWith(document.createTextNode(`<${element.localName}>`), ...inner);
}

function escapeUnclosedSwallowing(html: string): string {
  const sentinel = newSentinel();
  let current = html + sentinel;
  let repaired = false;
  for (let round = 0; round < MAX_REPAIRS; round++) {
    const body = parseBody(current);
    const found = findSentinel(body, sentinel);
    if (!found) {
      // 哨兵整個不見：尾端有寫到一半的標籤（引號沒關、結尾標記沒寫完）。從最後一個 < 往前試著改成字，
      // 試到哨兵重新出現為止
      const tried = tryEscapeTail(current, sentinel);
      if (tried === null) break;
      current = tried;
      repaired = true;
      continue;
    }
    const culprit = swallower(found);
    if (!culprit) return repaired ? current.replace(sentinel, "") : html;
    release(culprit);
    current = body.innerHTML;
    repaired = true;
  }
  // 修不完：從原文第一個 < 起全部改成字，寧可露標記也不丟正文
  const first = html.indexOf("<");
  return first < 0 ? html : html.slice(0, first) + html.slice(first).replace(/</g, "&lt;");
}

function tryEscapeTail(html: string, sentinel: string): string | null {
  for (let index = html.lastIndexOf("<"), tries = 0; index >= 0 && tries < MAX_REPAIRS; tries++) {
    const candidate = `${html.slice(0, index)}&lt;${html.slice(index + 1)}`;
    if (findSentinel(parseBody(candidate), sentinel)) return candidate;
    index = html.lastIndexOf("<", index - 1);
  }
  return null;
}

export function prepareRawHtml(html: string): string {
  return escapeUnclosedSwallowing(html.replace(CJK_CLOSING, "&lt;/$1&gt;"));
}
