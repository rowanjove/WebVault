import React, { useEffect, useState } from 'react';
import {
  Globe,
  Trash2,
  Play,
  Download,
  Settings,
  Clock,
  Layers,
  Search,
  ExternalLink,
  ChevronRight,
  Sliders,
  Check,
  KeyRound,
  Upload,
  X,
  Rss,
  Code,
  Plus,
  RefreshCw,
  Activity,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';
import { api, pickSaveFile } from '../../services/tauri';
import { toast } from '../../components/ui/Toast';
import type { Site, PageItem, CrawlProfile, SiteCredential, UserScriptItem, RssFeedItem } from '../../types';

export const Sites: React.FC = () => {
  const {
    sites,
    setSites,
    selectedSiteId,
    setSelectedSiteId,
    setSelectedPageId,
    setCurrentTab,
    openTimeMachine,
    setIsAddSiteOpen,
  } = useAppStore();

  const [filterText, setFilterText] = useState('');
  const [pages, setPages] = useState<PageItem[]>([]);
  const [profile, setProfile] = useState<CrawlProfile | null>(null);
  const [credentials, setCredentials] = useState<SiteCredential[]>([]);
  const [scripts, setScripts] = useState<UserScriptItem[]>([]);
  const [rssFeeds, setRssFeeds] = useState<RssFeedItem[]>([]);
  const [activeTab, setActiveTab] = useState<'pages' | 'profile' | 'credentials' | 'scripts' | 'rss'>('pages');
  const [savingProfile, setSavingProfile] = useState(false);
  const [savedSuccess, setSavedSuccess] = useState(false);

  // Batch URL modal state
  const [isBatchImportOpen, setIsBatchImportOpen] = useState(false);
  const [batchUrlsText, setBatchUrlsText] = useState('');
  const [batchImporting, setBatchImporting] = useState(false);

  // Interactive login credential modal state
  const [isRecordCredOpen, setIsRecordCredOpen] = useState(false);
  const [credName, setCredName] = useState('default');
  const [credUrl, setCredUrl] = useState('');
  const [interactivePort, setInteractivePort] = useState<number | null>(null);
  const [launchingBrowser, setLaunchingBrowser] = useState(false);
  const [savingCred, setSavingCred] = useState(false);

  // User script modal state
  const [isAddScriptOpen, setIsAddScriptOpen] = useState(false);
  const [scriptName, setScriptName] = useState('');
  const [scriptContent, setScriptContent] = useState('');
  const [savingScript, setSavingScript] = useState(false);

  // RSS Feed modal state
  const [isAddRssOpen, setIsAddRssOpen] = useState(false);
  const [rssUrl, setRssUrl] = useState('');
  const [rssTitle, setRssTitle] = useState('');
  const [savingRss, setSavingRss] = useState(false);
  const [syncingRssId, setSyncingRssId] = useState<string | null>(null);

  // Pick first site if none selected
  useEffect(() => {
    if (!selectedSiteId && sites.length > 0) {
      setSelectedSiteId(sites[0].id);
    }
  }, [sites, selectedSiteId]);

  const activeSite = sites.find((s) => s.id === selectedSiteId);

  useEffect(() => {
    if (selectedSiteId) {
      api.listPages(selectedSiteId).then(setPages).catch(console.error);
      api.getSiteProfile(selectedSiteId).then(setProfile).catch(console.error);
      api.getSiteCredentials(selectedSiteId).then(setCredentials).catch(console.error);
      api.listUserScripts(selectedSiteId).then(setScripts).catch(console.error);
      api.listRssFeeds(selectedSiteId).then(setRssFeeds).catch(console.error);
    }
  }, [selectedSiteId]);

  const handleSaveScript = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedSiteId || !scriptName.trim() || !scriptContent.trim()) return;
    try {
      setSavingScript(true);
      await api.saveUserScript({
        siteId: selectedSiteId,
        name: scriptName.trim(),
        scriptContent: scriptContent.trim(),
        enabled: true,
      });
      const updated = await api.listUserScripts(selectedSiteId);
      setScripts(updated);
      setIsAddScriptOpen(false);
      setScriptName('');
      setScriptContent('');
      toast.success('脚本已保存');
    } catch (e: any) {
      toast.error(`保存脚本失败: ${e?.toString()}`);
    } finally {
      setSavingScript(false);
    }
  };

  const handleDeleteScript = async (id: string) => {
    if (!confirm('确定要删除此用户脚本吗？')) return;
    try {
      await api.deleteUserScript(id);
      if (selectedSiteId) {
        const updated = await api.listUserScripts(selectedSiteId);
        setScripts(updated);
      }
      toast.success('脚本已删除');
    } catch (e: any) {
      toast.error(`删除失败: ${e?.toString()}`);
    }
  };

  const handleAddRss = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedSiteId || !rssUrl.trim()) return;
    try {
      setSavingRss(true);
      await api.addRssFeed({
        siteId: selectedSiteId,
        feedUrl: rssUrl.trim(),
        title: rssTitle.trim() || undefined,
      });
      const updated = await api.listRssFeeds(selectedSiteId);
      setRssFeeds(updated);
      setIsAddRssOpen(false);
      setRssUrl('');
      setRssTitle('');
      toast.success('RSS 订阅源已添加');
    } catch (e: any) {
      toast.error(`添加 RSS 源失败: ${e?.toString()}`);
    } finally {
      setSavingRss(false);
    }
  };

  const handleDeleteRss = async (id: string) => {
    if (!confirm('确定要删除此 RSS 订阅源吗？')) return;
    try {
      await api.deleteRssFeed(id);
      if (selectedSiteId) {
        const updated = await api.listRssFeeds(selectedSiteId);
        setRssFeeds(updated);
      }
      toast.success('RSS 订阅源已删除');
    } catch (e: any) {
      toast.error(`删除失败: ${e?.toString()}`);
    }
  };

  const handleSyncRss = async (id: string) => {
    try {
      setSyncingRssId(id);
      const newArticles = await api.syncRssFeed(id);
      toast.success(`RSS 同步完成，发现 ${newArticles} 条新链接（尚未抓取）。`);
      if (selectedSiteId) {
        const updatedPages = await api.listPages(selectedSiteId);
        setPages(updatedPages);
        const updatedFeeds = await api.listRssFeeds(selectedSiteId);
        setRssFeeds(updatedFeeds);
      }
    } catch (e: any) {
      toast.error(`同步失败: ${e?.toString()}`);
    } finally {
      setSyncingRssId(null);
    }
  };

  const handleQuickAddMonitor = async (url: string) => {
    if (!selectedSiteId) return;
    try {
      await api.createMonitorRule({
        site_id: selectedSiteId,
        url,
        schedule: '1h',
        strategy: 'content_hash',
        enabled: true,
      });
      toast.success('已将该页面加入定时监控');
    } catch (e: any) {
      toast.error(`添加监控失败: ${e?.toString()}`);
    }
  };

  const handleBatchImport = async () => {
    if (!selectedSiteId || !batchUrlsText.trim()) return;
    const urls = batchUrlsText
      .split('\n')
      .map((u) => u.trim())
      .filter((u) => u.length > 0 && (u.startsWith('http://') || u.startsWith('https://')));
    if (urls.length === 0) {
      toast.error('未检测到有效的 http:// 或 https:// 链接');
      return;
    }
    try {
      setBatchImporting(true);
      const count = await api.importBatchUrls({ siteId: selectedSiteId, urls });
      toast.success(`已成功导入 ${count} 个新链接！`);
      setBatchUrlsText('');
      setIsBatchImportOpen(false);
      const updatedPages = await api.listPages(selectedSiteId);
      setPages(updatedPages);
    } catch (e: any) {
      toast.error(`导入失败: ${e?.toString()}`);
    } finally {
      setBatchImporting(false);
    }
  };

  const handleLaunchBrowser = async () => {
    if (!selectedSiteId) return;
    const url = credUrl.trim() || activeSite?.root_url || '';
    if (!url) return;
    try {
      setLaunchingBrowser(true);
      const port = await api.launchInteractiveLogin({ siteId: selectedSiteId, loginUrl: url });
      setInteractivePort(port);
      toast.info('登录调试浏览器已启动，请在弹出的浏览器中登录');
    } catch (e: any) {
      toast.error(`启动浏览器失败: ${e?.toString()}`);
    } finally {
      setLaunchingBrowser(false);
    }
  };

  const handleSaveCredential = async () => {
    if (!selectedSiteId || !interactivePort) return;
    try {
      setSavingCred(true);
      const cred = await api.saveSiteCredential({
        siteId: selectedSiteId,
        port: interactivePort,
        name: credName.trim() || 'default',
      });
      toast.success(`凭证【${cred.name}】录制并保存成功！`);
      setIsRecordCredOpen(false);
      setInteractivePort(null);
      const updatedCreds = await api.getSiteCredentials(selectedSiteId);
      setCredentials(updatedCreds);
    } catch (e: any) {
      toast.error(`保存凭证失败: ${e?.toString()}`);
    } finally {
      setSavingCred(false);
    }
  };

  const handleDeleteCredential = async (credId: string) => {
    if (!confirm('确定要删除此登录凭证吗？')) return;
    try {
      await api.deleteSiteCredential(credId);
      if (selectedSiteId) {
        const updatedCreds = await api.getSiteCredentials(selectedSiteId);
        setCredentials(updatedCreds);
      }
      toast.success('凭证已删除');
    } catch (e: any) {
      toast.error(`删除失败: ${e?.toString()}`);
    }
  };

  const handleDeleteSite = async (id: string) => {
    if (!confirm('确定要删除此站点及其所有归档数据吗？此操作不可逆。')) return;
    try {
      await api.deleteSite(id);
      const updated = await api.listSites();
      setSites(updated);
      if (selectedSiteId === id) {
        setSelectedSiteId(updated[0]?.id || null);
      }
      toast.success('站点已删除');
    } catch (e: any) {
      toast.error(`删除站点失败: ${e?.toString()}`);
    }
  };

  const handleCaptureCover = async () => {
    if (!selectedSiteId || !activeSite) return;
    try {
      await api.startSingleCapture({ siteId: selectedSiteId, url: activeSite.root_url });
      toast.success('已开始抓取封面（仅当前页）');
      setCurrentTab('tasks');
    } catch (e: any) {
      toast.error(`抓取封面失败: ${e?.toString()}`);
    }
  };

  const handleStartCrawl = async () => {
    if (!selectedSiteId) return;
    const maxPages = profile?.max_pages ?? 500;
    if (
      !confirm(
        `整站抓取会顺着链接继续访问，最多 ${maxPages} 页，可能很久。只要首页请用「抓封面」。确定继续？`
      )
    ) {
      return;
    }
    try {
      await api.startCrawlJob({ siteId: selectedSiteId });
      toast.success('已开始整站抓取');
      setCurrentTab('tasks');
    } catch (e: any) {
      toast.error(`启动爬虫失败: ${e?.toString()}`);
    }
  };

  const handleSaveProfile = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!profile) return;
    try {
      setSavingProfile(true);
      await api.updateSiteProfile(profile);
      setSavedSuccess(true);
      toast.success('爬虫配置已更新');
      setTimeout(() => setSavedSuccess(false), 2000);
    } catch (e: any) {
      toast.error(`保存配置失败: ${e?.toString()}`);
    } finally {
      setSavingProfile(false);
    }
  };

  const handleExportWacz = async () => {
    if (!selectedSiteId || !activeSite) return;
    try {
      const exportPath = await pickSaveFile({
        defaultPath: `${activeSite.name}_${Date.now()}.wacz`,
        filters: [{ name: 'WACZ', extensions: ['wacz'] }],
      });
      if (!exportPath) return;
      const resultPath = await api.exportWacz({ siteId: selectedSiteId, outputPath: exportPath });
      toast.success(`WACZ 归档导出成功：${resultPath || exportPath}`);
    } catch (e: any) {
      toast.error(`导出失败: ${e?.toString()}`);
    }
  };

  const filteredSites = sites.filter(
    (s) =>
      s.name.toLowerCase().includes(filterText.toLowerCase()) ||
      s.root_url.toLowerCase().includes(filterText.toLowerCase())
  );

  return (
    <div className="flex-1 h-screen flex overflow-hidden bg-neutral-950 text-neutral-200">
      {/* Left site list column */}
      <div className="w-72 border-r border-neutral-800 flex flex-col justify-between shrink-0 bg-neutral-900/60">
        <div className="p-3.5 border-b border-neutral-800 space-y-2.5">
          <div className="flex items-center justify-between">
            <h1 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
              <Globe className="w-4 h-4 text-indigo-400" />
              站点管理
            </h1>
            <button
              onClick={() => setIsAddSiteOpen(true)}
              className="text-xs text-indigo-400 hover:text-indigo-300 font-semibold cursor-pointer"
            >
              + 添加
            </button>
          </div>
          <div className="relative flex items-center">
            <Search className="w-4 h-4 absolute left-3 text-neutral-400 pointer-events-none" />
            <input
              type="text"
              placeholder="搜索站点或域名..."
              value={filterText}
              onChange={(e) => setFilterText(e.target.value)}
              className="w-full pl-9 pr-3 py-2 bg-neutral-950 border border-neutral-800 rounded-lg text-xs text-neutral-200 placeholder-neutral-500 focus:outline-hidden focus:border-indigo-500 shadow-2xs"
            />
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-2.5 space-y-1.5">
          {filteredSites.map((site) => {
            const isSelected = site.id === selectedSiteId;
            return (
              <button
                key={site.id}
                onClick={() => setSelectedSiteId(site.id)}
                className={`w-full text-left p-3 rounded-lg text-xs transition space-y-1 cursor-pointer ${
                  isSelected
                    ? 'bg-neutral-800 text-neutral-100 font-medium shadow-2xs'
                    : 'hover:bg-neutral-800/60 border border-transparent text-neutral-400'
                }`}
              >
                <div className="flex items-center justify-between">
                  <span className="text-sm font-semibold text-neutral-100 truncate">{site.name}</span>
                  <span className="text-xs font-mono px-1.5 py-0.5 rounded bg-neutral-900 text-neutral-300">
                    {site.capture_count || 0}
                  </span>
                </div>
                <div className="text-xs text-neutral-400 truncate font-mono">
                  {site.normalized_host}
                </div>
              </button>
            );
          })}
        </div>
      </div>

      {/* Right site detail pane */}
      {activeSite ? (
        <div className="flex-1 flex flex-col h-full overflow-hidden">
          {/* Header */}
          <div className="p-4 border-b border-neutral-800 bg-neutral-900/40 flex items-center justify-between">
            <div className="space-y-1">
              <div className="flex items-center gap-2.5">
                <h1 className="text-lg font-bold text-neutral-100">{activeSite.name}</h1>
                <a
                  href={activeSite.root_url}
                  target="_blank"
                  rel="noreferrer"
                  className="text-neutral-400 hover:text-neutral-200 transition p-0.5"
                  title="在新标签页访问原站"
                >
                  <ExternalLink className="w-4 h-4" />
                </a>
              </div>
              <div className="text-xs text-neutral-400 font-mono">{activeSite.root_url}</div>
            </div>

            <div className="flex items-center gap-2.5">
              <button
                onClick={handleCaptureCover}
                className="flex items-center gap-2 px-3.5 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition shadow-xs cursor-pointer active:scale-95"
                title="只抓站点首页这一页，不顺着链接往下爬"
              >
                <Play className="w-3.5 h-3.5 fill-white" />
                抓封面
              </button>
              <button
                onClick={handleStartCrawl}
                className="flex items-center gap-2 px-3.5 py-2 bg-neutral-800 hover:bg-neutral-700 text-neutral-200 rounded-lg text-xs font-medium transition cursor-pointer"
                title="顺着链接递归抓取，页数受站点配置限制"
              >
                整站抓取
              </button>

              <button
                onClick={handleExportWacz}
                className="flex items-center gap-2 px-3.5 py-2 bg-neutral-800 hover:bg-neutral-700 text-neutral-200 rounded-lg text-xs font-medium transition cursor-pointer"
              >
                <Download className="w-3.5 h-3.5" />
                导出 WACZ
              </button>

              <button
                onClick={() => handleDeleteSite(activeSite.id)}
                className="p-2 bg-neutral-800 hover:bg-red-950/60 hover:text-red-400 text-neutral-400 rounded-lg transition cursor-pointer"
                title="删除站点"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            </div>
          </div>

          {/* Sub Navigation */}
          <div className="flex items-center gap-5 px-5 border-b border-neutral-800 text-sm font-medium">
            <button
              onClick={() => setActiveTab('pages')}
              className={`py-3 border-b-2 transition cursor-pointer ${
                activeTab === 'pages'
                  ? 'border-indigo-500 text-indigo-600 dark:text-indigo-400 font-semibold'
                  : 'border-transparent text-neutral-400 hover:text-neutral-200'
              }`}
            >
              页面列表 ({pages.length})
            </button>
            <button
              onClick={() => setActiveTab('profile')}
              className={`py-3 border-b-2 transition cursor-pointer ${
                activeTab === 'profile'
                  ? 'border-indigo-500 text-indigo-600 dark:text-indigo-400 font-semibold'
                  : 'border-transparent text-neutral-400 hover:text-neutral-200'
              }`}
            >
              抓取配置
            </button>
            <button
              onClick={() => setActiveTab('credentials')}
              className={`py-2.5 border-b-2 font-medium transition ${
                activeTab === 'credentials'
                  ? 'border-indigo-500 text-indigo-400'
                  : 'border-transparent text-neutral-400 hover:text-neutral-200'
              }`}
            >
              登录凭证 ({credentials.length})
            </button>
            <button
              onClick={() => setActiveTab('scripts')}
              className={`py-2.5 border-b-2 font-medium transition ${
                activeTab === 'scripts'
                  ? 'border-indigo-500 text-indigo-400'
                  : 'border-transparent text-neutral-400 hover:text-neutral-200'
              }`}
            >
              用户脚本 ({scripts.length})
            </button>
            <button
              onClick={() => setActiveTab('rss')}
              className={`py-2.5 border-b-2 font-medium transition ${
                activeTab === 'rss'
                  ? 'border-indigo-500 text-indigo-400'
                  : 'border-transparent text-neutral-400 hover:text-neutral-200'
              }`}
            >
              RSS 订阅 ({rssFeeds.length})
            </button>
          </div>

          {/* Tab Content */}
          <div className="flex-1 overflow-y-auto p-4">
            {activeTab === 'pages' ? (
              <div className="space-y-3">
                <div className="flex items-center justify-between pb-1">
                  <span className="text-xs text-neutral-400 font-mono">共 {pages.length} 个页面</span>
                  <button
                    onClick={() => setIsBatchImportOpen(true)}
                    className="flex items-center gap-1 px-2.5 py-1 bg-neutral-800 hover:bg-neutral-700 text-neutral-200 rounded text-xs transition"
                  >
                    <Upload className="w-3.5 h-3.5 text-neutral-400" />
                    批量导入 URL
                  </button>
                </div>

                {pages.length === 0 ? (
                  <div className="text-center py-12 text-neutral-500 text-xs">
                    暂无页面记录。只要首页请点右上角【抓封面】；要顺着链接抓全站再用【整站抓取】。
                  </div>
                ) : (
                  pages.map((page) => (
                    <div
                      key={page.id}
                      className="p-2.5 bg-neutral-900/60 border border-neutral-800 hover:border-neutral-700 rounded-md flex items-center justify-between transition"
                    >
                      <div className="space-y-0.5 max-w-xl">
                        <div className="text-xs font-semibold text-neutral-200 truncate">
                          {page.title || page.url}
                        </div>
                        <div className="text-xs text-neutral-500 font-mono truncate">
                          {page.url}
                        </div>
                      </div>

                      <div className="flex items-center gap-2">
                        <span className="text-xs text-neutral-400 font-mono mr-1">
                          {page.capture_count} 快照
                        </span>
                        <button
                          onClick={async () => {
                            if (!selectedSiteId) return;
                            try {
                              await api.startSingleCapture({ siteId: selectedSiteId, url: page.url });
                              toast.success('已开始抓取这一页');
                              setCurrentTab('tasks');
                            } catch (e: any) {
                              toast.error(`抓取失败: ${e?.toString()}`);
                            }
                          }}
                          className="px-2 py-1 text-xs bg-neutral-800 hover:bg-neutral-700 text-neutral-300 rounded flex items-center gap-1 transition cursor-pointer"
                          title="只抓这一页，不爬全站"
                        >
                          <Play className="w-3 h-3 fill-neutral-400" />
                          抓此页
                        </button>
                        <button
                          onClick={() => handleQuickAddMonitor(page.url)}
                          className="px-2 py-1 text-xs bg-neutral-800 hover:bg-neutral-700 text-neutral-300 hover:text-amber-300 rounded flex items-center gap-1 transition cursor-pointer"
                          title="一键将该页面加入定时监控"
                        >
                          <Activity className="w-3 h-3 text-neutral-400" />
                          监控
                        </button>
                        <button
                          onClick={() => openTimeMachine({ siteId: selectedSiteId, pageId: page.id })}
                          className="px-2.5 py-1 text-xs bg-indigo-600/20 hover:bg-indigo-600/30 text-indigo-300 rounded flex items-center gap-1 transition cursor-pointer font-medium"
                          title="在网页时光机中查看所有历史版本与回放"
                        >
                          <Clock className="w-3 h-3 text-indigo-400" />
                          时光机
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            ) : activeTab === 'credentials' ? (
              <div className="space-y-4 text-xs">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="text-xs font-semibold text-neutral-200">受控登录凭证</div>
                    <div className="text-xs text-neutral-500">
                      通过受控独立浏览器手动扫码或输入账号，捕获 Cookie 与 Storage 凭证，在自动化抓取时自动注入。
                    </div>
                  </div>
                  <button
                    onClick={() => {
                      setCredName('default');
                      setCredUrl(activeSite?.root_url || '');
                      setInteractivePort(null);
                      setIsRecordCredOpen(true);
                    }}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded text-xs font-medium transition shadow-xs"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    录制新凭证
                  </button>
                </div>

                {credentials.length === 0 ? (
                  <div className="p-8 text-center bg-neutral-900/40 rounded-lg border border-neutral-800 text-neutral-500">
                    当前站点尚未录制登录凭证。若该站点部分内容需要登录权限访问，请点击上方【录制新凭证】。
                  </div>
                ) : (
                  <div className="divide-y divide-neutral-800 bg-neutral-900/60 border border-neutral-800 rounded-lg overflow-hidden">
                    {credentials.map((cred) => (
                      <div key={cred.id} className="p-3 flex items-center justify-between">
                        <div className="space-y-1">
                          <div className="flex items-center gap-2">
                            <KeyRound className="w-3.5 h-3.5 text-indigo-400" />
                            <span className="font-semibold text-neutral-200">{cred.name}</span>
                            <span className="text-xs font-mono px-2 py-0.5 rounded bg-neutral-800 text-emerald-400">
                              有效凭证
                            </span>
                          </div>
                          <div className="text-xs text-neutral-500 font-mono">
                            更新于 {new Date(cred.updated_at).toLocaleString('zh-CN')}
                          </div>
                        </div>

                        <button
                          onClick={() => handleDeleteCredential(cred.id)}
                          className="p-1.5 hover:bg-rose-950/50 hover:text-rose-400 text-neutral-500 rounded transition"
                          title="删除凭证"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ) : activeTab === 'scripts' ? (
              <div className="space-y-4 text-xs">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="text-xs font-semibold text-neutral-200">用户自定义脚本 (User Scripts)</div>
                    <div className="text-xs text-neutral-500">
                      在页面导航与 DOM 解析之前注入执行，用于自动点击去弹窗、绕过遮罩、反反爬或清理广告。
                    </div>
                  </div>
                  <button
                    onClick={() => {
                      setScriptName('');
                      setScriptContent('');
                      setIsAddScriptOpen(true);
                    }}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded text-xs font-medium transition shadow-xs"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    新建脚本
                  </button>
                </div>

                {scripts.length === 0 ? (
                  <div className="p-8 text-center bg-neutral-900/40 rounded-lg border border-neutral-800 text-neutral-500">
                    当前站点尚未配置用户脚本。如果页面需要自动关闭弹窗、滚动注入等，可在此编写 JavaScript 脚本。
                  </div>
                ) : (
                  <div className="space-y-2.5">
                    {scripts.map((script) => (
                      <div
                        key={script.id}
                        className="p-3 bg-neutral-900/60 border border-neutral-800 rounded-lg space-y-2"
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <span className="font-semibold text-neutral-200">{script.name}</span>
                            <span className="text-xs font-mono px-2 py-0.5 rounded bg-neutral-800 text-indigo-400">
                              OnNewDocument
                            </span>
                          </div>
                          <button
                            onClick={() => handleDeleteScript(script.id)}
                            className="p-1 text-neutral-500 hover:text-rose-400 rounded transition"
                            title="删除脚本"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </div>
                        <pre className="p-2.5 bg-neutral-950 rounded border border-neutral-800 text-xs font-mono text-neutral-400 overflow-x-auto max-h-32">
                          {script.script_content}
                        </pre>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ) : activeTab === 'rss' ? (
              <div className="space-y-4 text-xs">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="text-xs font-semibold text-neutral-200">RSS / Atom 自动监控与订阅源</div>
                    <div className="text-xs text-neutral-500">
                      定时拉取 Feed 增量发现新发布的文章与快照，无需全站暴力爬取。
                    </div>
                  </div>
                  <button
                    onClick={() => {
                      setRssUrl('');
                      setRssTitle('');
                      setIsAddRssOpen(true);
                    }}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded text-xs font-medium transition shadow-xs"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    添加 RSS 源
                  </button>
                </div>

                {rssFeeds.length === 0 ? (
                  <div className="p-8 text-center bg-neutral-900/40 rounded-lg border border-neutral-800 text-neutral-500">
                    当前站点尚未绑定 RSS 订阅源。如果该站点提供 /feed 或 /rss.xml，可在此添加以便自动化同步更新。
                  </div>
                ) : (
                  <div className="space-y-2.5">
                    {rssFeeds.map((feed) => (
                      <div
                        key={feed.id}
                        className="p-3 bg-neutral-900/60 border border-neutral-800 rounded-lg flex items-center justify-between"
                      >
                        <div className="space-y-1">
                          <div className="flex items-center gap-2">
                            <span className="font-semibold text-neutral-200">
                              {feed.title || feed.feed_url}
                            </span>
                            {feed.last_synced ? (
                              <span className="text-xs font-mono text-neutral-500">
                                上次同步: {new Date(feed.last_synced).toLocaleTimeString('zh-CN')}
                              </span>
                            ) : null}
                          </div>
                          <div className="text-xs text-neutral-500 font-mono">{feed.feed_url}</div>
                        </div>

                        <div className="flex items-center gap-2">
                          <button
                            onClick={() => handleSyncRss(feed.id)}
                            disabled={syncingRssId === feed.id}
                            className="flex items-center gap-1 px-2.5 py-1 bg-neutral-800 hover:bg-neutral-700 disabled:opacity-50 text-neutral-200 rounded text-xs transition"
                          >
                            <RefreshCw
                              className={`w-3.5 h-3.5 text-neutral-400 ${
                                syncingRssId === feed.id ? 'animate-spin' : ''
                              }`}
                            />
                            {syncingRssId === feed.id ? '同步中...' : '立即同步'}
                          </button>
                          <button
                            onClick={() => handleDeleteRss(feed.id)}
                            className="p-1.5 text-neutral-500 hover:text-rose-400 rounded transition"
                            title="删除订阅"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ) : (
              profile && (
                <form onSubmit={handleSaveProfile} className="max-w-md space-y-3.5 text-xs">
                  <div>
                    <label className="block text-neutral-300 font-medium mb-1">范围 (Scope)</label>
                    <select
                      value={profile.scope_type}
                      onChange={(e) => setProfile({ ...profile, scope_type: e.target.value as any })}
                      className="w-full px-2.5 py-1.5 bg-neutral-900 border border-neutral-800 rounded text-neutral-200 text-xs focus:outline-hidden"
                    >
                      <option value="host">同主机 (Same Host)</option>
                      <option value="prefix">前缀匹配 (Prefix)</option>
                      <option value="domain">全域名 (Domain)</option>
                      <option value="current">仅当前页 (Current)</option>
                    </select>
                  </div>

                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="block text-neutral-300 font-medium mb-1">最大深度</label>
                      <input
                        type="number"
                        value={profile.max_depth}
                        onChange={(e) => setProfile({ ...profile, max_depth: parseInt(e.target.value) || 1 })}
                        className="w-full px-2.5 py-1 bg-neutral-900 border border-neutral-800 rounded text-neutral-200 text-xs"
                      />
                    </div>
                    <div>
                      <label className="block text-neutral-300 font-medium mb-1">页面上限</label>
                      <input
                        type="number"
                        value={profile.max_pages}
                        onChange={(e) => setProfile({ ...profile, max_pages: parseInt(e.target.value) || 10 })}
                        className="w-full px-2.5 py-1 bg-neutral-900 border border-neutral-800 rounded text-neutral-200 text-xs"
                      />
                    </div>
                  </div>

                  <div className="flex items-center justify-between p-2.5 bg-neutral-900 rounded border border-neutral-800">
                    <div>
                      <div className="text-neutral-200 font-medium">自动滚动加载懒加载资源</div>
                    </div>
                    <input
                      type="checkbox"
                      checked={profile.autoscroll}
                      onChange={(e) => setProfile({ ...profile, autoscroll: e.target.checked })}
                      className="accent-indigo-600"
                    />
                  </div>

                  <div className="pt-2">
                    <button
                      type="submit"
                      disabled={savingProfile}
                      className="flex items-center gap-1.5 px-3.5 py-1.5 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded font-medium transition shadow-xs"
                    >
                      {savedSuccess ? (
                        <>
                          <Check className="w-3.5 h-3.5 text-emerald-300" />
                          已保存设置
                        </>
                      ) : (
                        '保存设置'
                      )}
                    </button>
                  </div>
                </form>
              )
            )}
          </div>
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-neutral-500 text-xs">
          请选择一个站点查看
        </div>
      )}

      {/* Batch Import Modal */}
      {isBatchImportOpen && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center p-4 z-50">
          <div className="bg-neutral-900 border border-neutral-800 rounded-lg max-w-lg w-full p-4 space-y-3 text-xs">
            <div className="flex items-center justify-between">
              <div className="font-semibold text-sm text-neutral-100 flex items-center gap-1.5">
                <Upload className="w-4 h-4 text-indigo-400" />
                批量导入页面 URL
              </div>
              <button
                onClick={() => setIsBatchImportOpen(false)}
                className="text-neutral-400 hover:text-neutral-200"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <p className="text-neutral-400 text-xs">
              粘贴需要纳入归档的 URL 清单，每行输入一个链接（支持 http:// 与 https://）：
            </p>

            <textarea
              value={batchUrlsText}
              onChange={(e) => setBatchUrlsText(e.target.value)}
              placeholder="https://example.com/article/1&#10;https://example.com/article/2&#10;https://example.com/article/3"
              rows={8}
              className="w-full bg-neutral-950 border border-neutral-800 rounded p-2.5 font-mono text-neutral-200 text-xs focus:outline-hidden resize-none"
            />

            <div className="flex justify-end gap-2 pt-1">
              <button
                onClick={() => setIsBatchImportOpen(false)}
                className="px-3 py-1.5 rounded bg-neutral-800 hover:bg-neutral-700 text-neutral-300 transition text-xs"
              >
                取消
              </button>
              <button
                onClick={handleBatchImport}
                disabled={batchImporting || !batchUrlsText.trim()}
                className="px-4 py-1.5 rounded bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white font-medium transition text-xs"
              >
                {batchImporting ? '正在导入...' : '开始导入'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Record Credential Modal */}
      {isRecordCredOpen && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center p-4 z-50">
          <div className="bg-neutral-900 border border-neutral-800 rounded-lg max-w-md w-full p-4 space-y-3.5 text-xs">
            <div className="flex items-center justify-between">
              <div className="font-semibold text-sm text-neutral-100 flex items-center gap-1.5">
                <KeyRound className="w-4 h-4 text-indigo-400" />
                受控登录凭证录制
              </div>
              <button
                onClick={() => setIsRecordCredOpen(false)}
                className="text-neutral-400 hover:text-neutral-200"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="space-y-2.5">
              <div>
                <label className="block text-neutral-300 font-medium mb-1">凭证标识名称</label>
                <input
                  type="text"
                  value={credName}
                  onChange={(e) => setCredName(e.target.value)}
                  className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200 text-xs"
                  placeholder="例如：github_ryan 或 default"
                />
              </div>

              <div>
                <label className="block text-neutral-300 font-medium mb-1">登录入口 URL</label>
                <input
                  type="text"
                  value={credUrl}
                  onChange={(e) => setCredUrl(e.target.value)}
                  className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200 font-mono text-xs"
                  placeholder="https://..."
                />
              </div>

              <div className="p-2.5 bg-neutral-950 rounded border border-neutral-800 space-y-2">
                <div className="font-medium text-neutral-300 flex items-center justify-between">
                  <span>步骤 1：拉起受控浏览器</span>
                  <button
                    type="button"
                    onClick={handleLaunchBrowser}
                    disabled={launchingBrowser || interactivePort !== null}
                    className="px-2.5 py-1 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded transition text-xs"
                  >
                    {interactivePort !== null ? '浏览器运行中' : launchingBrowser ? '启动中...' : '启动浏览器'}
                  </button>
                </div>

                {interactivePort !== null ? (
                  <div className="text-xs text-emerald-400 space-y-1">
                    <div>✅ 受控浏览器已在端口 {interactivePort} 启动！</div>
                    <div className="text-neutral-400">
                      请在弹出的浏览器窗口中手动完成登录或扫码。登录完成后，点击下方按钮保存。
                    </div>
                  </div>
                ) : (
                  <div className="text-xs text-neutral-500">
                    点击后将打开一个独立的 Chromium 窗口供您输入账号或扫码。
                  </div>
                )}
              </div>
            </div>

            <div className="flex justify-end gap-2 pt-1">
              <button
                onClick={() => setIsRecordCredOpen(false)}
                className="px-3 py-1.5 rounded bg-neutral-800 hover:bg-neutral-700 text-neutral-300 transition"
              >
                取消
              </button>
              <button
                onClick={handleSaveCredential}
                disabled={interactivePort === null || savingCred}
                className="px-4 py-1.5 rounded bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-medium transition"
              >
                {savingCred ? '正在提取...' : '步骤 2：提取并保存凭证'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Add User Script Modal */}
      {isAddScriptOpen && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center p-4 z-50">
          <div className="bg-neutral-900 border border-neutral-800 rounded-lg max-w-lg w-full p-4 space-y-3 text-xs">
            <div className="flex items-center justify-between">
              <div className="font-semibold text-sm text-neutral-100 flex items-center gap-1.5">
                <Plus className="w-4 h-4 text-indigo-400" />
                新建站点用户脚本
              </div>
              <button
                onClick={() => setIsAddScriptOpen(false)}
                className="text-neutral-400 hover:text-neutral-200"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleSaveScript} className="space-y-3">
              <div>
                <label className="block text-neutral-300 font-medium mb-1">脚本名称</label>
                <input
                  type="text"
                  required
                  value={scriptName}
                  onChange={(e) => setScriptName(e.target.value)}
                  placeholder="例如：关闭Cookie弹窗 / 移除遮罩"
                  className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200"
                />
              </div>

              <div>
                <label className="block text-neutral-300 font-medium mb-1">
                  JavaScript 代码 (OnNewDocument)
                </label>
                <textarea
                  required
                  value={scriptContent}
                  onChange={(e) => setScriptContent(e.target.value)}
                  placeholder="// 在页面 DOM 加载前注入，例如：&#10;window.addEventListener('DOMContentLoaded', () => {&#10;  document.querySelector('.modal-mask')?.remove();&#10;});"
                  rows={8}
                  className="w-full bg-neutral-950 border border-neutral-800 rounded p-2.5 font-mono text-neutral-200 text-xs focus:outline-hidden resize-none"
                />
              </div>

              <div className="flex justify-end gap-2 pt-1">
                <button
                  type="button"
                  onClick={() => setIsAddScriptOpen(false)}
                  className="px-3 py-1.5 rounded bg-neutral-800 hover:bg-neutral-700 text-neutral-300 transition"
                >
                  取消
                </button>
                <button
                  type="submit"
                  disabled={savingScript || !scriptName.trim() || !scriptContent.trim()}
                  className="px-4 py-1.5 rounded bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white font-medium transition"
                >
                  {savingScript ? '保存中...' : '保存脚本'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Add RSS Feed Modal */}
      {isAddRssOpen && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center p-4 z-50">
          <div className="bg-neutral-900 border border-neutral-800 rounded-lg max-w-md w-full p-4 space-y-3 text-xs">
            <div className="flex items-center justify-between">
              <div className="font-semibold text-sm text-neutral-100 flex items-center gap-1.5">
                <Plus className="w-4 h-4 text-indigo-400" />
                添加 RSS / Atom 订阅源
              </div>
              <button
                onClick={() => setIsAddRssOpen(false)}
                className="text-neutral-400 hover:text-neutral-200"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleAddRss} className="space-y-3">
              <div>
                <label className="block text-neutral-300 font-medium mb-1">Feed 链接 URL</label>
                <input
                  type="url"
                  required
                  value={rssUrl}
                  onChange={(e) => setRssUrl(e.target.value)}
                  placeholder="https://example.com/feed.xml"
                  className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200 font-mono"
                />
              </div>

              <div>
                <label className="block text-neutral-300 font-medium mb-1">源标题备注 (可选)</label>
                <input
                  type="text"
                  value={rssTitle}
                  onChange={(e) => setRssTitle(e.target.value)}
                  placeholder="例如：博客更新源"
                  className="w-full px-2.5 py-1.5 bg-neutral-950 border border-neutral-800 rounded text-neutral-200"
                />
              </div>

              <div className="flex justify-end gap-2 pt-1">
                <button
                  type="button"
                  onClick={() => setIsAddRssOpen(false)}
                  className="px-3 py-1.5 rounded bg-neutral-800 hover:bg-neutral-700 text-neutral-300 transition"
                >
                  取消
                </button>
                <button
                  type="submit"
                  disabled={savingRss || !rssUrl.trim()}
                  className="px-4 py-1.5 rounded bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white font-medium transition"
                >
                  {savingRss ? '正在验证并添加...' : '确认添加'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
