export type ScopeType = 'current' | 'prefix' | 'host' | 'domain' | 'custom';

export type JobStatus = 'queued' | 'running' | 'paused' | 'completed' | 'cancelled' | 'failed';

export interface Site {
  id: string;
  name: string;
  root_url: string;
  normalized_host: string;
  created_at: number;
  updated_at: number;
  enabled: boolean;
  page_count?: number;
  capture_count?: number;
  storage_bytes?: number;
  last_capture_at?: number | null;
}

export interface CrawlProfile {
  id: string;
  site_id: string;
  scope_type: ScopeType;
  include_rules: string[];
  exclude_rules: string[];
  max_depth: number;
  max_pages: number;
  max_size_mb: number;
  max_duration_minutes: number;
  concurrency: number;
  autoscroll: boolean;
  autoplay: boolean;
  autofetch: boolean;
  popup_policy: 'block' | 'record' | 'crawl_if_in_scope';
  iframe_policy: 'same_origin' | 'all' | 'none';
  ad_policy: 'preserve' | 'block';
  media_policy: 'manifest_only' | 'limited' | 'full';
}

export interface CrawlJob {
  id: string;
  site_id: string;
  profile_id?: string;
  status: JobStatus;
  started_at: number;
  finished_at?: number | null;
  pages_discovered: number;
  pages_captured: number;
  resources_captured: number;
  bytes_written: number;
  error_count: number;
  trigger_type: 'manual' | 'monitor' | 'wayback';
  current_url?: string;
  logs?: string[];
}

export interface PageItem {
  id: string;
  site_id: string;
  url: string;
  normalized_url: string;
  title: string;
  first_seen: number;
  last_seen: number;
  capture_count: number;
  latest_capture?: CaptureItem;
}

export interface CaptureItem {
  id: string;
  page_id: string;
  job_id?: string;
  captured_at: number;
  status_code: number;
  mime_type: string;
  warc_file: string;
  warc_offset: number;
  warc_length: number;
  screenshot_path?: string;
  text_hash?: string;
  dom_hash?: string;
  visual_hash?: string;
  resource_count: number;
  missing_resource_count: number;
  capture_score: number;
  url?: string;
  title?: string;
}

export interface ResourceItem {
  id: string;
  capture_id: string;
  url: string;
  normalized_url: string;
  mime_type: string;
  status_code: number;
  size: number;
  sha256: string;
  resource_type: string;
}

export interface MonitorRule {
  id: string;
  site_id: string;
  url: string;
  schedule: string; // e.g., '15m', '1h', '24h'
  strategy: 'conditional_get' | 'content_hash' | 'selector' | 'keyword';
  selector?: string;
  keyword?: string;
  enabled: boolean;
  last_checked?: number | null;
  next_check?: number | null;
  last_status?: string;
}

export interface ChangeEventItem {
  id: string;
  page_id: string;
  site_id: string;
  url: string;
  title: string;
  old_capture_id: string;
  new_capture_id: string;
  old_time: number;
  new_time: number;
  text_changed: boolean;
  dom_changed: boolean;
  visual_changed: boolean;
  resource_changed: boolean;
  change_score: number;
  created_at: number;
}

export interface WaybackCaptureItem {
  id: string;
  url: string;
  timestamp: string;
  status_code: string;
  mime_type: string;
  digest: string;
  length: string;
  imported: boolean;
}

export interface SearchResultItem {
  page_id: string;
  capture_id: string;
  site_id: string;
  site_name: string;
  url: string;
  title: string;
  captured_at: number;
  snippet: string;
  score?: number;
}

export interface DomTagCount {
  tag: string;
  old_count: number;
  new_count: number;
  diff: number;
}

export interface DomDiffResult {
  total_tags_old: number;
  total_tags_new: number;
  tag_changes: DomTagCount[];
  structure_score: number;
}

export interface DiffResult {
  text_diff: {
    added_lines: number;
    removed_lines: number;
    lines: Array<{
      type: 'equal' | 'add' | 'remove';
      content: string;
    }>;
  };
  resource_diff: {
    added: ResourceItem[];
    removed: ResourceItem[];
    modified: Array<{ old: ResourceItem; new: ResourceItem }>;
  };
  dom_diff?: DomDiffResult;
  visual_diff?: {
    diff_ratio: number;
    changed_pixels: number;
    total_pixels: number;
    diff_image_base64?: string;
  };
  change_score: number;
}

export interface SiteCredential {
  id: string;
  site_id: string;
  name: string;
  cookies_json: string;
  storage_json: string;
  updated_at: number;
}

export interface SystemStats {
  site_count: number;
  page_count: number;
  capture_count: number;
  resource_count: number;
  storage_bytes: number;
  active_jobs: number;
  storage_path: string;
  browser_path: string;
  browser_detected: boolean;
}

export interface TagItem {
  id: string;
  name: string;
  color: string;
}

export interface BookmarkItem {
  id: string;
  capture_id: string;
  note: string;
  created_at: number;
  url?: string;
  title?: string;
}

export interface UserScriptItem {
  id: string;
  site_id?: string | null;
  name: string;
  script_content: string;
  enabled: boolean;
  created_at: number;
}

export interface RssFeedItem {
  id: string;
  site_id: string;
  feed_url: string;
  title?: string | null;
  last_synced?: number | null;
}
