import { escapeRegExp } from "./prefs";

/** Replace one top-level scalar while keeping comments and formatting intact. */
export function patchYamlScalar(
  text: string,
  key: string,
  value: string | number | boolean,
): string {
  const re = new RegExp(`^(\\s*${escapeRegExp(key)}\\s*:\\s*)([^#\\n]*?)(\\s*(#.*)?)$`, "m");
  if (!re.test(text)) {
    return `${text.trimEnd()}\n${key}: ${value}\n`;
  }
  return text.replace(re, (_m, p1: string, _old: string, tail: string) =>
    `${p1}${value}${tail}`,
  );
}

/** Replace fields inside one inline map line, e.g. `state_card: { on: true, budget: 700, trim: 3 }`. */
export function patchInlineMap(
  text: string,
  key: string,
  patch: Record<string, string | number | boolean>,
): string {
  const lineRe = new RegExp(`^(\\s*${escapeRegExp(key)}\\s*:\\s*)\\{([^}]*)\\}(\\s*(#.*)?)$`, "m");
  const lines = text.split("\n");
  let changed = false;
  const out = lines.map((line) => {
    const m = line.match(lineRe);
    if (!m) return line;
    changed = true;
    let body = m[2].trim();
    for (const [k, v] of Object.entries(patch)) {
      const fieldRe = new RegExp(`(${escapeRegExp(k)}\\s*:\\s*)([^,}]+)`);
      if (fieldRe.test(body)) {
        body = body.replace(fieldRe, (_x, p1: string) => `${p1}${v}`);
      } else {
        body = body ? `${body}, ${k}: ${v}` : `${k}: ${v}`;
      }
    }
    return `${m[1]}{ ${body} }${m[3]}`;
  });
  return changed ? out.join("\n") : text;
}

export function triggersToYaml(triggers: string[]): string {
  return `[${triggers.join(", ")}]`;
}

/**
 * Replace one scalar inside a nested block structure, e.g.
 * `engines → local → params → temperature`, keeping the file's comments and
 * indentation intact. When the key does not exist yet it is appended to the
 * end of its parent block (so a new option can be added from the GUI).
 */
export function patchNestedScalar(
  text: string,
  path: string[],
  value: string | number | boolean | null,
): string {
  const lines = text.split("\n");
  const stack: { indent: number; key: string }[] = [];
  let parentLine = -1;
  let parentIndent = -1;

  for (let i = 0; i < lines.length; i += 1) {
    const m = lines[i].match(/^(\s*)([A-Za-z0-9_-]+)\s*:\s*(.*)$/);
    if (!m) continue;
    const indent = m[1].length;
    const key = m[2];
    while (stack.length > 0 && stack[stack.length - 1].indent >= indent) stack.pop();
    stack.push({ indent, key });
    const here = stack.map((s) => s.key);

    if (here.length === path.length && here.every((k, idx) => k === path[idx])) {
      const rest = m[3];
      const commentIdx = rest.indexOf(" #");
      const comment = commentIdx >= 0 ? rest.slice(commentIdx) : "";
      lines[i] = `${m[1]}${key}: ${yamlValue(value)}${comment}`;
      return lines.join("\n");
    }
    if (
      here.length === path.length - 1 &&
      here.every((k, idx) => k === path[idx])
    ) {
      parentLine = i;
      parentIndent = indent;
    }
  }

  // 键不存在：插到父块末尾（父块 = 缩进比父键更深的连续行）
  if (parentLine >= 0) {
    let insertAt = parentLine + 1;
    for (let j = parentLine + 1; j < lines.length; j += 1) {
      const line = lines[j];
      if (line.trim() === "") continue;
      const ind = line.match(/^(\s*)/)?.[1].length ?? 0;
      if (ind <= parentIndent) break;
      insertAt = j + 1;
    }
    const childIndent = " ".repeat(parentIndent + 2);
    lines.splice(insertAt, 0, `${childIndent}${path[path.length - 1]}: ${yamlValue(value)}`);
    return lines.join("\n");
  }
  return text;
}

function yamlValue(value: string | number | boolean | null): string {
  if (typeof value === "number") return Number.isFinite(value) ? String(value) : "0";
  if (typeof value === "boolean") return value ? "true" : "false";
  if (value == null) return '""';
  const s = value;
  if (s === "") return '""';
  // URLs and model names are safe as plain scalars; quote anything that could be
  // read as a comment, a number, or a YAML keyword.
  const plain =
    /^[A-Za-z0-9_.\/:@+-]+$/.test(s) &&
    !/^(true|false|null|yes|no|on|off|~)$/i.test(s) &&
    !/^[-+]?[0-9.]+$/.test(s);
  return plain ? s : JSON.stringify(s);
}
