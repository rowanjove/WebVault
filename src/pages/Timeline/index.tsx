import React, { useEffect, useState } from 'react';
import {
  Clock,
  PlaySquare,
  GitCompare,
  CheckCircle2,
  AlertTriangle,
  Layers,
  ChevronRight,
  ExternalLink,
  Calendar,
  Sparkles,
  FileDown,
  Printer,
  Bookmark,
  Tag as TagIcon,
  Plus,
  X,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api } from '../../services/tauri';
import { toast } from '../../components/ui/Toast';
import type { PageItem, CaptureItem, BookmarkItem, TagItem } from '../../types';

export const Timeline: React.FC = () => {
  const {
    selectedSiteId,
    selectedPageId,
    setSelectedPageId,
    setSelectedCaptureId,
    setCurrentTab,
    setDiffPair,
  } = useAppStore();

  const [pages, setPages] = useState<PageItem[]>([]);
  const [captures, setCaptures] = useState<CaptureItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [bookmarks, setBookmarks] = useState<BookmarkItem[]>([]);
  const [pageTags, setPageTags] = useState<TagItem[]>([]);
  const [allTags, setAllTags] = useState<TagItem[]>([]);
  const [isTagDropdownOpen, setIsTagDropdownOpen] = useState(false);
  const [newTagName, setNewTagName] = useState('');

  const loadBookmarks = async () => {
    try {
      const bms = await api.listBookmarks();
      setBookmarks(bms);
    } catch {}
  };

  const loadTags = async (pageId?: string) => {
    try {
      const tags = await api.listTags();
      setAllTags(tags);
      if (pageId) {
        const pt = await api.getPageTags(pageId);
        setPageTags(pt);
      }
    } catch {}
  };

  const handleToggleBookmark = async (captureId: string) => {
    const existing = bookmarks.find((b) => b.capture_id === captureId);
    try {
      if (existing) {
        await api.deleteBookmark(existing.id);
      } else {
        await api.createBookmark({ captureId, note: '快照书签' });
      }
      await loadBookmarks();
      toast.success(existing ? '已取消书签' : '已添加书签');
    } catch (e: any) {
      toast.error(`书签操作失败: ${e?.toString()}`);
    }
  };

  const handleAddTagToPage = async (tagId: string) => {
    if (!selectedPageId) return;
    try {
      await api.addPageTag({ pageId: selectedPageId, tagId });
      await loadTags(selectedPageId);
      toast.success('标签已添加');
    } catch (e: any) {
      toast.error(`添加标签失败: ${e?.toString()}`);
    }
  };

  const handleRemoveTagFromPage = async (tagId: string) => {
    if (!selectedPageId) return;
    try {
      await api.removePageTag({ pageId: selectedPageId, tagId });
      await loadTags(selectedPageId);
      toast.success('标签已移除');
    } catch (e: any) {
      toast.error(`移除标签失败: ${e?.toString()}`);
    }
  };

  const handleCreateAndAttachTag = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedPageId || !newTagName.trim()) return;
    try {
      const tag = await api.createTag({ name: newTagName.trim(), color: '#6366f1' });
      await api.addPageTag({ pageId: selectedPageId, tagId: tag.id });
      setNewTagName('');
      await loadTags(selectedPageId);
      setIsTagDropdownOpen(false);
      toast.success('标签已创建');
    } catch (e: any) {
      toast.error(`创建标签失败: ${e?.toString()}`);
    }
  };

  const handleExportOffline = async (captureId: string, format: 'singlefile' | 'pdf') => {
    const defaultName = `webvault_${captureId}_${Date.now()}.${format === 'singlefile' ? 'html' : 'pdf'}`;
    const targetPath = prompt(`请输入导出文件保存路径或文件名：`, defaultName);
    if (!targetPath) return;
    try {
      const resultPath = await api.exportPageOffline({ captureId, format, outputPath: targetPath });
      toast.success(`脱机文件 (${format === 'singlefile' ? 'HTML' : 'PDF'}) 已导出：${resultPath || targetPath}`);
    } catch (e: any) {
      toast.error(`导出失败: ${e?.toString()}`);
    }
  };

  useEffect(() => {
    loadBookmarks();
    if (selectedSiteId) {
      api.listPages(selectedSiteId).then((p) => {
        setPages(p);
        if (!selectedPageId && p.length > 0) {
          setSelectedPageId(p[0].id);
        }
      });
    }
  }, [selectedSiteId]);

  useEffect(() => {
    if (selectedPageId) {
      setLoading(true);
      loadTags(selectedPageId);
      api
        .listCaptures(selectedPageId)
        .then(setCaptures)
        .finally(() => setLoading(false));
    } else {
      setCaptures([]);
      setPageTags([]);
    }
  }, [selectedPageId]);

  const activePage = pages.find((p) => p.id === selectedPageId);

  const formatDate = (ts: number) => {
    const d = new Date(ts);
    return {
      year: d.getFullYear(),
      dateStr: d.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' }),
      timeStr: d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
    };
  };

  return (
    <div className="flex-1 h-screen flex overflow-hidden bg-neutral-950 text-neutral-200">
      {/* Left Pages Selector */}
      <div className="w-64 border-r border-neutral-800 flex flex-col justify-between shrink-0 bg-neutral-900/40">
        <div className="p-3.5 border-b border-neutral-800">
          <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
            <Clock className="w-4 h-4 text-indigo-400" />
            时间线
          </h1>
        </div>

        <div className="flex-1 overflow-y-auto p-2.5 space-y-1.5">
          {pages.length === 0 ? (
            <div className="text-center py-8 text-neutral-400 text-xs">
              暂无已归档页面
            </div>
          ) : (
            pages.map((p) => {
              const isSelected = p.id === selectedPageId;
              return (
                <button
                  key={p.id}
                  onClick={() => setSelectedPageId(p.id)}
                  className={`w-full text-left p-2.5 rounded-lg text-xs transition space-y-1 cursor-pointer ${
                    isSelected
                      ? 'bg-neutral-800 text-neutral-100 shadow-2xs'
                      : 'hover:bg-neutral-800/60 border border-transparent text-neutral-400'
                  }`}
                >
                  <div className="font-semibold text-neutral-100 text-sm truncate">{p.title || p.url}</div>
                  <div className="text-xs text-neutral-400 truncate font-mono">{p.url}</div>
                  <div className="text-xs text-neutral-400 flex items-center justify-between pt-0.5 font-mono">
                    <span className="font-medium text-neutral-300">{p.capture_count} 个快照</span>
                  </div>
                </button>
              );
            })
          )}
        </div>
      </div>

      {/* Main Timeline View */}
      <div className="flex-1 flex flex-col h-full overflow-hidden">
        {activePage ? (
          <>
            {/* Header */}
            <div className="p-4 border-b border-neutral-800 bg-neutral-900/30 flex items-center justify-between">
              <div className="space-y-1.5 max-w-xl">
                <div className="flex items-center gap-2">
                  <h1 className="text-base font-bold text-neutral-100 truncate">
                    {activePage.title || activePage.url}
                  </h1>
                </div>
                <div className="text-xs text-neutral-400 font-mono truncate">
                  {activePage.url}
                </div>

                {/* Tags row */}
                <div className="flex items-center gap-1.5 flex-wrap pt-1">
                  <TagIcon className="w-3 h-3 text-neutral-500" />
                  {pageTags.map((tag) => (
                    <span
                      key={tag.id}
                      className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-mono bg-neutral-800 border border-neutral-700 text-neutral-300"
                    >
                      <span>{tag.name}</span>
                      <button
                        onClick={() => handleRemoveTagFromPage(tag.id)}
                        className="text-neutral-500 hover:text-rose-400"
                        title="移除标签"
                      >
                        <X className="w-3 h-3" />
                      </button>
                    </span>
                  ))}

                  <div className="relative">
                    <button
                      onClick={() => setIsTagDropdownOpen(!isTagDropdownOpen)}
                      className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-mono bg-neutral-900 hover:bg-neutral-800 border border-dashed border-neutral-700 text-neutral-400 hover:text-neutral-200 transition"
                    >
                      <Plus className="w-3 h-3" />
                      打标签
                    </button>

                    {isTagDropdownOpen && (
                      <div className="absolute left-0 top-7 z-20 w-52 bg-neutral-900 border border-neutral-800 rounded-lg shadow-lg p-2.5 space-y-2 text-xs">
                        <form onSubmit={handleCreateAndAttachTag} className="flex items-center gap-1.5">
                          <input
                            type="text"
                            placeholder="新标签名..."
                            value={newTagName}
                            onChange={(e) => setNewTagName(e.target.value)}
                            className="flex-1 px-2 py-1 bg-neutral-950 border border-neutral-800 rounded text-xs text-neutral-200"
                          />
                          <button
                            type="submit"
                            className="px-2.5 py-1 bg-indigo-600 hover:bg-indigo-500 text-white rounded text-xs font-medium"
                          >
                            添加
                          </button>
                        </form>

                        {allTags.length > 0 && (
                          <div className="max-h-36 overflow-y-auto space-y-1 pt-1.5 border-t border-neutral-800">
                            <div className="text-xs text-neutral-500">选择已有标签:</div>
                            {allTags.map((t) => {
                              const alreadyAttached = pageTags.some((pt) => pt.id === t.id);
                              return (
                                <button
                                  key={t.id}
                                  type="button"
                                  disabled={alreadyAttached}
                                  onClick={() => {
                                    handleAddTagToPage(t.id);
                                    setIsTagDropdownOpen(false);
                                  }}
                                  className="w-full text-left px-2 py-1 rounded hover:bg-neutral-800 text-xs text-neutral-300 disabled:opacity-40 truncate"
                                >
                                  {t.name} {alreadyAttached ? '(已添加)' : ''}
                                </button>
                              );
                            })}
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              </div>

              <div className="text-right">
                <span className="text-xs font-mono px-2 py-0.5 bg-neutral-800 text-neutral-300 rounded border border-neutral-700">
                  {captures.length} 个快照
                </span>
              </div>
            </div>

            {/* Timeline Stream */}
            <div className="flex-1 overflow-y-auto p-5">
              {loading ? (
                <div className="text-center py-12 text-neutral-500 text-xs">
                  正在加载快照...
                </div>
              ) : captures.length === 0 ? (
                <div className="text-center py-12 text-neutral-500 text-xs">
                  暂无快照记录
                </div>
              ) : (
                <div className="relative border-l border-neutral-800 ml-3 space-y-4 pb-12">
                  {captures.map((cap, index) => {
                    const { year, dateStr, timeStr } = formatDate(cap.captured_at);
                    const prevCap = captures[index + 1];

                    return (
                      <div key={cap.id} className="relative pl-5">
                        {/* Timeline dot */}
                        <div className="absolute -left-[5px] top-2.5 w-2 h-2 rounded-full bg-indigo-500" />

                        {/* Capture Card (放大 0.5 倍) */}
                        <div className="bg-neutral-900 border border-neutral-800 hover:border-neutral-700 rounded-xl p-4 transition space-y-3 max-w-3xl shadow-2xs">
                          <div className="flex items-center justify-between">
                            <div className="flex items-center gap-2.5">
                              <Calendar className="w-4 h-4 text-neutral-400" />
                              <span className="text-sm font-semibold text-neutral-100">
                                {year}年 {dateStr}
                              </span>
                              <span className="text-xs text-neutral-400 font-mono">{timeStr}</span>
                            </div>

                            <div className="flex items-center gap-2 font-mono text-xs">
                              <span
                                className={`px-2 py-0.5 rounded-md font-semibold ${
                                  cap.status_code === 200
                                    ? 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-400'
                                    : 'bg-amber-500/20 text-amber-500'
                                }`}
                              >
                                HTTP {cap.status_code}
                              </span>

                              <span className="px-2 py-0.5 rounded-md bg-neutral-800 text-neutral-300 font-medium">
                                完整度 {cap.capture_score.toFixed(0)}%
                              </span>
                            </div>
                          </div>

                          <div className="flex items-center justify-between text-xs text-neutral-400 pt-1">
                            <div className="flex items-center gap-3">
                              <span className="flex items-center gap-1.5">
                                <Layers className="w-4 h-4 text-neutral-400" />
                                <strong className="text-neutral-200">{cap.resource_count}</strong> 项资源
                              </span>
                              {cap.missing_resource_count > 0 && (
                                <span className="text-amber-500 font-medium">
                                  ({cap.missing_resource_count} 未加载)
                                </span>
                              )}
                            </div>

                            <div className="flex items-center gap-1.5">
                              <button
                                onClick={() => handleToggleBookmark(cap.id)}
                                className={`px-2.5 py-1 text-xs rounded flex items-center gap-1 transition font-mono ${
                                  bookmarks.some((b) => b.capture_id === cap.id)
                                    ? 'bg-amber-500/20 text-amber-300 border border-amber-500/40'
                                    : 'bg-neutral-800 hover:bg-neutral-700 text-neutral-400'
                                }`}
                                title={
                                  bookmarks.some((b) => b.capture_id === cap.id)
                                    ? '取消收藏'
                                    : '添加书签收藏'
                                }
                              >
                                <Bookmark
                                  className={`w-3 h-3 ${
                                    bookmarks.some((b) => b.capture_id === cap.id)
                                      ? 'fill-amber-400 text-amber-400'
                                      : ''
                                  }`}
                                />
                                {bookmarks.some((b) => b.capture_id === cap.id) ? '已收藏' : '收藏'}
                              </button>

                              {prevCap && (
                                <button
                                  onClick={() => setDiffPair(prevCap.id, cap.id)}
                                  className="px-2.5 py-1 text-xs bg-neutral-800 hover:bg-neutral-700 text-neutral-300 rounded flex items-center gap-1 transition"
                                >
                                  <GitCompare className="w-3 h-3 text-amber-400" />
                                  比对前一版
                                </button>
                              )}

                              <button
                                onClick={() => handleExportOffline(cap.id, 'singlefile')}
                                className="px-2.5 py-1 text-xs bg-neutral-800 hover:bg-neutral-700 text-neutral-300 rounded flex items-center gap-1 transition font-mono"
                                title="导出为独立单文件 HTML"
                              >
                                <FileDown className="w-3 h-3 text-cyan-400" />
                                HTML
                              </button>

                              <button
                                onClick={() => handleExportOffline(cap.id, 'pdf')}
                                className="px-2.5 py-1 text-xs bg-neutral-800 hover:bg-neutral-700 text-neutral-300 rounded flex items-center gap-1 transition font-mono"
                                title="导出为 PDF"
                              >
                                <Printer className="w-3 h-3 text-emerald-400" />
                                PDF
                              </button>

                              <button
                                onClick={() => {
                                  setSelectedCaptureId(cap.id);
                                  setCurrentTab('replay');
                                }}
                                className="px-3 py-1 text-xs bg-indigo-600 hover:bg-indigo-500 text-white rounded font-medium flex items-center gap-1 transition shadow-xs"
                              >
                                <PlaySquare className="w-3 h-3 fill-white" />
                                回放
                              </button>
                            </div>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center text-neutral-500 text-xs">
            请从左侧选择页面查看时间线
          </div>
        )}
      </div>
    </div>
  );
};
