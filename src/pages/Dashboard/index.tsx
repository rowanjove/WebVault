import React, { useEffect, useState } from 'react';
import {
  LayoutDashboard,
  Globe,
  Clock,
  Layers,
  HardDrive,
  ArrowUpRight,
  Play,
  Zap,
  Activity,
  AlertCircle,
  CheckCircle2,
  GitCompare,
  ClipboardPaste,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api } from '../../services/tauri';
import { toast } from '../../components/ui/Toast';
import type { Site, ChangeEventItem, CrawlJob } from '../../types';

export const Dashboard: React.FC = () => {
  const {
    stats,
    setStats,
    sites,
    setSites,
    setCurrentTab,
    setSelectedSiteId,
    setSelectedCaptureId,
    setDiffPair,
    jobs,
    setJobs,
    openTimeMachine,
    setIsAddSiteOpen,
  } = useAppStore();

  const [quickUrl, setQuickUrl] = useState('');
  const [quickLoading, setQuickLoading] = useState(false);
  const [changeEvents, setChangeEvents] = useState<ChangeEventItem[]>([]);

  const loadData = async () => {
    try {
      const [s, st, j, ce] = await Promise.all([
        api.listSites(),
        api.getSystemStats(),
        api.listJobs(),
        api.listChangeEvents(),
      ]);
      setSites(s);
      setStats(st);
      setJobs(j);
      setChangeEvents(ce);
    } catch (e) {
      console.error('Failed to load dashboard data:', e);
    }
  };

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 5000);
    return () => clearInterval(interval);
  }, []);

  const handleQuickCapture = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!quickUrl.trim()) return;

    try {
      setQuickLoading(true);
      let target = quickUrl.trim();
      if (!target.startsWith('http://') && !target.startsWith('https://')) {
        target = `https://${target}`;
      }
      const host = new URL(target).hostname;

      // Find or create site
      let site = sites.find((s) => s.normalized_host === host.toLowerCase());
      if (!site) {
        site = await api.createSite({ name: host, rootUrl: target });
      }

      await api.startSingleCapture({ siteId: site.id, url: target });
      setQuickUrl('');
      toast.success('已加入抓取队列');
      setCurrentTab('tasks');
    } catch (err: any) {
      toast.error(`抓取失败: ${err?.toString?.() || '未知错误'}`);
    } finally {
      setQuickLoading(false);
    }
  };

  const handlePasteClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText();
      const match = text.match(/https?:\/\/[^\s]+/i);
      if (match) {
        setQuickUrl(match[0]);
      } else if (text.trim()) {
        setQuickUrl(text.trim());
      }
    } catch (e) {
      console.error('Failed to read clipboard:', e);
    }
  };

  const formatBytes = (bytes?: number) => {
    if (!bytes) return '0 MB';
    const mb = bytes / (1024 * 1024);
    if (mb < 1024) return `${mb.toFixed(1)} MB`;
    return `${(mb / 1024).toFixed(2)} GB`;
  };

  const formatDate = (ts?: number | null) => {
    if (!ts) return '-';
    return new Date(ts).toLocaleString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  return (
    <div className="flex-1 h-screen overflow-y-auto bg-neutral-950 text-neutral-200 p-5 space-y-4">
      {/* Top Welcome & Instant Bar */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-3 border-b border-neutral-800">
        <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
          <LayoutDashboard className="w-4 h-4 text-indigo-400" />
          仪表盘
        </h1>

        {/* Quick URL Capture Bar */}
        <form onSubmit={handleQuickCapture} className="flex items-center gap-2 max-w-lg w-full">
          <div className="flex-1 relative flex items-center">
            <input
              type="text"
              placeholder="输入网页 URL 进行抓取..."
              value={quickUrl}
              onChange={(e) => setQuickUrl(e.target.value)}
              className="w-full pl-3.5 pr-9 py-2.5 bg-neutral-900 border border-neutral-800 rounded-lg text-sm text-neutral-100 placeholder-neutral-500 focus:outline-hidden focus:border-indigo-500 font-mono shadow-2xs"
            />
            <button
              type="button"
              onClick={handlePasteClipboard}
              className="absolute right-2.5 p-1 text-neutral-400 hover:text-neutral-200 transition cursor-pointer"
              title="从剪贴板粘贴 URL"
            >
              <ClipboardPaste className="w-4 h-4" />
            </button>
          </div>
          <button
            type="submit"
            disabled={quickLoading}
            className="flex items-center gap-2 px-4 py-2.5 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-lg text-sm font-semibold transition shrink-0 shadow-xs cursor-pointer active:scale-95"
          >
            <Zap className="w-4 h-4 fill-white" />
            {quickLoading ? '抓取中...' : '抓取'}
          </button>
        </form>
      </div>

      {/* Metrics Row */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 shadow-2xs">
          <div className="flex items-center justify-between text-neutral-400 text-sm font-medium">
            <span>站点数</span>
            <Globe className="w-4 h-4 text-neutral-400" />
          </div>
          <div className="text-2xl font-bold font-mono text-neutral-100">
            {stats?.site_count || 0}
          </div>
        </div>

        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 shadow-2xs">
          <div className="flex items-center justify-between text-neutral-400 text-sm font-medium">
            <span>快照总数</span>
            <Clock className="w-4 h-4 text-neutral-400" />
          </div>
          <div className="text-2xl font-bold font-mono text-neutral-100">
            {stats?.capture_count || 0}
          </div>
        </div>

        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 shadow-2xs">
          <div className="flex items-center justify-between text-neutral-400 text-sm font-medium">
            <span>已存资源数</span>
            <Layers className="w-4 h-4 text-neutral-400" />
          </div>
          <div className="text-2xl font-bold font-mono text-neutral-100">
            {stats?.resource_count || 0}
          </div>
        </div>

        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 shadow-2xs">
          <div className="flex items-center justify-between text-neutral-400 text-sm font-medium">
            <span>归档存储占用</span>
            <HardDrive className="w-4 h-4 text-neutral-400" />
          </div>
          <div className="text-2xl font-bold font-mono text-neutral-100">
            {formatBytes(stats?.storage_bytes)}
          </div>
        </div>
      </div>

      {/* Main Grid: Sites & Recent Activity */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-5">
        {/* Sites list (2 cols) */}
        <div className="lg:col-span-2 space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold text-neutral-100 flex items-center gap-2">
              <Globe className="w-4 h-4 text-indigo-500" />
              站点列表
            </h2>
            <button
              onClick={() => setIsAddSiteOpen(true)}
              className="text-xs font-medium text-indigo-500 hover:text-indigo-400 transition cursor-pointer"
            >
              + 添加站点
            </button>
          </div>

          {sites.length === 0 ? (
            <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-8 text-center space-y-3 shadow-2xs">
              <Globe className="w-8 h-8 text-neutral-400 mx-auto" />
              <div className="text-sm text-neutral-200 font-semibold">暂无站点记录</div>
              <p className="text-xs text-neutral-400 max-w-sm mx-auto">
                在上方输入 URL 或点击下方按钮添加站点开始抓取。
              </p>
              <button
                onClick={() => setIsAddSiteOpen(true)}
                className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition inline-flex items-center gap-2 shadow-xs cursor-pointer active:scale-95"
              >
                <Zap className="w-3.5 h-3.5 fill-white" />
                添加站点
              </button>
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3.5">
              {sites.map((site) => (
                <div
                  key={site.id}
                  className="bg-neutral-900 border border-neutral-800 hover:border-neutral-700 rounded-xl p-5 transition space-y-3 flex flex-col justify-between shadow-2xs"
                >
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <div className="text-base font-semibold text-neutral-100 truncate max-w-[180px]">
                        {site.name}
                      </div>
                      <span className="text-xs font-mono font-medium px-2 py-0.5 bg-neutral-800 text-neutral-300 rounded">
                        {formatBytes(site.storage_bytes)}
                      </span>
                    </div>
                    <div className="text-xs text-neutral-400 truncate font-mono">
                      {site.root_url}
                    </div>
                  </div>

                  <div className="flex items-center justify-between text-xs text-neutral-400 pt-2.5 border-t border-neutral-800/80">
                    <div>
                      <span className="font-medium">{site.capture_count || 0} 快照</span>
                      <span className="mx-1.5 text-neutral-400">·</span>
                      <span>{formatDate(site.last_capture_at)}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => openTimeMachine({ siteId: site.id })}
                        className="px-2.5 py-1 text-xs bg-neutral-800 hover:bg-neutral-700 text-neutral-200 rounded-md transition flex items-center gap-1 font-medium cursor-pointer"
                      >
                        <Clock className="w-3.5 h-3.5 text-indigo-400" />
                        时光机
                      </button>
                      <button
                        onClick={() => {
                          setSelectedSiteId(site.id);
                          setCurrentTab('sites');
                        }}
                        className="px-2.5 py-1 text-xs bg-indigo-600/15 hover:bg-indigo-600/25 text-indigo-600 dark:text-indigo-300 rounded-md transition flex items-center gap-1 font-medium cursor-pointer"
                      >
                        详情
                        <ArrowUpRight className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Right column: Recent Changes & Jobs */}
        <div className="space-y-4">
          {/* Active / Recent Jobs */}
          <div className="space-y-2.5">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-neutral-100 flex items-center gap-2">
                <Activity className="w-4 h-4 text-emerald-500" />
                近期任务
              </h2>
              <button
                onClick={() => setCurrentTab('tasks')}
                className="text-xs font-medium text-neutral-400 hover:text-neutral-200 transition cursor-pointer"
              >
                查看全部
              </button>
            </div>

            <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-3.5 space-y-2.5 shadow-2xs">
              {jobs.length === 0 ? (
                <div className="text-center py-5 text-neutral-400 text-xs">
                  暂无任务记录
                </div>
              ) : (
                jobs.slice(0, 3).map((job) => (
                  <div
                    key={job.id}
                    className="p-3 rounded-lg bg-neutral-950/70 border border-neutral-800 space-y-1.5"
                  >
                    <div className="flex items-center justify-between text-xs">
                      <span className="font-mono text-neutral-200 text-xs truncate max-w-[140px] font-medium">
                        {job.id}
                      </span>
                      <span
                        className={`text-xs px-2 py-0.5 rounded font-mono font-medium ${
                          job.status === 'running'
                            ? 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-400'
                            : job.status === 'completed'
                            ? 'bg-neutral-800 text-neutral-400'
                            : job.status === 'cancelled' || job.status === 'queued' || job.status === 'paused'
                            ? 'bg-neutral-800 text-neutral-500'
                            : 'bg-red-500/20 text-red-500 dark:text-red-400'
                        }`}
                      >
                        {job.status}
                      </span>
                    </div>
                    <div className="flex items-center justify-between text-xs text-neutral-400">
                      <span>已抓取: <strong className="text-neutral-200">{job.pages_captured}</strong> 页</span>
                      <span className="font-mono font-medium text-neutral-300">{formatBytes(job.bytes_written)}</span>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>

          {/* Change Events Feed */}
          <div className="space-y-2.5">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-neutral-100 flex items-center gap-2">
                <GitCompare className="w-4 h-4 text-amber-500" />
                监测变动
              </h2>
              <button
                onClick={() => setCurrentTab('monitor')}
                className="text-xs font-medium text-neutral-400 hover:text-neutral-200 transition cursor-pointer"
              >
                监控中心
              </button>
            </div>

            <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-3.5 space-y-2.5 shadow-2xs">
              {changeEvents.length === 0 ? (
                <div className="text-center py-5 text-neutral-400 text-xs">
                  暂无内容变动
                </div>
              ) : (
                changeEvents.slice(0, 4).map((event) => (
                  <div
                    key={event.id}
                    className="p-3 rounded-lg bg-neutral-950/70 border border-neutral-800 text-xs space-y-1.5"
                  >
                    <div className="flex items-center justify-between">
                      <span className="text-neutral-200 font-medium truncate max-w-[160px] text-xs">
                        {event.title || event.url}
                      </span>
                      <span className="text-xs font-mono font-medium px-2 py-0.5 bg-amber-500/15 text-amber-600 dark:text-amber-300 rounded">
                        变动 {event.change_score}%
                      </span>
                    </div>
                    <div className="flex items-center justify-between text-xs text-neutral-400">
                      <span>{formatDate(event.created_at)}</span>
                      {event.old_capture_id && (
                        <button
                          onClick={() =>
                            setDiffPair(event.old_capture_id!, event.new_capture_id, {
                              siteId: event.site_id,
                              pageId: event.page_id,
                            })
                          }
                          className="text-indigo-600 dark:text-indigo-400 hover:underline text-xs font-medium cursor-pointer"
                        >
                          比对 →
                        </button>
                      )}
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
