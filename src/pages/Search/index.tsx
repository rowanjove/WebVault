import React, { useState } from 'react';
import { Search as SearchIcon, Globe, Clock, PlaySquare, ArrowRight, Tag } from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api } from '../../services/tauri';
import type { SearchResultItem } from '../../types';

export const Search: React.FC = () => {
  const { openTimeMachine } = useAppStore();

  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResultItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [hasSearched, setHasSearched] = useState(false);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim()) return;

    try {
      setLoading(true);
      setHasSearched(true);
      const res = await api.search({ query: query.trim() });
      setResults(res);
    } catch (e) {
      console.error('Search failed:', e);
    } finally {
      setLoading(false);
    }
  };

  const handleAddFilter = (filter: string) => {
    setQuery((prev) => (prev ? `${prev} ${filter}` : filter));
  };

  return (
    <div className="flex-1 h-screen overflow-y-auto bg-neutral-950 text-neutral-200 p-5 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between pb-3 border-b border-neutral-800">
        <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
          <SearchIcon className="w-4 h-4 text-indigo-400" />
          全文搜索
        </h1>
      </div>

      {/* Search Bar & Chips - 增加 0.5 倍并向右拉到顶 (w-full) */}
      <div className="space-y-3 w-full">
        <form onSubmit={handleSearch} className="flex items-center gap-3 w-full">
          <div className="relative flex-1">
            <SearchIcon className="w-5 h-5 absolute left-3.5 top-3.5 text-neutral-400 pointer-events-none" />
            <input
              type="text"
              placeholder="输入关键词检索网页内容..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="w-full pl-11 pr-4 py-3 bg-neutral-900 border border-neutral-800 rounded-lg text-sm md:text-base text-neutral-100 placeholder-neutral-500 focus:outline-hidden focus:border-indigo-500 shadow-2xs"
            />
          </div>
          <button
            type="submit"
            disabled={loading}
            className="px-6 py-3 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-lg text-sm font-semibold transition shadow-xs shrink-0 cursor-pointer active:scale-95"
          >
            {loading ? '搜索中...' : '搜索'}
          </button>
        </form>

        {/* Syntax chips */}
        <div className="flex items-center gap-2.5 text-xs text-neutral-400 font-mono">
          <span className="text-neutral-400 font-sans font-medium">常用语法:</span>
          <button
            onClick={() => handleAddFilter('site:example.com')}
            className="px-2.5 py-1 rounded-md bg-neutral-900 hover:bg-neutral-800 border border-neutral-800 text-neutral-300 transition cursor-pointer shadow-2xs"
          >
            site:域名
          </button>
          <button
            onClick={() => handleAddFilter('title:"关键词"')}
            className="px-2.5 py-1 rounded-md bg-neutral-900 hover:bg-neutral-800 border border-neutral-800 text-neutral-300 transition cursor-pointer shadow-2xs"
          >
            title:"关键词"
          </button>
        </div>
      </div>

      {/* Search Results - 向右拉到顶 (w-full) */}
      <div className="space-y-3 w-full">
        {hasSearched && (
          <div className="text-xs text-neutral-400 flex items-center justify-between pb-2 border-b border-neutral-800">
            <span className="font-medium">共 {results.length} 条匹配结果</span>
          </div>
        )}

        {results.length === 0 && hasSearched && !loading && (
          <div className="p-10 text-center bg-neutral-900 rounded-xl border border-neutral-800 text-neutral-400 text-sm shadow-2xs">
            未找到包含该关键词的快照
          </div>
        )}

        {results.map((item) => (
          <div
            key={item.capture_id}
            className="p-4 bg-neutral-900 border border-neutral-800 hover:border-neutral-700 rounded-xl transition space-y-2 shadow-2xs"
          >
            <div className="flex items-center justify-between">
              <div
                className="text-sm md:text-base font-semibold text-neutral-100 hover:text-indigo-400 transition cursor-pointer"
                onClick={() =>
                  openTimeMachine({
                    siteId: item.site_id,
                    pageId: item.page_id,
                    captureId: item.capture_id,
                  })
                }
              >
                {item.title || item.url}
              </div>
              <span className="text-xs text-neutral-400 flex items-center gap-1.5 font-mono">
                <Clock className="w-3.5 h-3.5 text-neutral-400" />
                {new Date(item.captured_at).toLocaleString('zh-CN')}
              </span>
            </div>

            <div className="text-xs text-neutral-400 font-mono truncate">{item.url}</div>

            {/* Snippet with highlighted mark */}
            <div
              className="text-xs md:text-sm text-neutral-200 leading-relaxed bg-neutral-950/80 p-3 rounded-lg border border-neutral-800 font-mono"
              dangerouslySetInnerHTML={{ __html: item.snippet }}
            />

            <div className="flex items-center justify-between pt-1 text-xs text-neutral-400">
              <span className="font-medium">站点: {item.site_name}</span>
              <div className="flex items-center gap-2">
                <button
                  onClick={() =>
                    openTimeMachine({
                      siteId: item.site_id,
                      pageId: item.page_id,
                      captureId: item.capture_id,
                    })
                  }
                  className="px-3.5 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg font-semibold transition flex items-center gap-1.5 shadow-xs cursor-pointer active:scale-95 text-xs"
                >
                  <PlaySquare className="w-3.5 h-3.5 fill-white" />
                  时光机回放
                </button>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
