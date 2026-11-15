import type { ReactNode } from "react";
import { escapeRegExp } from "../../reader/prefs";
import { toParagraphs } from "../../reader/paragraphs";
import type { Pov } from "../../types";

interface Props {
  text: string;
  pov: Pov;
  showSceneMarkers: boolean;
  /** Emergent characters the reader may want to promote to notable. */
  candidates: string[];
  onMarkCharacter: (name: string) => void;
  /** 作者模式：主角视角下也把「〔暗场〕」段落显示出来（含剧透）。 */
  showHidden?: boolean;
  /** 点「看看暗场」时切到作者视图。 */
  onRevealHidden?: () => void;
  draft?: boolean;
}

/** Insert a tiny "＋" badge right after a nominated character's name. */
function annotate(paragraph: string, names: string[], onPick: (name: string) => void): ReactNode[] {
  const clean = names
    .map((n) => n.trim())
    .filter((n) => n.length >= 1 && n.length <= 24)
    .slice(0, 10);
  if (clean.length === 0) return [paragraph];
  // Longest first so "沈砚" doesn't shadow "沈砚之".
  const ordered = [...new Set(clean)].sort((a, b) => b.length - a.length);
  const re = new RegExp(`(${ordered.map(escapeRegExp).join("|")})`, "g");
  return paragraph.split(re).map((part, i) => {
    if (ordered.includes(part)) {
      return (
        <span key={i} className="char-inline">
          {part}
          <button className="char-plus" title={`把「${part}」标为重要角色`}
            onClick={(e) => { e.stopPropagation(); onPick(part); }}>＋</button>
        </span>
      );
    }
    return <span key={i}>{part}</span>;
  });
}

export default function Narration({
  text, pov, showSceneMarkers, candidates, onMarkCharacter,
  showHidden = false, onRevealHidden, draft = false,
}: Props) {
  const paragraphs = toParagraphs(text);
  const revealHidden = pov === "omniscient" || showHidden;
  const hiddenCount = paragraphs.filter((p) => p.hidden).length;
  const shown = revealHidden ? paragraphs : paragraphs.filter((p) => !p.hidden);

  return (
    <div className={draft ? "novel-draft" : undefined}>
      {paragraphs.length === 0 && draft && (
        <div className="novel-p muted">（正在等待模型写下第一段…）</div>
      )}
      {shown.map((p, idx) => {
        const isShift = pov === "omniscient" && showSceneMarkers && /^[◇◆]/.test(p.text);
        if (isShift) {
          return (
            <div className="scene-shift" key={idx}>
              <span className="scene-line" />
              <span className="scene-label">{p.text.replace(/^[◇◆]\s*/, "")}</span>
              <span className="scene-line" />
            </div>
          );
        }
        return (
          <p className={`novel-p ${p.hidden ? "novel-p-hidden" : ""}`} key={idx}>
            {annotate(p.text, candidates, onMarkCharacter)}
          </p>
        );
      })}
      {shown.length === 0 && hiddenCount > 0 && !revealHidden && !draft && (
        <div className="novel-p muted">
          本回合正文都在暗场里。
          <button className="hidden-hint-btn" onClick={onRevealHidden}>展开</button>
        </div>
      )}
    </div>
  );
}
