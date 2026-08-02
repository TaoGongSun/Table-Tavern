import DOMPurify from "dompurify";
import { Marked, type Tokens } from "marked";

export const STORY_MARKDOWN_ALLOWED_TAGS = [
  "em",
  "strong",
  "code",
  "pre",
  "blockquote",
  "ul",
  "ol",
  "li",
  "p",
  "br",
  "hr",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
];

export const STORY_MARKDOWN_ALLOWED_ATTR: string[] = [];

function escapeHtml(text: string): string {
  return text.replace(/[&<>]/g, (character) => {
    switch (character) {
      case "&":
        return "&amp;";
      case "<":
        return "&lt;";
      case ">":
        return "&gt;";
      default:
        return character;
    }
  });
}

const marked = new Marked({
  breaks: true,
  gfm: true,
  renderer: {
    html(token: Tokens.HTML | Tokens.Tag): string {
      return escapeHtml(token.text);
    },
    link(token): string {
      return token.text;
    },
  },
});

export function renderStoryMarkdown(text: string): string {
  const html = marked.parse(text) as string;
  return DOMPurify.sanitize(html, {
    ALLOWED_TAGS: STORY_MARKDOWN_ALLOWED_TAGS,
    ALLOWED_ATTR: STORY_MARKDOWN_ALLOWED_ATTR,
  });
}
