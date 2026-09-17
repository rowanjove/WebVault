import React from 'react';
import {
  LayoutDashboard,
  Globe,
  Clock,
  PlaySquare,
  Search,
  GitCompare,
  Activity,
  ListTodo,
  History,
  Settings,
  HardDrive,
  ShieldCheck,
  PlusCircle,
  Sparkles,
} from 'lucide-react';
import { useAppStore, TabId } from '../../stores/useAppStore';

interface NavItem {
  id: TabId;
  label: string;
  icon: React.ElementType;
  badge?: string | number;
}

export const Sidebar: React.FC = () => {
  const { currentTab, setCurrentTab, stats, setIsAddSiteOpen, jobs, setIsOnboardingOpen } = useAppStore();

  const activeJobsCount = jobs.filter((j) => j.status === 'running').length;

  const navItems: NavItem[] = [
    { id: 'dashboard', label: '仪表盘', icon: LayoutDashboard },
    { id: 'sites', label: '站点管理', icon: Globe },
    { id: 'timemachine', label: '网页时光机', icon: Clock },
    { id: 'search', label: '全文搜索', icon: Search },
    { id: 'monitor', label: '页面监控', icon: Activity },
    { id: 'tasks', label: '任务队列', icon: ListTodo, badge: activeJobsCount > 0 ? activeJobsCount : undefined },
    { id: 'wayback', label: 'Wayback', icon: History },
    { id: 'settings', label: '系统设置', icon: Settings },
  ];

  const formatBytes = (bytes?: number) => {
    if (!bytes) return '0 MB';
    const mb = bytes / (1024 * 1024);
    if (mb < 1024) return `${mb.toFixed(1)} MB`;
    return `${(mb / 1024).toFixed(2)} GB`;
  };

  return (
    <aside className="w-68 h-screen bg-neutral-900 border-r border-neutral-800 flex flex-col justify-between shrink-0 select-none transition-colors duration-150 shadow-xs">
      {/* Top branding & navigation */}
      <div className="flex flex-col flex-1 min-h-0">
        {/* Top branding (移除版本号，纯粹简洁) */}
        <div className="p-4 border-b border-neutral-800/90 flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <img
              src="/logo.png"
              alt="WebVault Logo"
              className="w-9 h-9 rounded-lg object-cover shadow-sm bg-neutral-800"
            />
            <div>
              <div className="text-base font-bold tracking-tight text-neutral-100 flex items-center">
                WebVault
              </div>
              <div className="text-xs text-neutral-500 font-medium">网页归档与时光机</div>
            </div>
          </div>
        </div>

        {/* Action Button (放大 1.5 倍) */}
        <div className="p-3.5 pb-2">
          <button
            onClick={() => setIsAddSiteOpen(true)}
            className="w-full flex items-center justify-center gap-2.5 py-2.5 px-4 text-sm font-semibold rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white shadow-xs transition-all active:scale-[0.98] cursor-pointer"
          >
            <PlusCircle className="w-5 h-5" />
            添加站点
          </button>
        </div>

        {/* Navigation list (按钮保持 1.5 倍舒适大尺寸) */}
        <nav className="flex-1 px-3 py-1.5 space-y-1.5 overflow-y-auto">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive =
              currentTab === item.id ||
              (item.id === 'timemachine' &&
                (currentTab === 'timeline' || currentTab === 'replay' || currentTab === 'diff'));
            return (
              <button
                key={item.id}
                onClick={() => setCurrentTab(item.id)}
                className={`w-full flex items-center justify-between px-3.5 py-2.5 rounded-lg text-[15px] font-medium transition-all cursor-pointer ${
                  isActive
                    ? 'bg-neutral-800 text-neutral-100 font-semibold shadow-xs'
                    : 'text-neutral-400 hover:text-neutral-100 hover:bg-neutral-800/50'
                }`}
              >
                <div className="flex items-center gap-3">
                  <Icon
                    className={`w-5 h-5 transition-colors ${
                      isActive ? 'text-indigo-600 dark:text-indigo-400' : 'text-neutral-400'
                    }`}
                  />
                  <span>{item.label}</span>
                </div>
                {item.badge !== undefined && (
                  <span className="px-2 py-0.5 text-xs font-mono font-semibold bg-indigo-500/20 text-indigo-600 dark:text-indigo-300 rounded-full">
                    {item.badge}
                  </span>
                )}
              </button>
            );
          })}
        </nav>

        {/* Interactive Tour Trigger Button */}
        <div className="px-3 pb-2 pt-1">
          <button
            onClick={() => setIsOnboardingOpen(true)}
            className="w-full flex items-center justify-between px-3 py-2 rounded-lg bg-neutral-900/90 hover:bg-neutral-800 text-neutral-300 hover:text-neutral-100 transition text-xs font-medium cursor-pointer border border-neutral-800 shadow-2xs group"
            title="打开交互式新手教程与流程指引"
          >
            <div className="flex items-center gap-2.5">
              <Sparkles className="w-4 h-4 text-indigo-400 group-hover:rotate-12 transition-transform" />
              <span>新手指南与流程</span>
            </div>
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-indigo-500/15 text-indigo-400 font-semibold font-mono">
              指引
            </span>
          </button>
        </div>
      </div>

      {/* Bottom Status Info (纯粹保留状态指示，已移出深浅切换) */}
      <div className="p-4 border-t border-neutral-800/90 bg-neutral-950/30 space-y-2.5 font-mono text-xs text-neutral-400">
        <div className="flex items-center justify-between">
          <span className="flex items-center gap-2 text-neutral-400 font-sans text-xs">
            <HardDrive className="w-4 h-4 text-neutral-400" />
            已用存储
          </span>
          <span className="text-neutral-200 font-semibold">{formatBytes(stats?.storage_bytes)}</span>
        </div>

        <div className="flex items-center justify-between">
          <span className="flex items-center gap-2 text-neutral-400 font-sans text-xs">
            <ShieldCheck className="w-4 h-4 text-neutral-400" />
            浏览器引擎
          </span>
          <span className="flex items-center gap-2">
            <span
              className={`w-2 h-2 rounded-full ${
                stats?.browser_detected ? 'bg-emerald-500 ring-2 ring-emerald-500/20' : 'bg-amber-500 ring-2 ring-amber-500/20'
              }`}
            />
            <span className="text-neutral-200 font-sans text-xs font-medium">
              {stats?.browser_detected ? '就绪' : '未检测到'}
            </span>
          </span>
        </div>
      </div>
    </aside>
  );
};
