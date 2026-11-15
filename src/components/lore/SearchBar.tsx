interface Props {
  query: string;
  onQuery: (v: string) => void;
  categoryName: string;
  shown: number;
  total: number;
}

export default function SearchBar({ query, onQuery, categoryName, shown, total }: Props) {
  return (
    <div className="search-bar">
      <input
        className="input"
        placeholder="搜索标题 / 简介 / 标签…"
        value={query}
        onChange={(e) => onQuery(e.target.value)}
      />
      <span className="current-cat">当前设定 · {categoryName}</span>
      <span className="hint">显示 {shown} / {total} 条</span>
    </div>
  );
}
