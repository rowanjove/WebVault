import React, { useState } from 'react';
import {
  History,
  Search,
  Download,
  ExternalLink,
  Clock,
  CheckCircle2,
  AlertCircle,
  Calendar,
  Layers,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api } from '../../services/tauri';
import { toast } from '../../components/ui/Toast';
import type { WaybackCaptureItem } from '../../types';

export const Wayback: React.FC = () => {
  const { sites, setSites, openTimeMachine, setSelectedSiteId } = useAppStore();

  const [url, setUrl] = useState('https://news.ycombinator.com');
  const [captures, setCaptures] = useState<WaybackCaptureItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [importingId, setImportingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hasQueried, setHasQueried] = useState(false);

  const handleQuery = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;

    try {
      setLoading(true);
      setError(null);
      setHasQueried(true);
      const results = await api.queryWayback(url.trim());
      setCaptures(results);
    } catch (err: any) {
      setError(err?.toString() || 'Wayback CDX 查询失败，请检查网络或重试');
    } finally {
      setLoading(false);
    }
  };

  const getSafeHost = (rawUrl: string): string => {
    try {
      const u = rawUrl.startsWith('http://') || rawUrl.startsWith('https://') ? rawUrl : `https://${rawUrl}`;
      return new URL(u).hostname;
    } catch {
      return 'unknown';
    }
  };

  const handleImport = async (item: WaybackCaptureItem) => {
    try {
      setImportingId(item.id);
      const host = getSafeHost(item.url);
      const safeUrl = item.url.startsWith('http://') || item.url.startsWith('https://') ? item.url : `https://${item.url}`;

      // Find or create local site
      let targetSite = sites.find((s) => s.normalized_host === host.toLowerCase());
      if (!targetSite) {
        targetSite = await api.createSite({ name: host, rootUrl: safeUrl });
      }

      await api.importWaybackCapture({ siteId: targetSite.id, capture: item });

      // Mark as imported
      setCaptures((prev) =>
        prev.map((c) => (c.id === item.id ? { ...c, imported: true } : c))
      );

      const updatedSites = await api.listSites();
      setSites(updatedSites);
      setSelectedSiteId(targetSite.id);
      toast.success('Wayback 快照已成功导入本地时光机！');
    } catch (e: any) {
      toast.error(`导入失败: ${e?.toString()}`);
    } finally {
      setImportingId(null);
    }
  };

  const formatWaybackDate = (ts: string) => {
    // format YYYYMMDDhhmmss
    if (ts.length >= 14) {
      const year = ts.slice(0, 4);
      const month = ts.slice(4, 6);
      const day = ts.slice(6, 8);
      const hour = ts.slice(8, 10);
      const min = ts.slice(10, 12);
      return `${year}-${month}-${day} ${hour}:${min}`;
    }
    return ts;
  };

  return (
    <div className="flex-1 h-screen overflow-y-auto bg-neutral-950 text-neutral-200 p-5 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between pb-3 border-b border-neutral-800">
        <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
          <History className="w-4 h-4 text-indigo-400" />
          Wayback 检索
        </h1>
      </div>

      {/* Query Bar (放大 0.5 倍) */}
      <form onSubmit={handleQuery} className="w-full flex items-center gap-3">
        <div className="relative flex-1">
          <Search className="w-4 h-4 absolute left-3.5 top-3 text-neutral-400 pointer-events-none" />
          <input
            type="text"
            placeholder="输入 URL，例如 https://news.ycombinator.com"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            className="w-full pl-10 pr-3.5 py-2.5 bg-neutral-900 border border-neutral-800 rounded-lg text-sm text-neutral-100 placeholder-neutral-500 focus:outline-hidden focus:border-indigo-500 font-mono shadow-2xs"
          />
        </div>
        <button
          type="submit"
          disabled={loading}
          className="px-5 py-2.5 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-lg text-xs font-semibold transition shrink-0 cursor-pointer active:scale-95 shadow-xs"
        >
          {loading ? '查询中...' : '查询'}
        </button>
      </form>

      {error && (
        <div className="p-3 bg-rose-50 border border-rose-200 text-rose-700 dark:bg-rose-950/80 dark:border-rose-800/60 dark:text-rose-200 rounded-lg text-xs font-medium flex items-center gap-2 select-text shadow-2xs">
          <AlertCircle className="w-4 h-4 shrink-0 text-rose-500 dark:text-rose-400" />
          <span className="break-all leading-relaxed">{error}</span>
        </div>
      )}

      {/* Snapshots List */}
      <div className="space-y-3 w-full">
        {hasQueried && !loading && (
          <div className="flex items-center justify-between pb-1.5 text-xs text-neutral-400 border-b border-neutral-800 font-medium">
            <span>共 {captures.length} 个历史快照</span>
          </div>
        )}

        {captures.length === 0 && hasQueried && !loading && (
          <div className="p-10 text-center bg-neutral-900 rounded-xl border border-neutral-800 text-neutral-400 text-sm shadow-2xs">
            未找到历史快照
          </div>
        )}

        {captures.map((cap) => {
          const isImporting = importingId === cap.id;
          return (
            <div
              key={cap.id}
              className="p-3.5 bg-neutral-900 border border-neutral-800 hover:border-neutral-700 rounded-xl transition flex items-center justify-between gap-4 text-xs shadow-2xs"
            >
              <div className="space-y-1">
                <div className="flex items-center gap-2.5">
                  <Calendar className="w-3.5 h-3.5 text-amber-500" />
                  <span className="font-semibold text-neutral-100 font-mono text-xs md:text-sm">
                    {formatWaybackDate(cap.timestamp)}
                  </span>
                  <span
                    className={`px-2 py-0.5 rounded-md text-xs font-mono font-medium ${
                      cap.status_code === '200'
                        ? 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-400'
                        : 'bg-neutral-800 text-neutral-400'
                    }`}
                  >
                    {cap.status_code}
                  </span>
                  <span className="text-xs text-neutral-400 font-mono">{cap.mime_type}</span>
                </div>
                <div className="text-xs text-neutral-400 font-mono truncate max-w-xl">
                  {cap.url}
                </div>
              </div>

              <div className="flex items-center gap-2 shrink-0">
                <a
                  href={`https://web.archive.org/web/${cap.timestamp}/${cap.url}`}
                  target="_blank"
                  rel="noreferrer"
                  className="p-1.5 rounded-lg hover:bg-neutral-800 text-neutral-400 hover:text-neutral-200 transition"
                  title="在 archive.org 查看"
                >
                  <ExternalLink className="w-4 h-4" />
                </a>

                {cap.imported ? (
                  <button
                    onClick={() => {
                      const host = getSafeHost(cap.url);
                      const targetSite = sites.find((s) => s.normalized_host === host.toLowerCase());
                      openTimeMachine({ siteId: targetSite?.id });
                    }}
                    className="flex items-center gap-1 px-2.5 py-1 rounded text-xs font-medium bg-indigo-600/20 hover:bg-indigo-600/30 text-indigo-300 transition cursor-pointer"
                  >
                    <Clock className="w-3 h-3 text-indigo-400" />
                    时光机回放
                  </button>
                ) : (
                  <button
                    onClick={() => handleImport(cap)}
                    disabled={isImporting}
                    className="flex items-center gap-1 px-2.5 py-1 rounded text-xs font-medium transition bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white cursor-pointer"
                  >
                    <Download className="w-3 h-3" />
                    {isImporting ? '导入中...' : '导入'}
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
