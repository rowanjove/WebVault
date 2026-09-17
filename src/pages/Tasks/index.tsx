import React, { useEffect, useState } from 'react';
import {
  ListTodo,
  Play,
  Pause,
  XCircle,
  Clock,
  Layers,
  HardDrive,
  AlertTriangle,
  RotateCw,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api } from '../../services/tauri';
import type { CrawlJob } from '../../types';

export const Tasks: React.FC = () => {
  const { jobs, setJobs, sites, openTimeMachine } = useAppStore();
  const [loading, setLoading] = useState(false);

  const loadJobs = async () => {
    try {
      const list = await api.listJobs();
      setJobs(list);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    loadJobs();
    const interval = setInterval(loadJobs, 2000);
    return () => clearInterval(interval);
  }, []);

  const handleCancel = async (id: string) => {
    try {
      await api.cancelJob(id);
      loadJobs();
    } catch (e) {
      console.error(e);
    }
  };

  const formatBytes = (bytes: number) => {
    if (!bytes) return '0 MB';
    const mb = bytes / (1024 * 1024);
    if (mb < 1024) return `${mb.toFixed(1)} MB`;
    return `${(mb / 1024).toFixed(2)} GB`;
  };

  return (
    <div className="flex-1 h-screen overflow-y-auto bg-neutral-950 text-neutral-200 p-5 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between pb-3 border-b border-neutral-800">
        <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
          <ListTodo className="w-4 h-4 text-indigo-400" />
          任务队列
        </h1>

        <button
          onClick={loadJobs}
          className="p-1 rounded hover:bg-neutral-800 text-neutral-400 hover:text-neutral-200 transition"
          title="刷新任务列表"
        >
          <RotateCw className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Task List - 铺展全宽并放大字体 */}
      <div className="space-y-3.5 w-full">
        {jobs.length === 0 ? (
          <div className="p-10 text-center bg-neutral-900 rounded-xl border border-neutral-800 text-neutral-400 text-sm shadow-2xs">
            暂无进行中或历史任务
          </div>
        ) : (
          jobs.map((job) => {
            const site = sites.find((s) => s.id === job.site_id);
            const isRunning = job.status === 'running';

            return (
              <div
                key={job.id}
                className="p-4 bg-neutral-900 border border-neutral-800 rounded-xl space-y-3 transition shadow-2xs"
              >
                <div className="flex items-center justify-between">
                  <div className="space-y-1">
                    <div className="flex items-center gap-2.5">
                      <span className="text-sm font-bold text-neutral-100">
                        {site ? site.name : job.site_id}
                      </span>
                      <span className="text-xs font-mono text-neutral-400 bg-neutral-800 px-1.5 py-0.5 rounded">{job.id}</span>
                    </div>
                    <div className="text-xs text-neutral-400 font-mono">
                      {site?.root_url || 'https://...'}
                    </div>
                  </div>

                  <div className="flex items-center gap-3">
                    <span
                      className={`text-xs px-2.5 py-1 rounded-md font-mono font-semibold ${
                        isRunning
                          ? 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-300 border border-emerald-500/30 animate-pulse'
                          : job.status === 'completed'
                          ? 'bg-neutral-800 text-neutral-300'
                          : 'bg-red-500/20 text-red-400'
                      }`}
                    >
                      {job.status.toUpperCase()}
                    </span>

                    {job.status === 'completed' && (
                      <button
                        onClick={() => openTimeMachine({ siteId: job.site_id })}
                        className="flex items-center gap-1.5 px-3 py-1 text-xs bg-indigo-600/20 hover:bg-indigo-600/30 text-indigo-300 rounded-md border border-indigo-500/30 transition cursor-pointer font-medium"
                      >
                        <Clock className="w-3.5 h-3.5 text-indigo-400" />
                        进入时光机
                      </button>
                    )}

                    {isRunning && (
                      <button
                        onClick={() => handleCancel(job.id)}
                        className="flex items-center gap-1.5 px-3 py-1 text-xs bg-red-950/60 hover:bg-red-900/60 text-red-300 rounded-md border border-red-800/60 transition cursor-pointer"
                      >
                        <XCircle className="w-3.5 h-3.5" />
                        终止
                      </button>
                    )}
                  </div>
                </div>

                {/* Metrics Grid (字号放大 0.5 倍) */}
                <div className="grid grid-cols-4 gap-3 bg-neutral-950/80 p-3 rounded-lg border border-neutral-800 text-xs">
                  <div>
                    <span className="text-neutral-400 block text-xs mb-0.5">已发现</span>
                    <span className="font-mono font-bold text-sm text-neutral-200">
                      {job.pages_discovered}
                    </span>
                  </div>

                  <div>
                    <span className="text-neutral-400 block text-xs mb-0.5">已抓取</span>
                    <span className="font-mono font-bold text-sm text-emerald-500">
                      {job.pages_captured}
                    </span>
                  </div>

                  <div>
                    <span className="text-neutral-400 block text-xs mb-0.5">资源数</span>
                    <span className="font-mono font-bold text-sm text-indigo-400">
                      {job.resources_captured}
                    </span>
                  </div>

                  <div>
                    <span className="text-neutral-400 block text-xs mb-0.5">写入大小</span>
                    <span className="font-mono font-bold text-sm text-purple-400">
                      {formatBytes(job.bytes_written)}
                    </span>
                  </div>
                </div>

                <div className="flex items-center justify-between text-xs text-neutral-400 pt-0.5">
                  <span>
                    开始: {new Date(job.started_at).toLocaleString('zh-CN')}
                  </span>
                  {job.finished_at && (
                    <span>
                      结束: {new Date(job.finished_at).toLocaleString('zh-CN')}
                    </span>
                  )}
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
