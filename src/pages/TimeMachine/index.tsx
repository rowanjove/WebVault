import React, { useEffect, useState, useMemo } from 'react';
import {
  Clock,
  PlaySquare,
  GitCompare,
  Layers,
  ArrowLeft,
  ArrowRight,
  RotateCw,
  ExternalLink,
  ShieldCheck,
  Lock,
  FileDown,
  Printer,
  Bookmark,
  Tag as TagIcon,
  Plus,
  X,
  Search,
  Activity,
  Globe,
  CheckCircle2,
  Calendar,
  ChevronLeft,
  ChevronRight,
  FileCode,
  Sliders,
  Minus,
} from 'lucide-react';
import { useAppStore, TimeMachineMode } from '../../stores/useAppStore';
import { api, pickSaveFile } from '../../services/tauri';
import { toast } from '../../components/ui/Toast';
import type { PageItem, CaptureItem, ResourceItem, DiffResult, TagItem, BookmarkItem, MonitorRule } from '../../types';

export const TimeMachine: React.FC = () => {
  const {
    sites,
    selectedSiteId,
    setSelectedSiteId,
    selectedPageId,
    setSelectedPageId,
    selectedCaptureId,
    setSelectedCaptureId,
    timeMachineMode,
    setTimeMachineMode,
    diffOldCaptureId,
    diffNewCaptureId,
    setDiffPair,
    setIsAddSiteOpen,
  } = useAppStore();

  // Navigation / Filter state
  const [pageFilter, setPageFilter] = useState('');
  const [pages, setPages] = useState<PageItem[]>([]);
  const [captures, setCaptures] = useState<CaptureItem[]>([]);
  const [loadingCaptures, setLoadingCaptures] = useState(false);

  // Active capture details & replay
  const [currentCapture, setCurrentCapture] = useState<CaptureItem | null>(null);
  const [resources, setResources] = useState<ResourceItem[]>([]);
  const [replayUrl, setReplayUrl] = useState<string>('');
  const [resourceFilter, setResourceFilter] = useState('');
  const [iframeLoading, setIframeLoading] = useState(false);

  // Diff state
  const [diffResult, setDiffResult] = useState<DiffResult | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [diffSubTab, setDiffSubTab] = useState<'text' | 'resources' | 'visual' | 'dom'>('text');
  const [compareTargetId, setCompareTargetId] = useState<string>('');

  // Bookmarks, Tags & Monitor states
  const [bookmarks, setBookmarks] = useState<BookmarkItem[]>([]);
  const [pageTags, setPageTags] = useState<TagItem[]>([]);
  const [allTags, setAllTags] = useState<TagItem[]>([]);
  const [isTagDropdownOpen, setIsTagDropdownOpen] = useState(false);
  const [newTagName, setNewTagName] = useState('');
  const [isMonitored, setIsMonitored] = useState(false);
  const [monitoringLoading, setMonitoringLoading] = useState(false);

  // 1. Ensure a site is selected
  useEffect(() => {
    if (!selectedSiteId && sites.length > 0) {
      setSelectedSiteId(sites[0].id);
    }
  }, [sites, selectedSiteId]);

  // 2. Load pages when selectedSiteId changes
  useEffect(() => {
    if (selectedSiteId) {
      api.listPages(selectedSiteId).then((p) => {
        setPages(p);
        if (p.length > 0) {
          const currentValid = p.some((item) => item.id === selectedPageId);
          if (!currentValid) {
            setSelectedPageId(p[0].id);
          }
        } else {
          setSelectedPageId(null);
          setCaptures([]);
        }
      }).catch(console.error);
    } else {
      setPages([]);
      setSelectedPageId(null);
    }
  }, [selectedSiteId]);

  // 3. Load captures when selectedPageId changes
  useEffect(() => {
    if (selectedPageId) {
      setLoadingCaptures(true);
      api
        .listCaptures(selectedPageId)
        .then((caps) => {
          setCaptures(caps);
          if (caps.length > 0) {
            // Keep selected capture if valid, otherwise pick the newest
            const currentValid = caps.some((c) => c.id === selectedCaptureId);
            if (!currentValid) {
              setSelectedCaptureId(caps[0].id);
            }
          } else {
            setSelectedCaptureId(null);
            setCurrentCapture(null);
            setReplayUrl('');
          }
        })
        .catch(console.error)
        .finally(() => setLoadingCaptures(false));

      // Load tags
      api.getPageTags(selectedPageId).then(setPageTags).catch(() => {});
      api.listTags().then(setAllTags).catch(() => {});

      // Check monitor status
      checkMonitorStatus(selectedPageId);
    } else {
      setCaptures([]);
      setSelectedCaptureId(null);
      setCurrentCapture(null);
      setReplayUrl('');
    }
  }, [selectedPageId]);

  // 4. Load capture details & replay URL when selectedCaptureId changes
  useEffect(() => {
    if (selectedCaptureId) {
      setIframeLoading(true);
      api.getCaptureDetails(selectedCaptureId)
        .then((res) => {
          setCurrentCapture(res.capture);
          setResources(res.resources);
        })
        .catch(console.error);

      api.getReplayUrl(selectedCaptureId)
        .then((url) => {
          setReplayUrl(url);
        })
        .catch(console.error)
        .finally(() => setIframeLoading(false));
    }
  }, [selectedCaptureId]);

  // 5. Load Bookmarks
  const loadBookmarks = async () => {
    try {
      const bms = await api.listBookmarks();
      setBookmarks(bms);
    } catch {}
  };
  useEffect(() => {
    loadBookmarks();
  }, []);

  // 6. Monitor status check & toggle
  const checkMonitorStatus = async (pageId: string) => {
    try {
      const targetPage = pages.find((p) => p.id === pageId);
      if (!targetPage) return;
      const rules = await api.listMonitorRules();
      const match = rules.some((r) => r.url === targetPage.url && r.enabled);
      setIsMonitored(match);
    } catch {}
  };

  const handleToggleMonitor = async () => {
    const activePage = pages.find((p) => p.id === selectedPageId);
    if (!activePage || !selectedSiteId) return;

    try {
      setMonitoringLoading(true);
      const rules = await api.listMonitorRules();
      const existing = rules.find((r) => r.url === activePage.url);

      if (existing) {
        await api.toggleMonitorRule(existing.id, !isMonitored);
        setIsMonitored(!isMonitored);
        toast.success(!isMonitored ? '已启用该页面的定时监控' : '已暂停该页面的定时监控');
      } else {
        await api.createMonitorRule({
          site_id: selectedSiteId,
          url: activePage.url,
          schedule: '1h',
          strategy: 'content_hash',
          enabled: true,
        });
        setIsMonitored(true);
        toast.success('已将当前页面加入定时监控 (每小时检查一次)');
      }
    } catch (e: any) {
      toast.error(`监控设置失败: ${e?.toString()}`);
    } finally {
      setMonitoringLoading(false);
    }
  };

  // Apply store diff pair when entering compare mode from Monitor / Dashboard
  useEffect(() => {
    if (timeMachineMode === 'diff' && diffOldCaptureId && diffNewCaptureId) {
      setCompareTargetId(diffOldCaptureId);
      if (selectedCaptureId !== diffNewCaptureId) {
        setSelectedCaptureId(diffNewCaptureId);
      }
    }
  }, [timeMachineMode, diffOldCaptureId, diffNewCaptureId]);

  // 7. Diff calculation
  useEffect(() => {
    if (timeMachineMode === 'diff' && captures.length > 1) {
      const currentIdx = captures.findIndex((c) => c.id === selectedCaptureId);
      let targetId = compareTargetId;
      if (diffOldCaptureId && captures.some((c) => c.id === diffOldCaptureId)) {
        targetId = diffOldCaptureId;
        if (targetId !== compareTargetId) {
          setCompareTargetId(targetId);
        }
      } else if (!targetId || targetId === selectedCaptureId) {
        if (currentIdx !== -1 && currentIdx + 1 < captures.length) {
          targetId = captures[currentIdx + 1].id;
        } else if (currentIdx > 0) {
          targetId = captures[0].id;
        } else {
          targetId = captures[1]?.id || '';
        }
        setCompareTargetId(targetId);
      }

      if (selectedCaptureId && targetId && selectedCaptureId !== targetId) {
        setDiffLoading(true);
        api.compareCaptures({ oldCaptureId: targetId, newCaptureId: selectedCaptureId })
          .then(setDiffResult)
          .catch(console.error)
          .finally(() => setDiffLoading(false));
      }
    }
  }, [timeMachineMode, selectedCaptureId, compareTargetId, captures]);

  // Active entities
  const activeSite = sites.find((s) => s.id === selectedSiteId);
  const activePage = pages.find((p) => p.id === selectedPageId);

  // Filtered pages
  const filteredPages = useMemo(() => {
    if (!pageFilter.trim()) return pages;
    const q = pageFilter.toLowerCase();
    return pages.filter(
      (p) => (p.title && p.title.toLowerCase().includes(q)) || p.url.toLowerCase().includes(q)
    );
  }, [pages, pageFilter]);

  // Current capture index in chronologically ordered captures (0 is newest)
  const currentCaptureIndex = captures.findIndex((c) => c.id === selectedCaptureId);
  const hasOlder = currentCaptureIndex !== -1 && currentCaptureIndex < captures.length - 1;
  const hasNewer = currentCaptureIndex > 0;

  const handleSelectOlder = () => {
    if (hasOlder) {
      setSelectedCaptureId(captures[currentCaptureIndex + 1].id);
    }
  };

  const handleSelectNewer = () => {
    if (hasNewer) {
      setSelectedCaptureId(captures[currentCaptureIndex - 1].id);
    }
  };

  const handleExportOffline = async (format: 'singlefile' | 'pdf') => {
    if (!selectedCaptureId) return;
    const ext = format === 'singlefile' ? 'html' : 'pdf';
    const defaultName = `webvault_${selectedCaptureId}_${Date.now()}.${ext}`;
    const targetPath = await pickSaveFile({
      defaultPath: defaultName,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (!targetPath) return;
    try {
      const resultPath = await api.exportPageOffline({ captureId: selectedCaptureId, format, outputPath: targetPath });
      toast.success(`脱机文件 (${format === 'singlefile' ? 'HTML' : 'PDF'}) 已导出：${resultPath || targetPath}`);
    } catch (e: any) {
      toast.error(`导出失败: ${e?.toString()}`);
    }
  };

  const handleToggleBookmark = async () => {
    if (!selectedCaptureId) return;
    const existing = bookmarks.find((b) => b.capture_id === selectedCaptureId);
    try {
      if (existing) {
        await api.deleteBookmark(existing.id);
      } else {
        await api.createBookmark({ captureId: selectedCaptureId, note: '时光机快照书签' });
      }
      await loadBookmarks();
      toast.success(existing ? '已取消书签' : '已添加书签');
    } catch (e: any) {
      toast.error(`书签操作失败: ${e?.toString()}`);
    }
  };

  const isBookmarked = bookmarks.some((b) => b.capture_id === selectedCaptureId);

  const formatDateTime = (ts: number) => {
    return new Date(ts).toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  };

  const formatBytes = (bytes: number) => {
    if (!bytes) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  };

  // Filtered resources
  const filteredResources = useMemo(() => {
    if (!resourceFilter.trim()) return resources;
    const q = resourceFilter.toLowerCase();
    return resources.filter(
      (r) => r.url.toLowerCase().includes(q) || (r.mime_type && r.mime_type.toLowerCase().includes(q))
    );
  }, [resources, resourceFilter]);

  return (
    <div className="flex-1 h-screen flex overflow-hidden bg-neutral-950 text-neutral-200">
      {/* 1. Left Sidebar: Site Switcher & Page Directory */}
      <div className="w-72 border-r border-neutral-800 flex flex-col justify-between shrink-0 bg-neutral-900/50">
        {/* Site Header & Switcher */}
        <div className="p-3.5 border-b border-neutral-800 space-y-2.5">
          <div className="flex items-center justify-between">
            <h2 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
              <Clock className="w-4 h-4 text-indigo-400" />
              网页时光机
            </h2>
            <button
              onClick={() => setIsAddSiteOpen(true)}
              className="text-xs text-indigo-400 hover:text-indigo-300 font-medium cursor-pointer"
              title="添加新站点或 URL"
            >
              + 抓取
            </button>
          </div>

          {/* Site Selector Dropdown */}
          <div className="relative">
            <select
              value={selectedSiteId || ''}
              onChange={(e) => setSelectedSiteId(e.target.value)}
              className="w-full px-3 py-2 bg-neutral-950 border border-neutral-800 rounded-lg text-xs font-medium text-neutral-200 focus:outline-hidden focus:border-indigo-500 shadow-2xs truncate"
            >
              {sites.length === 0 ? (
                <option value="">暂无站点</option>
              ) : (
                sites.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name} ({s.capture_count || 0} 快照)
                  </option>
                ))
              )}
            </select>
          </div>

          {/* Page Search Filter */}
          <div className="relative flex items-center">
            <Search className="w-3.5 h-3.5 absolute left-3 text-neutral-400 pointer-events-none" />
            <input
              type="text"
              placeholder="搜索站内网页..."
              value={pageFilter}
              onChange={(e) => setPageFilter(e.target.value)}
              className="w-full pl-8 pr-3 py-1.5 bg-neutral-950 border border-neutral-800 rounded-lg text-xs text-neutral-200 placeholder-neutral-500 focus:outline-hidden focus:border-indigo-500"
            />
          </div>
        </div>

        {/* Page List */}
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {pages.length === 0 ? (
            <div className="p-6 text-center text-xs text-neutral-400 space-y-2">
              <div>暂无已抓取页面</div>
              <button
                onClick={() => setIsAddSiteOpen(true)}
                className="px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded text-xs font-medium transition cursor-pointer"
              >
                开始首次抓取
              </button>
            </div>
          ) : filteredPages.length === 0 ? (
            <div className="p-4 text-center text-xs text-neutral-400">
              无匹配页面
            </div>
          ) : (
            filteredPages.map((page) => {
              const isSelected = page.id === selectedPageId;
              return (
                <button
                  key={page.id}
                  onClick={() => setSelectedPageId(page.id)}
                  className={`w-full text-left p-2.5 rounded-lg text-xs transition space-y-1 cursor-pointer ${
                    isSelected
                      ? 'bg-neutral-800 text-neutral-100 shadow-2xs font-medium'
                      : 'hover:bg-neutral-800/50 text-neutral-400 border border-transparent'
                  }`}
                >
                  <div className="flex items-center justify-between gap-1">
                    <span className="truncate font-semibold text-neutral-100 text-xs">
                      {page.title || page.url}
                    </span>
                    <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-neutral-900 text-neutral-300 shrink-0">
                      {page.capture_count} 帧
                    </span>
                  </div>
                  <div className="text-[11px] text-neutral-400 truncate font-mono">
                    {page.url}
                  </div>
                </button>
              );
            })
          )}
        </div>
      </div>

      {/* 2. Main Right Workspace: Integrated TimeMachine */}
      <div className="flex-1 flex flex-col h-full overflow-hidden">
        {activePage && currentCapture ? (
          <>
            {/* Top Bar Row 1: URL & Meta & Global Actions */}
            <div className="px-4 py-2.5 border-b border-neutral-800 bg-neutral-900 flex items-center justify-between gap-4 shrink-0 shadow-2xs">
              {/* Left URL pill */}
              <div className="flex-1 max-w-2xl flex items-center bg-neutral-950 border border-neutral-800 rounded-lg px-3 py-1.5 text-xs gap-2.5 shadow-2xs">
                <div className="flex items-center gap-1 px-1.5 py-0.5 rounded bg-neutral-800 text-neutral-200 font-mono text-[11px] font-semibold shrink-0">
                  <Lock className="w-3 h-3 text-emerald-400" />
                  <span>WARC 离线沙盒</span>
                </div>

                <div className="flex-1 text-neutral-100 font-mono text-xs truncate">
                  {currentCapture.url || activePage.url}
                </div>

                <a
                  href={currentCapture.url || activePage.url}
                  target="_blank"
                  rel="noreferrer"
                  className="text-neutral-400 hover:text-neutral-200 p-0.5 transition"
                  title="在新标签页访问外网实时页面"
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                </a>
              </div>

              {/* Right Action Tools */}
              <div className="flex items-center gap-2">
                {/* One-Click Monitor Toggle */}
                <button
                  onClick={handleToggleMonitor}
                  disabled={monitoringLoading}
                  className={`flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-xs font-medium transition cursor-pointer border ${
                    isMonitored
                      ? 'bg-amber-500/15 border-amber-500/30 text-amber-300'
                      : 'bg-neutral-900 border-neutral-800 text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800'
                  }`}
                  title={isMonitored ? '该页面已被监控，点击可暂停' : '点击将此页面加入定时监控'}
                >
                  <Activity className={`w-3.5 h-3.5 ${isMonitored ? 'text-amber-400 animate-pulse' : ''}`} />
                  <span>{isMonitored ? '监控中' : '设为监控'}</span>
                </button>

                {/* Bookmark Toggle */}
                <button
                  onClick={handleToggleBookmark}
                  className={`p-1.5 rounded-lg border transition cursor-pointer ${
                    isBookmarked
                      ? 'bg-amber-500/20 border-amber-500/40 text-amber-400'
                      : 'bg-neutral-900 border-neutral-800 text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800'
                  }`}
                  title={isBookmarked ? '取消书签' : '为此版本加书签'}
                >
                  <Bookmark className={`w-3.5 h-3.5 ${isBookmarked ? 'fill-amber-400' : ''}`} />
                </button>

                {/* Offline Exports */}
                <div className="flex items-center rounded-lg border border-neutral-800 bg-neutral-900 p-0.5">
                  <button
                    onClick={() => handleExportOffline('singlefile')}
                    className="flex items-center gap-1 px-2.5 py-1 rounded text-xs font-medium text-neutral-300 hover:bg-neutral-800 hover:text-neutral-100 transition cursor-pointer"
                    title="导出为脱机单 HTML 文件"
                  >
                    <FileDown className="w-3.5 h-3.5 text-cyan-400" />
                    <span>HTML</span>
                  </button>
                  <div className="w-px h-3 bg-neutral-800" />
                  <button
                    onClick={() => handleExportOffline('pdf')}
                    className="flex items-center gap-1 px-2.5 py-1 rounded text-xs font-medium text-neutral-300 hover:bg-neutral-800 hover:text-neutral-100 transition cursor-pointer"
                    title="导出为 PDF"
                  >
                    <Printer className="w-3.5 h-3.5 text-emerald-400" />
                    <span>PDF</span>
                  </button>
                </div>
              </div>
            </div>

            {/* Top Bar Row 2: Wayback Style Time Navigation Bar */}
            <div className="px-4 py-2 border-b border-neutral-800 bg-neutral-900/90 flex flex-wrap items-center justify-between gap-3 shrink-0 text-xs">
              {/* Snapshot Scrubber & Selector */}
              <div className="flex items-center gap-2">
                <span className="text-neutral-400 font-medium">版本快照:</span>

                <div className="flex items-center bg-neutral-950 border border-neutral-800 rounded-lg p-0.5">
                  <button
                    onClick={handleSelectOlder}
                    disabled={!hasOlder}
                    className="p-1.5 rounded hover:bg-neutral-800 disabled:opacity-30 text-neutral-300 transition cursor-pointer"
                    title="查看更早一版快照"
                  >
                    <ChevronLeft className="w-4 h-4" />
                  </button>

                  <select
                    value={selectedCaptureId || ''}
                    onChange={(e) => setSelectedCaptureId(e.target.value)}
                    className="bg-transparent px-2 py-1 text-xs font-mono font-medium text-neutral-100 focus:outline-hidden cursor-pointer"
                  >
                    {captures.map((c, idx) => (
                      <option key={c.id} value={c.id} className="bg-neutral-900 text-neutral-200">
                        {formatDateTime(c.captured_at)} (第 {captures.length - idx} 版)
                      </option>
                    ))}
                  </select>

                  <button
                    onClick={handleSelectNewer}
                    disabled={!hasNewer}
                    className="p-1.5 rounded hover:bg-neutral-800 disabled:opacity-30 text-neutral-300 transition cursor-pointer"
                    title="查看更新一版快照"
                  >
                    <ChevronRight className="w-4 h-4" />
                  </button>
                </div>

                <span className="text-[11px] font-mono text-neutral-400">
                  {currentCaptureIndex !== -1 ? `第 ${captures.length - currentCaptureIndex} / ${captures.length} 帧` : ''}
                </span>
              </div>

              {/* View Mode Switcher */}
              <div className="flex items-center bg-neutral-950 p-1 rounded-lg border border-neutral-800 text-xs font-medium">
                <button
                  onClick={() => setTimeMachineMode('replay')}
                  className={`flex items-center gap-1.5 px-3 py-1 rounded-md transition cursor-pointer ${
                    timeMachineMode === 'replay'
                      ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                      : 'text-neutral-400 hover:text-neutral-200'
                  }`}
                >
                  <PlaySquare className="w-3.5 h-3.5 text-indigo-400" />
                  网页回放
                </button>

                <button
                  onClick={() => setTimeMachineMode('diff')}
                  className={`flex items-center gap-1.5 px-3 py-1 rounded-md transition cursor-pointer ${
                    timeMachineMode === 'diff'
                      ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                      : 'text-neutral-400 hover:text-neutral-200'
                  }`}
                >
                  <GitCompare className="w-3.5 h-3.5 text-amber-400" />
                  版本比对
                </button>

                <button
                  onClick={() => setTimeMachineMode('resources')}
                  className={`flex items-center gap-1.5 px-3 py-1 rounded-md transition cursor-pointer ${
                    timeMachineMode === 'resources'
                      ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                      : 'text-neutral-400 hover:text-neutral-200'
                  }`}
                >
                  <Layers className="w-3.5 h-3.5 text-cyan-400" />
                  资源清单 ({resources.length})
                </button>
              </div>
            </div>

            {/* Main Content Area */}
            <div className="flex-1 overflow-hidden relative bg-neutral-950">
              {/* Mode A: Replay Iframe */}
              {timeMachineMode === 'replay' && (
                <div className="w-full h-full relative flex flex-col">
                  {iframeLoading ? (
                    <div className="absolute inset-0 bg-neutral-950/80 backdrop-blur-xs flex items-center justify-center z-10 text-xs text-neutral-400 gap-2">
                      <RotateCw className="w-4 h-4 animate-spin text-indigo-400" />
                      正在加载 WARC 离线沙盒...
                    </div>
                  ) : null}

                  {replayUrl ? (
                    <iframe
                      id="replay-frame"
                      src={replayUrl}
                      title="Web Archive Replay"
                      className="w-full flex-1 border-0 bg-white"
                      sandbox="allow-scripts allow-same-origin allow-forms"
                      onContextMenu={(e) => e.preventDefault()}
                    />
                  ) : (
                    <div className="flex-1 flex items-center justify-center text-xs text-neutral-400">
                      未能获取回放地址
                    </div>
                  )}
                </div>
              )}

              {/* Mode B: Integrated Diff Inspector */}
              {timeMachineMode === 'diff' && (
                <div className="w-full h-full overflow-y-auto p-5 space-y-4">
                  {captures.length < 2 ? (
                    <div className="p-10 text-center bg-neutral-900/40 rounded-xl border border-neutral-800 text-neutral-400 text-xs space-y-2">
                      <GitCompare className="w-8 h-8 text-neutral-600 mx-auto" />
                      <div>该网页当前仅有 1 次快照，无法执行双版本比对</div>
                      <p className="text-neutral-400">当再次抓取产生新快照后，可在此查看文本、DOM、视觉与资源差异。</p>
                    </div>
                  ) : (
                    <>
                      {/* Diff Selection Bar */}
                      <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-4 flex flex-wrap items-center justify-between gap-4 shadow-2xs">
                        <div className="flex items-center gap-4 text-xs">
                          <span className="font-semibold text-neutral-200">对照基准:</span>
                          <select
                            value={compareTargetId}
                            onChange={(e) => setCompareTargetId(e.target.value)}
                            className="px-3 py-1.5 bg-neutral-950 border border-neutral-800 rounded-lg text-xs font-mono text-neutral-200 focus:outline-hidden"
                          >
                            {captures
                              .filter((c) => c.id !== selectedCaptureId)
                              .map((c) => (
                                <option key={c.id} value={c.id}>
                                  基准版本: {formatDateTime(c.captured_at)}
                                </option>
                              ))}
                          </select>
                          <span className="text-neutral-400">vs</span>
                          <span className="font-mono text-indigo-300 font-semibold">
                            当前版本: {formatDateTime(currentCapture.captured_at)}
                          </span>
                        </div>

                        {/* Diff Sub Tabs */}
                        <div className="flex items-center bg-neutral-950 p-1 rounded-lg border border-neutral-800 text-xs">
                          <button
                            onClick={() => setDiffSubTab('text')}
                            className={`px-3 py-1 rounded-md transition font-medium cursor-pointer ${
                              diffSubTab === 'text'
                                ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                                : 'text-neutral-400 hover:text-neutral-200'
                            }`}
                          >
                            文本差异
                          </button>
                          <button
                            onClick={() => setDiffSubTab('resources')}
                            className={`px-3 py-1 rounded-md transition font-medium cursor-pointer ${
                              diffSubTab === 'resources'
                                ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                                : 'text-neutral-400 hover:text-neutral-200'
                            }`}
                          >
                            资源差异
                          </button>
                          <button
                            onClick={() => setDiffSubTab('visual')}
                            className={`px-3 py-1 rounded-md transition font-medium cursor-pointer ${
                              diffSubTab === 'visual'
                                ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                                : 'text-neutral-400 hover:text-neutral-200'
                            }`}
                          >
                            视觉对比
                          </button>
                          <button
                            onClick={() => setDiffSubTab('dom')}
                            className={`px-3 py-1 rounded-md transition font-medium cursor-pointer ${
                              diffSubTab === 'dom'
                                ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                                : 'text-neutral-400 hover:text-neutral-200'
                            }`}
                          >
                            DOM 结构
                          </button>
                        </div>
                      </div>

                      {/* Diff Result Body */}
                      {diffLoading ? (
                        <div className="text-center py-12 text-neutral-400 text-xs flex items-center justify-center gap-2">
                          <RotateCw className="w-4 h-4 animate-spin text-indigo-400" />
                          正在计算版本差异分析...
                        </div>
                      ) : diffResult ? (
                        <div className="space-y-4">
                          {/* Metrics summary */}
                          <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-4 flex flex-wrap items-center justify-between gap-4 text-xs font-mono">
                            <div className="flex items-center gap-6">
                              <div>
                                <span className="text-neutral-400 block text-[11px] font-sans">变动评分</span>
                                <span className="text-lg font-bold text-neutral-100">
                                  {diffResult.change_score}{' '}
                                  <span className="text-xs text-neutral-400 font-normal">/ 100</span>
                                </span>
                              </div>
                              <div className="flex items-center gap-3">
                                <span className="flex items-center gap-1 text-emerald-400">
                                  <Plus className="w-3.5 h-3.5" />
                                  {diffResult.text_diff.added_lines} 行新增
                                </span>
                                <span className="flex items-center gap-1 text-rose-400">
                                  <Minus className="w-3.5 h-3.5" />
                                  {diffResult.text_diff.removed_lines} 行删除
                                </span>
                              </div>
                            </div>

                            <div className="flex items-center gap-3 text-neutral-400">
                              <span>+{diffResult.resource_diff.added.length} 新增资源</span>
                              <span>·</span>
                              <span>-{diffResult.resource_diff.removed.length} 移除资源</span>
                              <span>·</span>
                              <span>{diffResult.resource_diff.modified.length} 变更资源</span>
                            </div>
                          </div>

                          {/* SubTab Views */}
                          {diffSubTab === 'text' && (
                            <div className="bg-neutral-900/70 border border-neutral-800 rounded-xl overflow-hidden font-mono text-xs shadow-2xs">
                              <div className="p-2.5 bg-neutral-900 border-b border-neutral-800 text-neutral-300 font-sans font-semibold text-xs">
                                网页正文文本差异对比
                              </div>
                              <div className="max-h-[500px] overflow-y-auto divide-y divide-neutral-900/60">
                                {diffResult.text_diff.lines.map((line, idx) => {
                                  const isAdd = line.type === 'add';
                                  const isRemove = line.type === 'remove';
                                  return (
                                    <div
                                      key={idx}
                                      className={`flex items-start px-3 py-1 gap-2 leading-relaxed ${
                                        isAdd
                                          ? 'bg-emerald-950/30 text-emerald-300'
                                          : isRemove
                                          ? 'bg-rose-950/30 text-rose-300 line-through opacity-80'
                                          : 'text-neutral-300 hover:bg-neutral-900/30'
                                      }`}
                                    >
                                      <span className="w-4 text-neutral-500 select-none shrink-0 text-right">
                                        {isAdd ? '+' : isRemove ? '-' : ' '}
                                      </span>
                                      <span className="flex-1 whitespace-pre-wrap break-all">{line.content}</span>
                                    </div>
                                  );
                                })}
                              </div>
                            </div>
                          )}

                          {diffSubTab === 'resources' && (
                            <div className="bg-neutral-900/70 border border-neutral-800 rounded-xl overflow-hidden text-xs shadow-2xs">
                              <div className="p-2.5 bg-neutral-900 border-b border-neutral-800 font-semibold text-neutral-200">
                                资源变动清单
                              </div>
                              <div className="p-4 space-y-4">
                                {diffResult.resource_diff.added.length > 0 && (
                                  <div className="space-y-2">
                                    <div className="text-emerald-400 font-semibold flex items-center gap-1">
                                      <Plus className="w-3.5 h-3.5" />
                                      新增资源 ({diffResult.resource_diff.added.length})
                                    </div>
                                    <div className="divide-y divide-neutral-800 bg-neutral-950 rounded-lg border border-neutral-800 font-mono text-xs">
                                      {diffResult.resource_diff.added.map((r) => (
                                        <div key={r.id} className="p-2 flex items-center justify-between">
                                          <span className="text-neutral-200 truncate max-w-xl">{r.url}</span>
                                          <span className="text-neutral-400">{r.mime_type}</span>
                                        </div>
                                      ))}
                                    </div>
                                  </div>
                                )}

                                {diffResult.resource_diff.removed.length > 0 && (
                                  <div className="space-y-2">
                                    <div className="text-rose-400 font-semibold flex items-center gap-1">
                                      <Minus className="w-3.5 h-3.5" />
                                      移除资源 ({diffResult.resource_diff.removed.length})
                                    </div>
                                    <div className="divide-y divide-neutral-800 bg-neutral-950 rounded-lg border border-neutral-800 font-mono text-xs">
                                      {diffResult.resource_diff.removed.map((r) => (
                                        <div key={r.id} className="p-2 flex items-center justify-between">
                                          <span className="text-neutral-400 truncate max-w-xl line-through">{r.url}</span>
                                          <span className="text-neutral-400">{r.mime_type}</span>
                                        </div>
                                      ))}
                                    </div>
                                  </div>
                                )}
                              </div>
                            </div>
                          )}

                          {diffSubTab === 'visual' && (
                            <div className="bg-neutral-900/70 border border-neutral-800 rounded-xl overflow-hidden text-xs shadow-2xs">
                              <div className="p-2.5 bg-neutral-900 border-b border-neutral-800 font-semibold text-neutral-200 flex items-center justify-between">
                                <span>像素级视觉差异对比</span>
                                {diffResult.visual_diff && (
                                  <span className="text-xs font-mono text-amber-400">
                                    差异比例: {diffResult.visual_diff.diff_ratio}% ({diffResult.visual_diff.changed_pixels.toLocaleString()} px)
                                  </span>
                                )}
                              </div>
                              <div className="p-4 flex justify-center bg-neutral-950/60">
                                {diffResult.visual_diff?.diff_image_base64 ? (
                                  <img
                                    src={`data:image/png;base64,${diffResult.visual_diff.diff_image_base64}`}
                                    alt="Visual Diff"
                                    className="max-w-full rounded border border-neutral-800 shadow-md object-contain"
                                  />
                                ) : (
                                  <div className="p-8 text-center text-neutral-400 text-xs">
                                    快照未记录视觉截图或尺寸不匹配，暂无法生成像素对比图
                                  </div>
                                )}
                              </div>
                            </div>
                          )}

                          {diffSubTab === 'dom' && (
                            <div className="bg-neutral-900/70 border border-neutral-800 rounded-xl overflow-hidden text-xs shadow-2xs">
                              <div className="p-2.5 bg-neutral-900 border-b border-neutral-800 font-semibold text-neutral-200 flex items-center justify-between">
                                <span>DOM 节点树结构变动</span>
                                {diffResult.dom_diff && (
                                  <span className="text-xs font-mono text-cyan-400">
                                    结构相似度: {diffResult.dom_diff.structure_score}%
                                  </span>
                                )}
                              </div>
                              <div className="p-4 space-y-4">
                                {diffResult.dom_diff ? (
                                  <>
                                    <div className="grid grid-cols-3 gap-3">
                                      <div className="bg-neutral-950 border border-neutral-800 rounded-lg p-2.5">
                                        <span className="text-neutral-400 block text-[11px]">基准版本节点数</span>
                                        <span className="text-sm font-bold font-mono text-neutral-200">
                                          {diffResult.dom_diff.total_tags_old.toLocaleString()}
                                        </span>
                                      </div>
                                      <div className="bg-neutral-950 border border-neutral-800 rounded-lg p-2.5">
                                        <span className="text-neutral-400 block text-[11px]">当前版本节点数</span>
                                        <span className="text-sm font-bold font-mono text-neutral-200">
                                          {diffResult.dom_diff.total_tags_new.toLocaleString()}
                                        </span>
                                      </div>
                                      <div className="bg-neutral-950 border border-neutral-800 rounded-lg p-2.5">
                                        <span className="text-neutral-400 block text-[11px]">变动标签种类</span>
                                        <span className="text-sm font-bold font-mono text-cyan-400">
                                          {diffResult.dom_diff.tag_changes.length} 个
                                        </span>
                                      </div>
                                    </div>

                                    {diffResult.dom_diff.tag_changes.length > 0 && (
                                      <div className="space-y-2">
                                        <div className="text-xs font-semibold text-neutral-300">节点标签数量增减详情</div>
                                        <div className="divide-y divide-neutral-800 bg-neutral-950 rounded-lg border border-neutral-800 font-mono text-xs overflow-hidden">
                                          <div className="grid grid-cols-4 p-2 bg-neutral-900/60 font-sans text-neutral-400 font-medium">
                                            <span>标签名称</span>
                                            <span className="text-right">基准数量</span>
                                            <span className="text-right">当前数量</span>
                                            <span className="text-right">变动差值</span>
                                          </div>
                                          {diffResult.dom_diff.tag_changes.map((tc) => (
                                            <div key={tc.tag} className="grid grid-cols-4 p-2 items-center hover:bg-neutral-900/30">
                                              <span className="text-neutral-200 font-semibold">&lt;{tc.tag}&gt;</span>
                                              <span className="text-neutral-400 text-right">{tc.old_count}</span>
                                              <span className="text-neutral-300 text-right">{tc.new_count}</span>
                                              <span
                                                className={`text-right font-bold ${
                                                  tc.diff > 0 ? 'text-emerald-400' : 'text-rose-400'
                                                }`}
                                              >
                                                {tc.diff > 0 ? `+${tc.diff}` : tc.diff}
                                              </span>
                                            </div>
                                          ))}
                                        </div>
                                      </div>
                                    )}
                                  </>
                                ) : (
                                  <div className="p-6 text-center text-neutral-400">
                                    快照格式不支持或未能提取 DOM 结构
                                  </div>
                                )}
                              </div>
                            </div>
                          )}
                        </div>
                      ) : null}
                    </>
                  )}
                </div>
              )}

              {/* Mode C: Resources Explorer */}
              {timeMachineMode === 'resources' && (
                <div className="w-full h-full overflow-y-auto p-5 space-y-4">
                  <div className="flex items-center justify-between gap-4 pb-2 border-b border-neutral-800">
                    <div className="relative flex-1 max-w-md">
                      <Search className="w-3.5 h-3.5 absolute left-3 top-2.5 text-neutral-400 pointer-events-none" />
                      <input
                        type="text"
                        placeholder="过滤资源 URL 或 MIME 类型..."
                        value={resourceFilter}
                        onChange={(e) => setResourceFilter(e.target.value)}
                        className="w-full pl-8 pr-3 py-1.5 bg-neutral-900 border border-neutral-800 rounded-lg text-xs text-neutral-200 placeholder-neutral-500 focus:outline-hidden focus:border-indigo-500"
                      />
                    </div>
                    <span className="text-xs font-mono text-neutral-400">
                      共 {filteredResources.length} / {resources.length} 项资源
                    </span>
                  </div>

                  <div className="bg-neutral-900/70 border border-neutral-800 rounded-xl overflow-hidden font-mono text-xs shadow-2xs">
                    <div className="grid grid-cols-12 p-2.5 bg-neutral-900 border-b border-neutral-800 font-sans text-neutral-400 font-semibold">
                      <div className="col-span-6 truncate">URL</div>
                      <div className="col-span-3">类型 (MIME)</div>
                      <div className="col-span-1 text-center">状态</div>
                      <div className="col-span-2 text-right">大小</div>
                    </div>
                    <div className="max-h-[600px] overflow-y-auto divide-y divide-neutral-900/60">
                      {filteredResources.map((r) => (
                        <div key={r.id} className="grid grid-cols-12 p-2 items-center hover:bg-neutral-800/40 transition">
                          <div className="col-span-6 truncate text-neutral-200 pr-2" title={r.url}>
                            {r.url}
                          </div>
                          <div className="col-span-3 text-neutral-400 truncate pr-2">
                            {r.mime_type || 'unknown'}
                          </div>
                          <div className="col-span-1 text-center font-bold text-emerald-400">
                            {r.status_code}
                          </div>
                          <div className="col-span-2 text-right text-neutral-300">
                            {formatBytes(r.size)}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
              )}
            </div>
          </>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center p-8 text-center space-y-3">
            <Clock className="w-12 h-12 text-neutral-700 mx-auto" />
            <div className="text-base font-semibold text-neutral-300">未选择网页快照</div>
            <p className="text-xs text-neutral-400 max-w-sm">
              请在左侧站点与页面列表中选择一个网页，或点击下方按钮开始抓取新页面。
            </p>
            <button
              onClick={() => setIsAddSiteOpen(true)}
              className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition cursor-pointer"
            >
              + 抓取新站点或网页
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
