import React, { useEffect, useState } from 'react';
import {
  ArrowLeft,
  ArrowRight,
  RotateCw,
  Globe,
  Lock,
  Layers,
  ExternalLink,
  ChevronDown,
  ChevronUp,
  Search,
  FileCode,
  Image as ImageIcon,
  Clock,
  FileDown,
  Printer,
  PlaySquare,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api } from '../../services/tauri';
import { toast } from '../../components/ui/Toast';
import type { CaptureItem, ResourceItem } from '../../types';

export const Replay: React.FC = () => {
  const { selectedCaptureId, sites, setCurrentTab } = useAppStore();

  const [capture, setCapture] = useState<CaptureItem | null>(null);
  const [resources, setResources] = useState<ResourceItem[]>([]);
  const [replayUrl, setReplayUrl] = useState<string>('');
  const [showNetwork, setShowNetwork] = useState(false);
  const [resourceFilter, setResourceFilter] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (selectedCaptureId) {
      setLoading(true);
      api
        .getCaptureDetails(selectedCaptureId)
        .then((res) => {
          setCapture(res.capture);
          setResources(res.resources);
        })
        .finally(() => setLoading(false));

      api.getReplayUrl(selectedCaptureId).then(setReplayUrl);
    }
  }, [selectedCaptureId]);

  const filteredResources = resources.filter(
    (r) =>
      r.url.toLowerCase().includes(resourceFilter.toLowerCase()) ||
      r.mime_type?.toLowerCase().includes(resourceFilter.toLowerCase())
  );

  const formatSize = (bytes: number) => {
    if (!bytes) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  };

  const handleExportOffline = async (format: 'singlefile' | 'pdf') => {
    if (!selectedCaptureId) return;
    const defaultName = `webvault_${selectedCaptureId}_${Date.now()}.${format === 'singlefile' ? 'html' : 'pdf'}`;
    const targetPath = prompt(`请输入导出文件保存路径或文件名：`, defaultName);
    if (!targetPath) return;
    try {
      const resultPath = await api.exportPageOffline({ captureId: selectedCaptureId, format, outputPath: targetPath });
      toast.success(`脱机文件 (${format === 'singlefile' ? 'HTML' : 'PDF'}) 已导出：${resultPath || targetPath}`);
    } catch (e: any) {
      toast.error(`导出失败: ${e?.toString()}`);
    }
  };

  return (
    <div className="flex-1 h-screen flex flex-col overflow-hidden bg-neutral-950 text-neutral-200">
      {/* Top Browser Bar */}
      <div className="px-5 py-3 border-b border-neutral-800 bg-neutral-900 flex items-center justify-between gap-4 shrink-0 shadow-2xs">
        <div className="flex items-center gap-3 shrink-0">
          <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
            <PlaySquare className="w-4 h-4 text-indigo-400" />
            快照回放
          </h1>
          <div className="h-4 w-px bg-neutral-800" />
          <div className="flex items-center gap-1 text-neutral-400">
            <button
              onClick={() => setCurrentTab('timeline')}
              className="p-1.5 rounded-lg hover:bg-neutral-800 hover:text-neutral-100 transition cursor-pointer"
              title="返回时间线"
            >
              <ArrowLeft className="w-4 h-4" />
            </button>
            <button
              onClick={() => {
                const iframe = document.getElementById('replay-frame') as HTMLIFrameElement;
                if (iframe) iframe.src = replayUrl;
              }}
              className="p-1.5 rounded-lg hover:bg-neutral-800 hover:text-neutral-100 transition cursor-pointer"
              title="刷新"
            >
              <RotateCw className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Address Input & Status Pill (高度增加 0.5 倍) */}
        <div className="flex-1 max-w-4xl flex items-center bg-neutral-950 border border-neutral-800 rounded-lg px-3.5 py-2 text-sm gap-3 shadow-2xs">
          {/* ARCHIVE status badge */}
          <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-neutral-800 text-neutral-200 font-mono text-xs font-semibold shrink-0">
            <Lock className="w-3.5 h-3.5 text-neutral-400" />
            <span>离线快照</span>
          </div>

          <div className="flex-1 text-neutral-100 font-mono text-xs md:text-sm truncate">
            {capture?.url || 'https://...'}
          </div>

          {capture && (
            <div className="text-xs text-neutral-400 shrink-0 flex items-center gap-1.5 font-mono">
              <Clock className="w-3.5 h-3.5 text-neutral-400" />
              <span>{new Date(capture.captured_at).toLocaleString('zh-CN')}</span>
            </div>
          )}
        </div>

        {/* Right Actions */}
        <div className="flex items-center gap-2.5">
          <button
            onClick={() => handleExportOffline('singlefile')}
            className="flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium transition border bg-neutral-900 border-neutral-800 text-neutral-200 hover:bg-neutral-800 cursor-pointer shadow-2xs"
            title="导出为完全脱机的单 HTML 文件"
          >
            <FileDown className="w-4 h-4 text-cyan-500" />
            <span>导出 HTML</span>
          </button>

          <button
            onClick={() => handleExportOffline('pdf')}
            className="flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium transition border bg-neutral-900 border-neutral-800 text-neutral-200 hover:bg-neutral-800 cursor-pointer shadow-2xs"
            title="导出为 PDF"
          >
            <Printer className="w-4 h-4 text-emerald-500" />
            <span>导出 PDF</span>
          </button>

          <button
            onClick={() => setShowNetwork(!showNetwork)}
            className={`flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium transition border cursor-pointer shadow-2xs ${
              showNetwork
                ? 'bg-neutral-800 border-neutral-700 text-neutral-100 font-semibold'
                : 'bg-neutral-900 border-neutral-800 text-neutral-300 hover:bg-neutral-800'
            }`}
          >
            <Layers className="w-4 h-4" />
            <span>网络资源 ({resources.length})</span>
            {showNetwork ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronUp className="w-3.5 h-3.5" />}
          </button>

          {capture?.url && (
            <a
              href={capture.url}
              target="_blank"
              rel="noreferrer"
              className="p-2 rounded-lg hover:bg-neutral-800 text-neutral-400 hover:text-neutral-200 transition"
              title="访问当前在线版本"
            >
              <ExternalLink className="w-4 h-4" />
            </a>
          )}
        </div>
      </div>

      {/* Main Sandbox Iframe View */}
      <div className="flex-1 relative bg-neutral-900 flex flex-col overflow-hidden">
        {selectedCaptureId && replayUrl ? (
          <iframe
            id="replay-frame"
            src={replayUrl}
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
            referrerPolicy="no-referrer"
            className="w-full h-full border-0 bg-white"
            title="Archived Webpage Replay Sandbox"
          />
        ) : (
          <div className="flex-1 flex items-center justify-center text-neutral-500 text-xs">
            未选择快照，请在时间线中选择快照进行回放。
          </div>
        )}

        {/* Collapsible Network Drawer */}
        {showNetwork && (
          <div className="h-72 border-t border-neutral-800 bg-neutral-950 flex flex-col shrink-0">
            <div className="p-2.5 border-b border-neutral-800 bg-neutral-900/60 flex items-center justify-between text-xs">
              <div className="flex items-center gap-2">
                <span className="font-semibold text-neutral-200">资源列表</span>
                <span className="text-neutral-500 font-mono text-xs">
                  共 {resources.length} 项
                </span>
              </div>
              <div className="relative w-64">
                <Search className="w-3.5 h-3.5 absolute left-2.5 top-2 text-neutral-500" />
                <input
                  type="text"
                  placeholder="过滤 URL 或 MIME 类型..."
                  value={resourceFilter}
                  onChange={(e) => setResourceFilter(e.target.value)}
                  className="w-full pl-8 pr-2 py-1 bg-neutral-950 border border-neutral-800 rounded text-xs text-neutral-200 focus:outline-hidden"
                />
              </div>
            </div>

            <div className="flex-1 overflow-y-auto font-mono text-xs">
              <table className="w-full text-left border-collapse">
                <thead className="bg-neutral-900/40 text-neutral-400 sticky top-0 border-b border-neutral-800">
                  <tr>
                    <th className="p-2">状态</th>
                    <th className="p-2">资源类型</th>
                    <th className="p-2">URL 路径</th>
                    <th className="p-2 text-right">体积</th>
                    <th className="p-2">SHA-256</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-neutral-900">
                  {filteredResources.map((res) => (
                    <tr key={res.id} className="hover:bg-neutral-900/60">
                      <td className="p-2">
                        <span
                          className={`px-1.5 py-0.5 rounded text-xs ${
                            res.status_code === 200
                              ? 'bg-emerald-500/20 text-emerald-400'
                              : 'bg-amber-500/20 text-amber-400'
                          }`}
                        >
                          {res.status_code}
                        </span>
                      </td>
                      <td className="p-2 text-neutral-400 truncate max-w-[120px]">
                        {res.mime_type || 'unknown'}
                      </td>
                      <td className="p-2 text-neutral-200 truncate max-w-md">
                        {res.url}
                      </td>
                      <td className="p-2 text-right text-neutral-400">
                        {formatSize(res.size)}
                      </td>
                      <td className="p-2 text-neutral-500 text-xs truncate max-w-[100px]">
                        {res.sha256 ? res.sha256.slice(0, 12) : '-'}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
