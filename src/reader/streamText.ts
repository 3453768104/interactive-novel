/**
 * Turn a partially streamed model reply into the prose the reader should see.
 *
 * The model streams the entire four-block contract, so the raw text contains
 * `<<<NARRATION>>>` markers plus the ANALYSIS / STOP / CHOICES payloads.
 * Rendering that raw stream is what made the draft look broken; here we keep
 * only the narration that has arrived so far and tolerate every half-received
 * marker (a chunk boundary can land in the middle of `<<<END_NARRA`).
 */

const OPEN_MARKER = /<<<\s*NARRATION\s*>>>/i;
const OPEN_ALIAS = /【正文】/;
/** Anything that means "narration is over": a marker, a stray `<`, or an alias. */
const CLOSE_HINT = /<<<|【\/?(?:分析|选项|状态|摘要|停顿|正文)】/;

/**
 * 小模型有时候一个标记都不写，直接把分析对象接在正文后面。
 * 先按"已知字段名"定位（流式时字段名可能只收到一半），兜底再按 `{"` 定位。
 */
const JSON_TAIL_KEYED = /\{\s*"(?:world|player|charac|summary|narrat|analys|stop|choic|prompt)/i;
const JSON_TAIL_ANY = /\n?\s*\{\s*"/;

const COURTESY =
  /^(?:好的[，,。！!]?|以下是.*[:：]?|Sure[!,.]?|Here'?s .*[:：]?|Here is .*[:：]?)$/i;

export function streamingNarration(raw: string): string {
  if (!raw) return "";
  let text = raw.replace(/\r\n?/g, "\n");

  // api mode may answer with a single JSON object instead of markers.
  const head = text.trimStart();
  if (head.startsWith("{")) {
    const fromJson = jsonStringField(head, "narration");
    // Still waiting for the field: show nothing rather than raw JSON.
    return fromJson == null ? "" : tidy(fromJson);
  }

  const open = OPEN_MARKER.exec(text) ?? OPEN_ALIAS.exec(text);
  if (open && open.index != null) {
    text = text.slice(open.index + open[0].length);
  } else {
    // No narration marker yet: whatever precedes the first block marker is prose.
    const cut = text.search(CLOSE_HINT);
    if (cut > 0) text = text.slice(0, cut);
  }

  const end = text.search(CLOSE_HINT);
  if (end >= 0) text = text.slice(0, end);

  const keyed = text.search(JSON_TAIL_KEYED);
  if (keyed > 0) {
    text = text.slice(0, keyed);
  } else {
    const any = text.search(JSON_TAIL_ANY);
    // 40 字以上的正文后面才可能出现对象；太短的先不动，避免误伤引用里的花括号。
    if (any > 40) text = text.slice(0, any);
  }
  return tidy(text);
}

/** Drop code fences, leading blank lines and one-line courtesy preambles. */
function tidy(text: string): string {
  const kept = text.split("\n").filter((line) => !/^\s*```/.test(line));
  while (kept.length > 0) {
    const first = kept[0].trim();
    if (first === "" || COURTESY.test(first) || /^```/.test(first)) kept.shift();
    else break;
  }
  return kept.join("\n").replace(/^\s+/, "");
}

/**
 * Read a string field out of an unterminated JSON object. Returns null when the
 * field has not arrived yet, and a partial value while it is still streaming.
 */
function jsonStringField(text: string, field: string): string | null {
  const m = new RegExp(`"${field}"\\s*:\\s*"`).exec(text);
  if (!m) return null;
  let out = "";
  let i = m.index + m[0].length;
  while (i < text.length) {
    const ch = text[i];
    if (ch === "\\") {
      const next = text[i + 1];
      if (next === undefined) break; // escape split across chunks
      if (next === "n") out += "\n";
      else if (next === "t") out += "\t";
      else if (next === "u") {
        const hex = text.slice(i + 2, i + 6);
        if (hex.length < 4) break;
        const code = parseInt(hex, 16);
        if (!Number.isNaN(code)) out += String.fromCharCode(code);
        i += 6;
        continue;
      } else if (next !== "r") out += next;
      i += 2;
      continue;
    }
    if (ch === '"') break;
    out += ch;
    i += 1;
  }
  return out;
}
