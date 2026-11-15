import { useCallback, useEffect, useState } from "react";
import { api } from "../../ipc";
import type { StoryDirector } from "../../types";

interface Props {
  storyId: string;
  onClose: () => void;
  onToast: (msg: string, kind?: "ok" | "error") => void;
  onSaved: () => void;
}

export default function DirectorPanel({ storyId, onClose, onToast, onSaved }: Props) {
  const [d, setD] = useState<StoryDirector | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try { setD(await api.getDirector(storyId)); }
    catch (e) { onToast(String(e), "error"); }
  }, [storyId, onToast]);

  useEffect(() => { void load(); }, [load]);

  const patch = (p: Partial<StoryDirector>) => setD((d) => (d ? { ...d, ...p } : d));

  const save = async () => {
    if (!d) return;
    setBusy(true);
    try {
      setD(await api.saveDirector({
        story_id: storyId,
        enabled: d.enabled,
        outline_md: d.outline_md,
        next_note: d.next_note,
      }));
      onToast(d.enabled ? "导演模式已保存（下一回合起生效）" : "导演模式已关闭");
      onSaved();
    } catch (e) { onToast(String(e), "error"); }
    finally { setBusy(false); }
  };

  return (
    <aside className="archive-drawer director-panel">
      <div className="archive-head">
        <strong>导演</strong>
        <label className="switch-row">
          <input type="checkbox" checked={d?.enabled ?? false}
            onChange={(e) => patch({ enabled: e.target.checked })} />
          <span>启用</span>
        </label>
        <div className="grow" />
        <button className="btn btn-sm btn-ghost" onClick={onClose}>收起</button>
      </div>

      <div className="archive-body">
        <div className="hint">
          导演模式不改变"读者代入主角"这件事，只是给作者留一个后门：大纲管长线走向，指令管本回合必须发生什么。
        </div>

        <div className="field-block">
          <div className="field-title">① 剧情大纲（长期生效）</div>
          <div className="hint">写你想推进的主线、必须铺垫的伏笔、某个角色该登场了。它每回合都会注入，模型参考但不照抄。</div>
          <textarea className="input director-text" rows={10}
            value={d?.outline_md ?? ""}
            placeholder={"示例：\n- 第 3~5 回合内让唐三发现小舞的兔耳\n- 主线：史莱克七怪成型，不要提前写魂师大赛\n- 戴沐白对主角保持敌意，直到第 8 回合"}
            onChange={(e) => patch({ outline_md: e.target.value })} />
        </div>

        <div className="field-block">
          <div className="field-title">② 本回合指令（只生效一次）</div>
          <div className="hint">
            这次生成必须照做，写进去之后自动清空。适合"这回合我要跟小舞对上一拳"这类硬要求。
          </div>
          <textarea className="input director-text" rows={4}
            value={d?.next_note ?? ""}
            placeholder="例如：不管选项怎么选，本回合结尾必须让赵无极出现并叫停比试"
            onChange={(e) => patch({ next_note: e.target.value })} />
          {d?.next_note ? (
            <div className="hint warn-text">这条指令还没被用掉；生成一回合后会自动清空。</div>
          ) : null}
        </div>

        <div className="row" style={{ marginTop: 12 }}>
          <button className="btn btn-primary" disabled={busy || !d} onClick={() => void save()}>
            {busy ? "保存中…" : "保存导演设置"}
          </button>
          <button className="btn btn-ghost" onClick={() => void load()}>重新读取</button>
        </div>
      </div>
    </aside>
  );
}
