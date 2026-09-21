import React, { useState } from 'react';
import { X, Globe, Sliders, Play, Zap, AlertCircle } from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api } from '../../services/tauri';

export const AddSiteModal: React.FC = () => {
  const { isAddSiteOpen, setIsAddSiteOpen, setSites, setCurrentTab } = useAppStore();

  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [mode, setMode] = useState<'single' | 'crawl'>('single');
  const [scopeType, setScopeType] = useState<'current' | 'prefix' | 'host' | 'domain'>('host');
  const [maxDepth, setMaxDepth] = useState(2);
  const [maxPages, setMaxPages] = useState(30);
  const [autoscroll, setAutoscroll] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isAddSiteOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) {
      setError('请输入有效的网址');
      return;
    }

    try {
      setLoading(true);
      setError(null);

      let formattedUrl = url.trim();
      if (!formattedUrl.startsWith('http://') && !formattedUrl.startsWith('https://')) {
        formattedUrl = `https://${formattedUrl}`;
      }

      const siteName = name.trim() || new URL(formattedUrl).hostname;

      if (mode === 'crawl') {
        if (!confirm(`整站抓取最多 ${maxPages} 页，会顺着链接继续访问。只要封面请改选「只要这一页」。确定继续？`)) {
          setLoading(false);
          return;
        }
      }

      const site = await api.createSite({ name: siteName, rootUrl: formattedUrl });

      if (mode === 'crawl') {
        const profile = await api.getSiteProfile(site.id);
        await api.updateSiteProfile({
          ...profile,
          scope_type: scopeType,
          max_depth: maxDepth,
          max_pages: maxPages,
          autoscroll,
        });
        await api.startCrawlJob({ siteId: site.id });
        setCurrentTab('tasks');
      } else {
        await api.startSingleCapture({ siteId: site.id, url: formattedUrl });
        setCurrentTab('tasks');
      }

      const updatedSites = await api.listSites();
      setSites(updatedSites);
      setIsAddSiteOpen(false);
      setName('');
      setUrl('');
    } catch (err: any) {
      setError(err?.toString() || '添加并抓取失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-xs p-4">
      <div className="bg-neutral-900 border border-neutral-800 rounded-2xl max-w-lg w-full p-6 shadow-2xl space-y-5">
        <div className="flex items-center justify-between pb-3.5 border-b border-neutral-800">
          <div className="flex items-center gap-2.5 text-base font-bold text-neutral-100">
            <Globe className="w-5 h-5 text-indigo-500" />
            添加站点
          </div>
          <button
            onClick={() => setIsAddSiteOpen(false)}
            className="text-neutral-400 hover:text-neutral-200 p-1.5 rounded-lg transition cursor-pointer"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {error && (
          <div className="p-3 rounded-lg bg-rose-50 border border-rose-200 text-rose-700 dark:bg-rose-950/80 dark:border-rose-800/60 dark:text-rose-200 text-xs font-medium flex items-start gap-2.5 select-text shadow-2xs">
            <AlertCircle className="w-4 h-4 text-rose-500 dark:text-rose-400 shrink-0 mt-0.5" />
            <div className="flex-1 break-all leading-relaxed">{error}</div>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4 text-sm">
          <div>
            <label className="block text-neutral-200 font-semibold mb-1.5 text-xs">网页或站点 URL</label>
            <input
              type="text"
              placeholder="https://example.com"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              required
              className="w-full px-3.5 py-2 bg-neutral-950 border border-neutral-800 rounded-lg text-neutral-100 focus:outline-hidden focus:border-indigo-500 font-mono text-sm shadow-2xs"
            />
          </div>

          <div>
            <label className="block text-neutral-200 font-semibold mb-1.5 text-xs">站点名称（可选）</label>
            <input
              type="text"
              placeholder="留空则自动使用域名"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3.5 py-2 bg-neutral-950 border border-neutral-800 rounded-lg text-neutral-100 focus:outline-hidden focus:border-indigo-500 text-sm shadow-2xs"
            />
          </div>

          {/* Mode Selector */}
          <div className="space-y-2">
            <label className="block text-neutral-200 font-semibold text-xs">抓取模式</label>
            <div className="grid grid-cols-2 gap-3">
              <button
                type="button"
                onClick={() => setMode('single')}
                className={`flex items-center gap-3 p-3 rounded-xl border text-left transition cursor-pointer ${
                  mode === 'single'
                    ? 'border-indigo-500 bg-indigo-500/10 text-indigo-400 ring-1 ring-indigo-500/30'
                    : 'border-neutral-800 bg-neutral-950/60 text-neutral-400 hover:border-neutral-700'
                }`}
              >
                <Zap className="w-5 h-5 text-indigo-500 shrink-0" />
                <div>
                  <div className="font-semibold text-neutral-100 text-sm">只要这一页</div>
                  <div className="text-xs text-neutral-400 mt-0.5">只抓封面/当前 URL，不往下爬</div>
                </div>
              </button>

              <button
                type="button"
                onClick={() => setMode('crawl')}
                className={`flex items-center gap-3 p-3 rounded-xl border text-left transition cursor-pointer ${
                  mode === 'crawl'
                    ? 'border-indigo-500 bg-indigo-500/10 text-indigo-400 ring-1 ring-indigo-500/30'
                    : 'border-neutral-800 bg-neutral-950/60 text-neutral-400 hover:border-neutral-700'
                }`}
              >
                <Sliders className="w-5 h-5 text-indigo-500 shrink-0" />
                <div>
                  <div className="font-semibold text-neutral-100 text-sm">整站抓取</div>
                  <div className="text-xs text-neutral-400 mt-0.5">会顺着链接继续抓，可能很多页</div>
                </div>
              </button>
            </div>
          </div>

          {/* Crawl Scope Settings if crawl mode is active */}
          {mode === 'crawl' && (
            <div className="p-3.5 bg-neutral-950/80 rounded-xl border border-neutral-800 space-y-3">
              <div>
                <label className="block text-neutral-300 text-xs mb-1.5 font-medium">范围 (Scope)</label>
                <select
                  value={scopeType}
                  onChange={(e) => setScopeType(e.target.value as any)}
                  className="w-full px-3 py-2 bg-neutral-900 border border-neutral-700 rounded-lg text-neutral-200 text-xs focus:outline-hidden"
                >
                  <option value="host">同主机 (Same Host)</option>
                  <option value="prefix">前缀匹配 (Prefix)</option>
                  <option value="domain">全域名 (Domain)</option>
                  <option value="current">仅当前页 (Current)</option>
                </select>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-neutral-300 text-xs mb-1.5 font-medium">最大深度</label>
                  <input
                    type="number"
                    min={1}
                    max={5}
                    value={maxDepth}
                    onChange={(e) => setMaxDepth(parseInt(e.target.value) || 1)}
                    className="w-full px-3 py-1.5 bg-neutral-900 border border-neutral-700 rounded-lg text-neutral-200 text-xs"
                  />
                </div>
                <div>
                  <label className="block text-neutral-300 text-xs mb-1.5 font-medium">最大页数</label>
                  <input
                    type="number"
                    min={10}
                    max={5000}
                    value={maxPages}
                    onChange={(e) => setMaxPages(parseInt(e.target.value) || 10)}
                    className="w-full px-3 py-1.5 bg-neutral-900 border border-neutral-700 rounded-lg text-neutral-200 text-xs"
                  />
                </div>
              </div>

              <div className="flex items-center justify-between pt-1">
                <span className="text-neutral-300 text-xs">自动滚动加载懒加载资源</span>
                <input
                  type="checkbox"
                  checked={autoscroll}
                  onChange={(e) => setAutoscroll(e.target.checked)}
                  className="accent-indigo-600 rounded w-4 h-4"
                />
              </div>
            </div>
          )}

          <div className="flex justify-end gap-3 pt-3 border-t border-neutral-800">
            <button
              type="button"
              onClick={() => setIsAddSiteOpen(false)}
              className="px-4 py-2 text-xs font-medium text-neutral-400 hover:text-neutral-200 transition cursor-pointer"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={loading}
              className="flex items-center gap-2 px-5 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-lg font-semibold text-xs transition shadow-xs cursor-pointer active:scale-95"
            >
              <Play className="w-3.5 h-3.5 fill-white" />
              {loading ? '正在启动...' : '开始抓取'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
