import { useEffect, useState } from "react";
import { api } from "../../ipc";
import type { Nav } from "../../types";

interface Props {
  worldId: string;
  nav: Nav;
}

/** Rough client-side preview; the authoritative estimate is in the backend
 *  budget report (Settings -> 上下文预览). */
function roughTokens(text: string): number {
  let cjk = 0;
  let other = 0;
  for (const c of text) {
    if (c >= "\u4e00" && c <= "\u9fff") cjk += 1;
    else other += 1;
  }
  return Math.ceil((cjk + other * 0.28) * 1.15);
}

export default function EventBackgroundEditor({ worldId, nav }: Props) {
  const [content, setContent] = useState("");
  const [base, setBase] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api.getWorldEventBackground(worldId)
      .then((bg) => { setContent(bg.content_md); setBase(bg.content_md); })
      .catch((e) => nav.toast(String(e), "error"));
  }, [worldId]);

  const save = async () => {
    setSaving(true);
    try {
      await api.saveWorldEventBackground(worldId, content);
      setBase(content);
      nav.toast("事件背景已保存（下回合生效）");
    } catch (e) { nav.toast(String(e), "error"); }
    finally { setSaving(false); }
  };

  const est = roughTokens(content);

  return (
    <div className="card">
      <div className="card-title">
        世界观事件背景 <span className="muted">· 该世界观下所有剧本默认继承</span>
      </div>
      <div className="hint" style={{ marginBottom: 10 }}>
        这里写时代、起因、当前局势、核心矛盾。它<b>默认全部注入上下文</b>、不走检索、不参与自动裁剪；
        某本剧本可以在「剧本」页单独覆盖。写太长会挤占本地 10K 窗口，建议控制在 2000 tokens 内。
      </div>
      <textarea className="textarea" style={{ minHeight: 260 }}
        value={content} onChange={(e) => setContent(e.target.value)}
        placeholder="例：第二纪末年，北境封印松动，各城邦开始征召能感应魔力的人……" />
      <div className="row" style={{ marginTop: 10 }}>
        <span className={`badge ${est > 2000 ? "badge-dirty" : "badge-clean"}`}>
          估算 {est} tokens {est > 2000 ? "（偏长）" : ""}
        </span>
        <span className="hint">{content !== base ? "有未保存修改" : "已保存"}</span>
        <div className="grow" />
        <button className="btn btn-primary" disabled={saving} onClick={() => void save()}>
          {saving ? <span className="spinner" /> : "保存"}
        </button>
      </div>
    </div>
  );
}
