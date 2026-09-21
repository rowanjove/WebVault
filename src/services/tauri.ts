import { invoke } from '@tauri-apps/api/core';
import type {
  Site,
  CrawlProfile,
  CrawlJob,
  PageItem,
  CaptureItem,
  ResourceItem,
  MonitorRule,
  ChangeEventItem,
  WaybackCaptureItem,
  SearchResultItem,
  DiffResult,
  SystemStats,
  SiteCredential,
  TagItem,
  BookmarkItem,
  UserScriptItem,
  RssFeedItem,
} from '../types';

export const isTauri = () => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
};

export const api = {
  // System
  getSystemStats: async (): Promise<SystemStats> => {
    return invoke('get_system_stats');
  },
  
  // Sites
  listSites: async (): Promise<Site[]> => {
    return invoke('list_sites');
  },
  createSite: async (params: { name: string; rootUrl?: string; root_url?: string }): Promise<Site> => {
    const rootUrl = params.rootUrl ?? params.root_url ?? '';
    return invoke('create_site', { name: params.name, rootUrl });
  },
  deleteSite: async (id: string): Promise<void> => {
    return invoke('delete_site', { id });
  },
  getSiteProfile: async (siteId: string): Promise<CrawlProfile> => {
    return invoke('get_site_profile', { siteId });
  },
  updateSiteProfile: async (profile: CrawlProfile): Promise<void> => {
    return invoke('update_site_profile', { profile });
  },

  // Pages & Captures
  listPages: async (siteId: string): Promise<PageItem[]> => {
    return invoke('list_pages', { siteId });
  },
  listCaptures: async (pageId: string): Promise<CaptureItem[]> => {
    return invoke('list_captures', { pageId });
  },
  getCaptureDetails: async (captureId: string): Promise<{
    capture: CaptureItem;
    resources: ResourceItem[];
    rendered_text: string;
  }> => {
    return invoke('get_capture_details', { captureId });
  },

  // Capture Execution (Single Page / Crawl)
  startSingleCapture: async (params: { siteId: string; url: string }): Promise<string> => {
    return invoke('start_single_capture', params);
  },
  startCrawlJob: async (params: { siteId: string }): Promise<string> => {
    return invoke('start_crawl_job', params);
  },
  listJobs: async (): Promise<CrawlJob[]> => {
    return invoke('list_jobs');
  },
  cancelJob: async (jobId: string): Promise<void> => {
    return invoke('cancel_job', { jobId });
  },

  // Replay
  getReplayUrl: async (captureId: string): Promise<string> => {
    return invoke('get_replay_url', { captureId });
  },

  // Search
  search: async (params: { query: string; siteId?: string }): Promise<SearchResultItem[]> => {
    return invoke('search_archive', params);
  },

  // Diff
  compareCaptures: async (params: { oldCaptureId: string; newCaptureId: string }): Promise<DiffResult> => {
    return invoke('compare_captures', params);
  },

  // Monitor
  listMonitorRules: async (): Promise<MonitorRule[]> => {
    return invoke('list_monitor_rules');
  },
  createMonitorRule: async (rule: Partial<MonitorRule>): Promise<MonitorRule> => {
    return invoke('create_monitor_rule', { rule });
  },
  toggleMonitorRule: async (id: string, enabled: boolean): Promise<void> => {
    return invoke('toggle_monitor_rule', { id, enabled });
  },
  deleteMonitorRule: async (id: string): Promise<void> => {
    return invoke('delete_monitor_rule', { id });
  },
  listChangeEvents: async (): Promise<ChangeEventItem[]> => {
    return invoke('list_change_events');
  },
  triggerMonitorCheck: async (): Promise<void> => {
    return invoke('trigger_monitor_check');
  },

  // Wayback
  queryWayback: async (url: string): Promise<WaybackCaptureItem[]> => {
    return invoke('query_wayback', { url });
  },
  importWaybackCapture: async (params: { siteId: string; capture: WaybackCaptureItem }): Promise<void> => {
    return invoke('import_wayback_capture', params);
  },

  // Export / Import
  exportWacz: async (params: { siteId: string; outputPath: string }): Promise<string> => {
    return invoke('export_wacz', params);
  },
  importWacz: async (filePath: string): Promise<Site> => {
    return invoke('import_wacz', { filePath });
  },
  exportPageOffline: async (params: { captureId: string; format: string; outputPath: string }): Promise<string> => {
    return invoke('export_page_offline', params);
  },

  // Credentials & Login
  launchInteractiveLogin: async (params: { siteId: string; loginUrl: string }): Promise<number> => {
    return invoke('launch_interactive_login', params);
  },
  saveSiteCredential: async (params: { siteId: string; port: number; name: string }): Promise<SiteCredential> => {
    return invoke('save_site_credential', params);
  },
  getSiteCredentials: async (siteId: string): Promise<SiteCredential[]> => {
    return invoke('get_site_credentials', { siteId });
  },
  deleteSiteCredential: async (credentialId: string): Promise<void> => {
    return invoke('delete_site_credential', { credentialId });
  },

  // Batch URLs
  importBatchUrls: async (params: { siteId: string; urls: string[] }): Promise<number> => {
    return invoke('import_batch_urls', params);
  },

  // Tags
  createTag: async (params: { name: string; color: string }): Promise<TagItem> => {
    return invoke('create_tag', params);
  },
  listTags: async (): Promise<TagItem[]> => {
    return invoke('list_tags');
  },
  deleteTag: async (tagId: string): Promise<void> => {
    return invoke('delete_tag', { tagId });
  },
  addPageTag: async (params: { pageId: string; tagId: string }): Promise<void> => {
    return invoke('add_page_tag', params);
  },
  removePageTag: async (params: { pageId: string; tagId: string }): Promise<void> => {
    return invoke('remove_page_tag', params);
  },
  getPageTags: async (pageId: string): Promise<TagItem[]> => {
    return invoke('get_page_tags', { pageId });
  },

  // Bookmarks
  createBookmark: async (params: { captureId: string; note: string }): Promise<BookmarkItem> => {
    return invoke('create_bookmark', params);
  },
  listBookmarks: async (): Promise<BookmarkItem[]> => {
    return invoke('list_bookmarks');
  },
  deleteBookmark: async (bookmarkId: string): Promise<void> => {
    return invoke('delete_bookmark', { bookmarkId });
  },

  // User Scripts
  saveUserScript: async (params: {
    siteId?: string | null;
    name: string;
    scriptContent: string;
    enabled: boolean;
  }): Promise<UserScriptItem> => {
    return invoke('save_user_script', params);
  },
  listUserScripts: async (siteId?: string | null): Promise<UserScriptItem[]> => {
    return invoke('list_user_scripts', { siteId });
  },
  deleteUserScript: async (scriptId: string): Promise<void> => {
    return invoke('delete_user_script', { scriptId });
  },

  // RSS Feeds
  addRssFeed: async (params: { siteId: string; feedUrl: string; title?: string }): Promise<RssFeedItem> => {
    return invoke('add_rss_feed', params);
  },
  listRssFeeds: async (siteId: string): Promise<RssFeedItem[]> => {
    return invoke('list_rss_feeds', { siteId });
  },
  deleteRssFeed: async (feedId: string): Promise<void> => {
    return invoke('delete_rss_feed', { feedId });
  },
  syncRssFeed: async (feedId: string): Promise<number> => {
    return invoke('sync_rss_feed', { feedId });
  },
  openFolder: async (path: string): Promise<void> => {
    return invoke('open_folder', { path });
  },
  exportBackup: async (outputPath: string): Promise<string> => {
    return invoke('export_backup', { outputPath });
  },
  importBackup: async (filePath: string): Promise<void> => {
    return invoke('import_backup', { filePath });
  },
};

export async function pickOpenFile(filters: { name: string; extensions: string[] }[]): Promise<string | null> {
  if (!isTauri()) {
    return window.prompt('请输入文件路径') || null;
  }
  const { open } = await import('@tauri-apps/plugin-dialog');
  const selected = await open({ multiple: false, filters });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export async function pickSaveFile(opts: {
  defaultPath: string;
  filters: { name: string; extensions: string[] }[];
}): Promise<string | null> {
  if (!isTauri()) {
    return window.prompt('请输入保存路径', opts.defaultPath) || null;
  }
  const { save } = await import('@tauri-apps/plugin-dialog');
  return save(opts);
}
