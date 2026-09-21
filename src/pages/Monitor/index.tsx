import React, { useEffect, useState } from 'react';
import {
  Activity,
  Plus,
  Clock,
  Trash2,
  GitCompare,
  CheckCircle,
  XCircle,
  Sliders,
  Bell,
  Search,
  RotateCw,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api } from '../../services/tauri';
import type { MonitorRule, ChangeEventItem } from '../../types';

export const Monitor: React.FC = () => {
  const { sites, setDiffPair, openTimeMachine } = useAppStore();

  const [rules, setRules] = useState<MonitorRule[]>([]);
  const [events, setEvents] = useState<ChangeEventItem[]>([]);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [checking, setChecking] = useState(false);

  // New rule form
  const [url, setUrl] = useState('');
  const [siteId, setSiteId] = useState('');
  const [schedule, setSchedule] = useState('1h');
  const [strategy, setStrategy] = useState<'conditional_get' | 'content_hash' | 'selector' | 'keyword'>('content_hash');
  const [keyword, setKeyword] = useState('');

  const loadData = async () => {
    try {
      const [r, ev] = await Promise.all([api.listMonitorRules(), api.listChangeEvents()]);
      setRules(r);
      setEvents(ev);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    loadData();
    if (sites.length > 0 && !siteId) {
      setSiteId(sites[0].id);
    }
  }, [sites]);

  const handleCreateRule = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim() || !siteId) return;

    try {
      await api.createMonitorRule({
        site_id: siteId,
        url: url.trim(),
        schedule,
        strategy,
        keyword: keyword.trim() || undefined,
        enabled: true,
      });
      setIsModalOpen(false);
      setUrl('');
      setKeyword('');
      loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleToggleRule = async (id: string, enabled: boolean) => {
    try {
      await api.toggleMonitorRule(id, enabled);
      loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleDeleteRule = async (id: string) => {
    if (!confirm('确定要删除此监控项吗？')) return;
    try {
      await api.deleteMonitorRule(id);
      loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleTriggerCheck = async () => {
    try {
      setChecking(true);
      await api.triggerMonitorCheck();
      await loadData();
    } catch (e) {
      console.error(e);
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="flex-1 h-screen overflow-y-auto bg-neutral-950 text-neutral-200 p-5 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between pb-3 border-b border-neutral-800">
        <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
          <Activity className="w-4 h-4 text-indigo-400" />
          页面监控
        </h1>

        <div className="flex items-center gap-2">
          <button
            onClick={handleTriggerCheck}
            disabled={checking}
            className="flex items-center gap-1.5 px-2.5 py-1.5 bg-neutral-800 hover:bg-neutral-700 disabled:opacity-50 text-neutral-200 rounded text-xs font-medium transition"
            title="立即对满足条件的规则执行巡检"
          >
            <RotateCw className={`w-3.5 h-3.5 ${checking ? 'animate-spin' : ''}`} />
            {checking ? '巡检中...' : '立即巡检'}
          </button>

          <button
            onClick={() => setIsModalOpen(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded text-xs font-medium transition shadow-xs"
          >
            <Plus className="w-3.5 h-3.5" />
            新建监控
          </button>
        </div>
      </div>

      {/* Rules Section */}
      <div className="space-y-3">
        <h2 className="text-sm font-semibold text-neutral-100 flex items-center gap-2">
          <Activity className="w-4 h-4 text-indigo-500" />
          监控规则 ({rules.length})
        </h2>

        <div className="bg-neutral-900 border border-neutral-800 rounded-xl overflow-hidden text-sm shadow-2xs">
          {rules.length === 0 ? (
            <div className="p-10 text-center text-neutral-400 text-sm">
              暂无监控规则，点击右上角【新建监控】添加。
            </div>
          ) : (
            <div className="divide-y divide-neutral-800">
              {rules.map((rule) => (
                <div key={rule.id} className="p-4 flex items-center justify-between hover:bg-neutral-950/40 transition">
                  <div className="space-y-1.5 max-w-xl">
                    <div className="font-mono text-neutral-100 text-sm font-semibold truncate">{rule.url}</div>
                    <div className="flex items-center gap-3 text-xs text-neutral-400 font-mono">
                      <span>频率: <strong className="text-neutral-200">{rule.schedule}</strong></span>
                      <span>·</span>
                      <span>策略: <strong className="text-neutral-200">{rule.strategy}</strong></span>
                      {rule.keyword && (
                        <>
                          <span>·</span>
                          <span>关键词: <strong className="text-indigo-400">{rule.keyword}</strong></span>
                        </>
                      )}
                    </div>
                  </div>

                  <div className="flex items-center gap-3.5">
                    <div className="text-right text-xs text-neutral-400 font-mono">
                      <div>下次: {rule.next_check ? new Date(rule.next_check).toLocaleTimeString('zh-CN') : '-'}</div>
                      <div className="text-neutral-400 text-xs font-medium">{rule.last_status || '待巡检'}</div>
                    </div>

                    <button
                      onClick={() => handleToggleRule(rule.id, !rule.enabled)}
                      className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition cursor-pointer ${
                        rule.enabled
                          ? 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-300 hover:bg-emerald-500/30'
                          : 'bg-neutral-800 text-neutral-400 hover:bg-neutral-700'
                      }`}
                    >
                      {rule.enabled ? '已开启' : '已暂停'}
                    </button>

                    <button
                      onClick={() => handleDeleteRule(rule.id)}
                      className="p-2 rounded-lg hover:bg-red-950/60 text-neutral-400 hover:text-red-400 transition cursor-pointer"
                      title="删除"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Change Events Feed */}
      <div className="space-y-3">
        <h2 className="text-sm font-semibold text-neutral-100 flex items-center gap-2">
          <GitCompare className="w-4 h-4 text-amber-500" />
          变动事件 ({events.length})
        </h2>

        <div className="bg-neutral-900 border border-neutral-800 rounded-xl overflow-hidden text-sm shadow-2xs">
          {events.length === 0 ? (
            <div className="p-10 text-center text-neutral-400 text-sm">
              暂未检测到内容变动
            </div>
          ) : (
            <div className="divide-y divide-neutral-800">
              {events.map((ev) => (
                <div key={ev.id} className="p-4 flex items-center justify-between hover:bg-neutral-950/40 transition">
                  <div className="space-y-1">
                    <div className="font-semibold text-neutral-100 text-sm md:text-base">{ev.title || ev.url}</div>
                    <div className="text-xs text-neutral-400 font-mono truncate max-w-lg">{ev.url}</div>
                  </div>

                  <div className="flex items-center gap-3.5">
                    <span className="text-xs font-mono px-2.5 py-1 rounded-md bg-amber-500/15 text-amber-600 dark:text-amber-300 font-semibold">
                      变动 {ev.change_score}%
                    </span>

                    <span className="text-xs text-neutral-400 font-mono">
                      {new Date(ev.created_at).toLocaleString('zh-CN')}
                    </span>

                    {ev.old_capture_id && (
                      <button
                        onClick={() =>
                          setDiffPair(ev.old_capture_id!, ev.new_capture_id, {
                            siteId: ev.site_id,
                            pageId: ev.page_id,
                          })
                        }
                        className="px-3 py-1.5 bg-neutral-800 hover:bg-neutral-700 text-neutral-200 rounded-lg text-xs font-semibold flex items-center gap-1.5 transition cursor-pointer"
                      >
                        <GitCompare className="w-3.5 h-3.5 text-amber-400" />
                        比对
                      </button>
                    )}

                    <button
                      onClick={() =>
                        openTimeMachine({
                          siteId: ev.site_id,
                          pageId: ev.page_id,
                          captureId: ev.new_capture_id,
                        })
                      }
                      className="px-2.5 py-1.5 bg-indigo-600/20 hover:bg-indigo-600/30 text-indigo-300 rounded-lg text-xs font-medium flex items-center gap-1.5 transition cursor-pointer"
                    >
                      <Clock className="w-3.5 h-3.5 text-indigo-400" />
                      回放
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Create Rule Modal */}
      {isModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-xs p-4">
          <div className="bg-neutral-900 border border-neutral-800 rounded-lg max-w-md w-full p-4 shadow-xl space-y-3">
            <h3 className="text-sm font-semibold text-neutral-100 flex items-center gap-2">
              <Activity className="w-4 h-4 text-indigo-400" />
              新建监控规则
            </h3>

            <form onSubmit={handleCreateRule} className="space-y-3 text-xs">
              <div>
                <label className="block text-neutral-400 mb-1">所属站点</label>
                <select
                  value={siteId}
                  onChange={(e) => setSiteId(e.target.value)}
                  className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200 focus:outline-hidden focus:border-neutral-700"
                >
                  {sites.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} ({s.normalized_host})
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-neutral-400 mb-1">网页 URL</label>
                <input
                  type="text"
                  placeholder="https://example.com/changelog"
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  required
                  className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200 font-mono focus:outline-hidden focus:border-neutral-700"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-neutral-400 mb-1">检查周期</label>
                  <select
                    value={schedule}
                    onChange={(e) => setSchedule(e.target.value)}
                    className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200 focus:outline-hidden focus:border-neutral-700"
                  >
                    <option value="5m">每 5 分钟</option>
                    <option value="15m">每 15 分钟</option>
                    <option value="30m">每 30 分钟</option>
                    <option value="1h">每 1 小时</option>
                    <option value="6h">每 6 小时</option>
                    <option value="24h">每天 (24小时)</option>
                  </select>
                </div>

                <div>
                  <label className="block text-neutral-400 mb-1">比对策略</label>
                  <select
                    value={strategy}
                    onChange={(e) => setStrategy(e.target.value as any)}
                    className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200 focus:outline-hidden focus:border-neutral-700"
                  >
                    <option value="content_hash">正文哈希变动</option>
                    <option value="conditional_get">HTTP ETag 条件请求</option>
                    <option value="keyword">关键词追踪</option>
                  </select>
                </div>
              </div>

              {strategy === 'keyword' && (
                <div>
                  <label className="block text-neutral-400 mb-1">关注关键词</label>
                  <input
                    type="text"
                    placeholder="如：已售罄、Version 2.0"
                    value={keyword}
                    onChange={(e) => setKeyword(e.target.value)}
                    className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200 focus:outline-hidden focus:border-neutral-700"
                  />
                </div>
              )}

              <div className="flex justify-end gap-2 pt-2 border-t border-neutral-800">
                <button
                  type="button"
                  onClick={() => setIsModalOpen(false)}
                  className="px-3 py-1.5 text-neutral-400 hover:text-neutral-200 font-medium"
                >
                  取消
                </button>
                <button
                  type="submit"
                  className="px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded font-medium transition"
                >
                  保存
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
