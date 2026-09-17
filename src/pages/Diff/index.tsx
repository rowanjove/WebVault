import React, { useEffect, useState } from 'react';
import {
  GitCompare,
  Plus,
  Minus,
  Layers,
  FileText,
  AlertCircle,
  Clock,
  ArrowRight,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api } from '../../services/tauri';
import type { DiffResult } from '../../types';

export const Diff: React.FC = () => {
  const { diffOldCaptureId, diffNewCaptureId } = useAppStore();

  const [diffResult, setDiffResult] = useState<DiffResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [activeTab, setActiveTab] = useState<'text' | 'resources' | 'visual' | 'dom'>('text');

  useEffect(() => {
    if (diffOldCaptureId && diffNewCaptureId) {
      setLoading(true);
      api
        .compareCaptures({ oldCaptureId: diffOldCaptureId, newCaptureId: diffNewCaptureId })
        .then(setDiffResult)
        .catch(console.error)
        .finally(() => setLoading(false));
    }
  }, [diffOldCaptureId, diffNewCaptureId]);

  return (
    <div className="flex-1 h-screen overflow-y-auto bg-neutral-950 text-neutral-200 p-5 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between pb-3 border-b border-neutral-800">
        <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
          <GitCompare className="w-4 h-4 text-indigo-400" />
          版本比对
        </h1>
      </div>

      {!diffOldCaptureId || !diffNewCaptureId ? (
        <div className="p-10 text-center bg-neutral-900/40 rounded-lg border border-neutral-800 text-neutral-500 text-xs space-y-1.5">
          <GitCompare className="w-7 h-7 text-neutral-600 mx-auto" />
          <div>未选择比对快照</div>
          <div className="text-xs text-neutral-500">
            在时间线或监控列表中点击【比对前一版】以查看两个版本的变动差异。
          </div>
        </div>
      ) : loading ? (
        <div className="text-center py-12 text-neutral-500 text-xs">
          正在计算差异...
        </div>
      ) : diffResult ? (
        <div className="space-y-3.5">
          {/* Summary Metric Header (字号与控件放大 0.5 倍) */}
          <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-4 flex flex-wrap items-center justify-between gap-4 shadow-2xs">
            <div className="flex items-center gap-6 text-sm">
              <div>
                <span className="text-neutral-400 block text-xs font-medium">变动评分</span>
                <span className="text-xl font-bold font-mono text-neutral-100">
                  {diffResult.change_score} <span className="text-xs text-neutral-400 font-normal">/ 100</span>
                </span>
              </div>

              <div className="flex items-center gap-3.5 font-mono text-xs md:text-sm font-medium">
                <span className="flex items-center gap-1 text-emerald-500">
                  <Plus className="w-4 h-4" />
                  {diffResult.text_diff.added_lines} 行新增
                </span>
                <span className="flex items-center gap-1 text-rose-500">
                  <Minus className="w-4 h-4" />
                  {diffResult.text_diff.removed_lines} 行删除
                </span>
              </div>

              <div className="flex items-center gap-3 text-neutral-400 font-mono text-xs md:text-sm font-medium">
                <span>+{diffResult.resource_diff.added.length} 新增资源</span>
                <span>·</span>
                <span>-{diffResult.resource_diff.removed.length} 移除资源</span>
                <span>·</span>
                <span>{diffResult.resource_diff.modified.length} 变更资源</span>
              </div>
            </div>

            {/* Sub Tabs */}
            <div className="flex items-center bg-neutral-950 p-1 rounded-lg border border-neutral-800 text-xs md:text-sm">
              <button
                onClick={() => setActiveTab('text')}
                className={`px-3 py-1.5 rounded-md transition font-medium cursor-pointer ${
                  activeTab === 'text'
                    ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                    : 'text-neutral-400 hover:text-neutral-200'
                }`}
              >
                文本差异
              </button>
              <button
                onClick={() => setActiveTab('resources')}
                className={`px-3 py-1.5 rounded-md transition font-medium cursor-pointer ${
                  activeTab === 'resources'
                    ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                    : 'text-neutral-400 hover:text-neutral-200'
                }`}
              >
                资源差异
              </button>
              <button
                onClick={() => setActiveTab('visual')}
                className={`px-3 py-1.5 rounded-md transition font-medium cursor-pointer ${
                  activeTab === 'visual'
                    ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                    : 'text-neutral-400 hover:text-neutral-200'
                }`}
              >
                视觉对比
              </button>
              <button
                onClick={() => setActiveTab('dom')}
                className={`px-3 py-1.5 rounded-md transition font-medium cursor-pointer ${
                  activeTab === 'dom'
                    ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                    : 'text-neutral-400 hover:text-neutral-200'
                }`}
              >
                DOM 结构
              </button>
            </div>
          </div>

          {/* Diff View Body */}
          {activeTab === 'text' ? (
            <div className="bg-neutral-900/60 border border-neutral-800 rounded-lg overflow-hidden font-mono text-xs">
              <div className="p-2.5 bg-neutral-900/80 border-b border-neutral-800 text-neutral-400 font-sans font-semibold text-xs">
                文本差异清单
              </div>
              <div className="max-h-[600px] overflow-y-auto divide-y divide-neutral-900/60">
                {diffResult.text_diff.lines.map((line, idx) => {
                  const isAdd = line.type === 'add';
                  const isRemove = line.type === 'remove';
                  return (
                    <div
                      key={idx}
                      className={`flex items-start px-3 py-0.5 gap-2 leading-relaxed ${
                        isAdd
                          ? 'bg-emerald-950/30 text-emerald-300'
                          : isRemove
                          ? 'bg-rose-950/30 text-rose-300 line-through opacity-80'
                          : 'text-neutral-300 hover:bg-neutral-900/30'
                      }`}
                    >
                      <span className="w-4 text-neutral-600 select-none shrink-0 text-right">
                        {isAdd ? '+' : isRemove ? '-' : ' '}
                      </span>
                      <span className="flex-1 whitespace-pre-wrap break-all">{line.content}</span>
                    </div>
                  );
                })}
              </div>
            </div>
          ) : activeTab === 'resources' ? (
            <div className="bg-neutral-900/60 border border-neutral-800 rounded-lg overflow-hidden text-xs">
              <div className="p-2.5 bg-neutral-900/80 border-b border-neutral-800 font-semibold text-neutral-200">
                变动资源列表
              </div>
              <div className="p-4 space-y-4">
                {diffResult.resource_diff.added.length > 0 && (
                  <div className="space-y-2">
                    <div className="text-emerald-400 font-semibold text-xs flex items-center gap-1">
                      <Plus className="w-3.5 h-3.5" />
                       新增资源 ({diffResult.resource_diff.added.length})
                    </div>
                    <div className="divide-y divide-neutral-800 bg-neutral-950 rounded-lg border border-neutral-800 font-mono text-xs">
                      {diffResult.resource_diff.added.map((r) => (
                        <div key={r.id} className="p-2 flex items-center justify-between">
                          <span className="text-neutral-200 truncate max-w-xl">{r.url}</span>
                          <span className="text-neutral-500">{r.mime_type}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {diffResult.resource_diff.removed.length > 0 && (
                  <div className="space-y-2">
                    <div className="text-rose-400 font-semibold text-xs flex items-center gap-1">
                      <Minus className="w-3.5 h-3.5" />
                      移除资源 ({diffResult.resource_diff.removed.length})
                    </div>
                    <div className="divide-y divide-neutral-800 bg-neutral-950 rounded-lg border border-neutral-800 font-mono text-xs">
                      {diffResult.resource_diff.removed.map((r) => (
                        <div key={r.id} className="p-2 flex items-center justify-between">
                          <span className="text-neutral-400 truncate max-w-xl line-through">{r.url}</span>
                          <span className="text-neutral-600">{r.mime_type}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          ) : activeTab === 'visual' ? (
            <div className="bg-neutral-900/60 border border-neutral-800 rounded-lg overflow-hidden text-xs">
              <div className="p-2.5 bg-neutral-900/80 border-b border-neutral-800 font-semibold text-neutral-200 flex items-center justify-between">
                <span>像素差异图 (红色高亮差异区域)</span>
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
                  <div className="p-8 text-center text-neutral-500 text-xs">
                    快照无对应视觉截图或尺寸不一致，无法生成像素级对比
                  </div>
                )}
              </div>
            </div>
          ) : (
            <div className="bg-neutral-900/60 border border-neutral-800 rounded-lg overflow-hidden text-xs">
              <div className="p-2.5 bg-neutral-900/80 border-b border-neutral-800 font-semibold text-neutral-200 flex items-center justify-between">
                <span>DOM 树结构变化 (节点分布与标签增减)</span>
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
                      <div className="bg-neutral-950 border border-neutral-800 rounded p-2.5">
                        <span className="text-xs text-neutral-500 block">旧版节点总数</span>
                        <span className="text-sm font-bold font-mono text-neutral-200">
                          {diffResult.dom_diff.total_tags_old.toLocaleString()}
                        </span>
                      </div>
                      <div className="bg-neutral-950 border border-neutral-800 rounded p-2.5">
                        <span className="text-xs text-neutral-500 block">新版节点总数</span>
                        <span className="text-sm font-bold font-mono text-neutral-200">
                          {diffResult.dom_diff.total_tags_new.toLocaleString()}
                        </span>
                      </div>
                      <div className="bg-neutral-950 border border-neutral-800 rounded p-2.5">
                        <span className="text-xs text-neutral-500 block">变动标签类目</span>
                        <span className="text-sm font-bold font-mono text-cyan-400">
                          {diffResult.dom_diff.tag_changes.length} 个
                        </span>
                      </div>
                    </div>

                    {diffResult.dom_diff.tag_changes.length > 0 ? (
                      <div className="space-y-2">
                        <div className="text-xs font-semibold text-neutral-300">节点标签数量变化</div>
                        <div className="divide-y divide-neutral-800 bg-neutral-950 rounded-lg border border-neutral-800 font-mono text-xs overflow-hidden">
                          <div className="grid grid-cols-4 p-2 bg-neutral-900/60 font-sans text-neutral-400 font-medium">
                            <span>标签名称</span>
                            <span className="text-right">旧版本数量</span>
                            <span className="text-right">新版本数量</span>
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
                    ) : (
                      <div className="p-6 text-center text-neutral-500">
                        DOM 节点标签分布完全一致，未检测到结构层级变化
                      </div>
                    )}
                  </>
                ) : (
                  <div className="p-8 text-center text-neutral-500">
                    快照无对应 WARC 主文档或格式不支持，未能提取 DOM 结构
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      ) : null}
    </div>
  );
};
