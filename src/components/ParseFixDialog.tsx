import { useState } from "react";
import { CHOICES_SUSPECT, type ParsedOutput } from "../types";

interface Props {
  parsed: ParsedOutput;
  onCancel: () => void;
  onConfirm: (p: ParsedOutput) => void;
}

export default function ParseFixDialog({ parsed, onCancel, onConfirm }: Props) {
  const [narration, setNarration] = useState(parsed.narration);
  const [choices, setChoices] = useState(parsed.choices.join("\n"));
  const [stateDelta, setStateDelta] = useState(parsed.state_delta);
  const [summaryDelta, setSummaryDelta] = useState(parsed.summary_delta);
  const [analysisJson, setAnalysisJson] = useState(parsed.analysis_json);
  const [stop, setStop] = useState(parsed.stop);

  const confirm = () => {
    onConfirm({
      ...parsed,
      narration,
      choices: choices.split("\n").map((s) => s.trim()).filter(Boolean),
      state_delta: stateDelta,
      summary_delta: summaryDelta,
      analysis_json: analysisJson,
      stop,
      // Manual confirmation resolves the glued-options warning.
      warnings: parsed.warnings.filter((w) => !w.startsWith(CHOICES_SUSPECT)),
    });
  };

  return (
    <div className="dialog-backdrop">
      <div className="dialog">
        <h2>人工修正解析结果</h2>
        {parsed.warnings.length > 0 && (
          <ul className="warn-list">
            {parsed.warnings.map((w, i) => <li key={i}>{w}</li>)}
          </ul>
        )}
        <div className="field">
          <span className="label">正文</span>
          <textarea className="textarea" style={{ minHeight: 160 }} value={narration}
            onChange={(e) => setNarration(e.target.value)} />
        </div>
        <div className="grid-2">
          <div className="field">
            <span className="label">选项（每行一个）</span>
            <textarea className="textarea" value={choices} onChange={(e) => setChoices(e.target.value)} />
          </div>
          <div className="field">
            <span className="label">状态增量（旧格式兼容）</span>
            <textarea className="textarea" value={stateDelta} onChange={(e) => setStateDelta(e.target.value)} />
          </div>
        </div>
        <div className="field">
          <span className="label">摘要增量</span>
          <textarea className="textarea" style={{ minHeight: 56 }} value={summaryDelta}
            onChange={(e) => setSummaryDelta(e.target.value)} />
        </div>
        <div className="field">
          <span className="label">分析段 ANALYSIS（JSON，可留空）</span>
          <textarea className="textarea mono" style={{ minHeight: 76, fontSize: 11 }}
            value={analysisJson} onChange={(e) => setAnalysisJson(e.target.value)} />
        </div>
        <div className="field">
          <label className="check-row" style={{ border: 0, padding: 0 }}>
            <input type="checkbox" checked={stop} onChange={(e) => setStop(e.target.checked)} />
            本回合停下等我行动（不勾则显示「继续阅读」）
          </label>
        </div>
        <div className="hint">修正只会写入本回合；状态卡与摘要的旧值不会被半吊子数据覆盖。</div>
        <div className="dialog-actions">
          <button className="btn" onClick={onCancel}>取消</button>
          <button className="btn btn-primary" disabled={!narration.trim()} onClick={confirm}>应用并写入</button>
        </div>
      </div>
    </div>
  );
}
