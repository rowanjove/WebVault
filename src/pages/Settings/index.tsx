import React, { useState } from 'react';
import {
  Settings as SettingsIcon,
  HardDrive,
  ShieldCheck,
  Upload,
  Info,
  CheckCircle2,
  Sun,
  Moon,
  FolderOpen,
  Copy,
  Sparkles,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api, pickOpenFile, pickSaveFile } from '../../services/tauri';
import { toast } from '../../components/ui/Toast';

export const Settings: React.FC = () => {
  const { stats, setSites, setStats, theme, toggleTheme, setIsOnboardingOpen } = useAppStore();
  const [importPath, setImportPath] = useState('');
  const [importLoading, setImportLoading] = useState(false);
  const [importSuccess, setImportSuccess] = useState<string | null>(null);
  const [backupLoading, setBackupLoading] = useState(false);

  const handleOpenStoragePath = async () => {
    const path = stats?.storage_path;
    if (!path) {
      toast.error('未获取到存储路径');
      return;
    }
    try {
      await api.openFolder(path);
      toast.success('已打开存储目录');
    } catch (e: any) {
      console.error('Failed to open folder:', e);
      try {
        await navigator.clipboard.writeText(path);
        toast.info('未能直接打开，已将路径复制到剪贴板');
      } catch {
        toast.error(`打开目录失败: ${e?.toString()}`);
      }
    }
  };

  const handleCopyStoragePath = async () => {
    const path = stats?.storage_path;
    if (!path) return;
    try {
      await navigator.clipboard.writeText(path);
      toast.success('存储路径已复制到剪贴板');
    } catch {
      toast.error('复制失败');
    }
  };

  const handleImportWacz = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!importPath.trim()) return;

    try {
      setImportLoading(true);
      const newSite = await api.importWacz(importPath.trim());
      const updated = await api.listSites();
      setSites(updated);
      setImportSuccess(`成功导入 WACZ 归档并创建站点: ${newSite.name}`);
      toast.success(`成功导入 WACZ 归档并创建站点: ${newSite.name}`);
      setImportPath('');
    } catch (err: any) {
      toast.error(`导入失败: ${err?.toString()}`);
    } finally {
      setImportLoading(false);
    }
  };

  const isDark = theme === 'dark';

  return (
    <div className="flex-1 h-screen overflow-y-auto bg-neutral-950 text-neutral-200 p-5 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between pb-3 border-b border-neutral-800">
        <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
          <SettingsIcon className="w-4 h-4 text-indigo-400" />
          系统设置
        </h1>

        {/* 无文字纯图标深浅切换按钮 */}
        <button
          onClick={toggleTheme}
          className="p-2 rounded-lg border border-neutral-800 bg-neutral-900 hover:bg-neutral-800 text-neutral-200 transition shadow-2xs cursor-pointer active:scale-95 flex items-center justify-center"
          title={isDark ? '切换为白灰浅色' : '切换为深色夜间'}
        >
          {isDark ? (
            <Sun className="w-4 h-4 text-amber-500" />
          ) : (
            <Moon className="w-4 h-4 text-indigo-400" />
          )}
        </button>
      </div>

      {/* Settings list - 拉到右侧顶住 (w-full) */}
      <div className="w-full space-y-5 text-sm">
        {/* Interactive Tour Card */}
        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 shadow-2xs">
          <div className="flex items-center justify-between">
            <h2 className="font-semibold text-neutral-100 flex items-center gap-2 text-sm">
              <Sparkles className="w-4 h-4 text-indigo-400" />
              新手教程与流程指引
            </h2>
            <button
              type="button"
              onClick={() => setIsOnboardingOpen(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition cursor-pointer shadow-xs active:scale-95"
            >
              <Sparkles className="w-3.5 h-3.5" />
              <span>打开指引</span>
            </button>
          </div>
          <p className="text-xs text-neutral-400">
            随时复习 WebVault 的核心工作链路：真实会话抓取、时光机穿梭、版本比对与自动化监控。
          </p>
        </div>

        {/* Browser Engine */}
        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 shadow-2xs">
          <div className="flex items-center justify-between">
            <h2 className="font-semibold text-neutral-100 flex items-center gap-2 text-sm">
              <ShieldCheck className="w-4 h-4 text-emerald-500" />
              浏览器引擎 (CDP)
            </h2>
            <span className={`flex items-center gap-1.5 text-xs font-mono px-2.5 py-1 rounded border font-medium ${
              stats?.browser_detected
                ? 'text-emerald-500 bg-emerald-500/10 border-emerald-500/20'
                : 'text-amber-400 bg-amber-500/10 border-amber-500/20'
            }`}>
              <span className={`w-2 h-2 rounded-full ${stats?.browser_detected ? 'bg-emerald-500' : 'bg-amber-400'}`} />
              {stats?.browser_detected ? '就绪' : '未检测到'}
            </span>
          </div>

          <div className="p-3 bg-neutral-950 rounded-lg border border-neutral-800 font-mono text-neutral-300 break-all text-xs">
            {stats?.browser_path || '未检测到可用浏览器 (Chrome / Edge)'}
          </div>
        </div>

        {/* Storage Location */}
        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 shadow-2xs">
          <div className="flex items-center justify-between">
            <h2 className="font-semibold text-neutral-100 flex items-center gap-2 text-sm">
              <HardDrive className="w-4 h-4 text-indigo-400" />
              存储路径
            </h2>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleCopyStoragePath}
                className="flex items-center gap-1.5 px-2.5 py-1.5 bg-neutral-800 hover:bg-neutral-700 text-neutral-300 rounded-lg text-xs font-medium transition cursor-pointer active:scale-95 border border-neutral-700/60"
                title="复制存储路径到剪贴板"
              >
                <Copy className="w-3.5 h-3.5 text-neutral-400" />
                <span>复制路径</span>
              </button>
              <button
                type="button"
                onClick={handleOpenStoragePath}
                className="flex items-center gap-1.5 px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition cursor-pointer shadow-xs active:scale-95"
                title="在系统文件管理器中打开该路径"
              >
                <FolderOpen className="w-3.5 h-3.5" />
                <span>打开目录</span>
              </button>
            </div>
          </div>

          <div className="p-3 bg-neutral-950 rounded-lg border border-neutral-800 font-mono text-neutral-300 break-all text-xs">
            {stats?.storage_path || 'AppData/WebVault'}
          </div>
        </div>

        {/* WACZ Import */}
        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 shadow-2xs">
          <h2 className="font-semibold text-neutral-100 flex items-center gap-2 text-sm">
            <Upload className="w-4 h-4 text-amber-500" />
            WACZ 归档导入
          </h2>

          <form onSubmit={handleImportWacz} className="flex items-center gap-3">
            <input
              type="text"
              placeholder="选择或输入 .wacz 文件路径..."
              value={importPath}
              onChange={(e) => setImportPath(e.target.value)}
              className="flex-1 px-3.5 py-2.5 bg-neutral-950 border border-neutral-800 rounded-lg text-neutral-200 font-mono text-sm focus:outline-hidden focus:border-indigo-500 shadow-2xs"
            />
            <button
              type="button"
              onClick={async () => {
                const selected = await pickOpenFile([{ name: 'WACZ', extensions: ['wacz'] }]);
                if (selected) setImportPath(selected);
              }}
              className="px-3 py-2.5 bg-neutral-800 hover:bg-neutral-700 text-neutral-200 rounded-lg text-xs font-medium transition shrink-0 cursor-pointer"
            >
              浏览
            </button>
            <button
              type="submit"
              disabled={importLoading}
              className="px-5 py-2.5 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-lg font-semibold text-xs transition shrink-0 cursor-pointer active:scale-95 shadow-xs"
            >
              {importLoading ? '导入中...' : '导入'}
            </button>
          </form>

          {importSuccess && (
            <div className="p-3 rounded-lg bg-emerald-950/40 border border-emerald-800 text-emerald-300 text-xs flex items-center gap-2">
              <CheckCircle2 className="w-4 h-4 shrink-0" />
              {importSuccess}
            </div>
          )}
        </div>

        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 shadow-2xs">
          <h2 className="font-semibold text-neutral-100 flex items-center gap-2 text-sm">
            <HardDrive className="w-4 h-4 text-cyan-400" />
            数据备份与恢复
          </h2>
          <p className="text-xs text-neutral-400">
            备份包含数据库、WARC 归档、截图和浏览器登录配置目录。恢复会覆盖当前本地数据，请先确认。
          </p>
          <div className="flex items-center gap-3">
            <button
              type="button"
              disabled={backupLoading}
              onClick={async () => {
                const path = await pickSaveFile({
                  defaultPath: `webvault-backup-${Date.now()}.zip`,
                  filters: [{ name: 'Zip', extensions: ['zip'] }],
                });
                if (!path) return;
                try {
                  setBackupLoading(true);
                  const saved = await api.exportBackup(path);
                  toast.success(`备份已保存：${saved}`);
                } catch (e: any) {
                  toast.error(`备份失败: ${e?.toString()}`);
                } finally {
                  setBackupLoading(false);
                }
              }}
              className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-lg text-xs font-semibold cursor-pointer"
            >
              导出备份
            </button>
            <button
              type="button"
              disabled={backupLoading}
              onClick={async () => {
                if (!confirm('恢复备份会覆盖当前归档数据，确定继续？')) return;
                const path = await pickOpenFile([{ name: 'Zip', extensions: ['zip'] }]);
                if (!path) return;
                try {
                  setBackupLoading(true);
                  await api.importBackup(path);
                  const [updated, st] = await Promise.all([api.listSites(), api.getSystemStats()]);
                  setSites(updated);
                  setStats(st);
                  toast.success('备份已恢复');
                } catch (e: any) {
                  toast.error(`恢复失败: ${e?.toString()}`);
                } finally {
                  setBackupLoading(false);
                }
              }}
              className="px-4 py-2 bg-neutral-800 hover:bg-neutral-700 disabled:opacity-50 text-neutral-200 rounded-lg text-xs font-semibold cursor-pointer"
            >
              从备份恢复
            </button>
          </div>
        </div>

        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 text-neutral-400 text-xs shadow-2xs">
          <div className="flex items-center gap-2 text-neutral-200 font-semibold text-sm">
            <ShieldCheck className="w-4 h-4 text-emerald-400" />
            隐私说明
          </div>
          <p className="text-neutral-300 leading-relaxed">
            WebVault 在本机运行，不上传归档、Cookie 或使用数据。登录凭证仅保存在本地数据库，抓取依赖本机已安装的 Chrome / Edge。日志写在存储目录的 logs/webvault.log，不会自动外发。
          </p>
        </div>

        {/* Software Info */}
        <div className="bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-3 text-neutral-400 text-xs shadow-2xs">
          <div className="flex items-center gap-2 text-neutral-200 font-semibold text-sm">
            <Info className="w-4 h-4 text-neutral-400" />
            关于 WebVault
          </div>
          <div className="grid grid-cols-2 gap-3 pt-1 text-sm text-neutral-300">
            <div>版本：v0.1.1</div>
            <div>归档格式：WARC 1.1 / WACZ 1.1.1</div>
            <div>存储引擎：SQLite (WAL + FTS5)</div>
            <div>沙箱环境：127.0.0.1 隔离代理</div>
          </div>
        </div>
      </div>
    </div>
  );
};
