use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub root_url: String,
    pub normalized_host: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_capture_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlProfile {
    pub id: String,
    pub site_id: String,
    pub scope_type: String, // current, prefix, host, domain, custom
    pub include_rules: Vec<String>,
    pub exclude_rules: Vec<String>,
    pub max_depth: i32,
    pub max_pages: i32,
    pub max_size_mb: i32,
    pub max_duration_minutes: i32,
    pub concurrency: i32,
    pub autoscroll: bool,
    pub autoplay: bool,
    pub autofetch: bool,
    pub popup_policy: String,
    pub iframe_policy: String,
    pub ad_policy: String,
    pub media_policy: String,
}

impl Default for CrawlProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            site_id: String::new(),
            scope_type: "host".to_string(),
            include_rules: Vec::new(),
            exclude_rules: Vec::new(),
            max_depth: 2,
            max_pages: 500,
            max_size_mb: 2048,
            max_duration_minutes: 60,
            concurrency: 2,
            autoscroll: true,
            autoplay: false,
            autofetch: true,
            popup_policy: "crawl_if_in_scope".to_string(),
            iframe_policy: "same_origin".to_string(),
            ad_policy: "preserve".to_string(),
            media_policy: "manifest_only".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlJob {
    pub id: String,
    pub site_id: String,
    pub profile_id: Option<String>,
    pub status: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub pages_discovered: i64,
    pub pages_captured: i64,
    pub resources_captured: i64,
    pub bytes_written: i64,
    pub error_count: i64,
    pub trigger_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageItem {
    pub id: String,
    pub site_id: String,
    pub url: String,
    pub normalized_url: String,
    pub title: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub capture_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureItem {
    pub id: String,
    pub page_id: String,
    pub job_id: Option<String>,
    pub captured_at: i64,
    pub status_code: i32,
    pub mime_type: Option<String>,
    pub warc_file: String,
    pub warc_offset: i64,
    pub warc_length: i64,
    pub screenshot_path: Option<String>,
    pub text_hash: Option<String>,
    pub dom_hash: Option<String>,
    pub visual_hash: Option<String>,
    pub resource_count: i64,
    pub missing_resource_count: i64,
    pub capture_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceItem {
    pub id: String,
    pub capture_id: String,
    pub url: String,
    pub normalized_url: String,
    pub mime_type: Option<String>,
    pub status_code: i32,
    pub size: i64,
    pub sha256: Option<String>,
    pub resource_type: String,
    #[serde(default)]
    pub warc_file: Option<String>,
    #[serde(default)]
    pub warc_offset: Option<i64>,
    #[serde(default)]
    pub warc_length: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorRule {
    #[serde(default)]
    pub id: String,
    pub site_id: String,
    pub url: String,
    pub schedule: String,
    pub strategy: String,
    pub selector: Option<String>,
    pub keyword: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub last_checked: Option<i64>,
    pub next_check: Option<i64>,
    pub last_status: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEventItem {
    pub id: String,
    pub page_id: String,
    pub url: String,
    pub title: String,
    pub old_capture_id: Option<String>,
    pub new_capture_id: String,
    pub old_time: i64,
    pub new_time: i64,
    pub text_changed: bool,
    pub dom_changed: bool,
    pub visual_changed: bool,
    pub resource_changed: bool,
    pub change_score: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaybackCaptureItem {
    pub id: String,
    pub url: String,
    pub timestamp: String,
    pub status_code: String,
    pub mime_type: String,
    pub digest: String,
    pub length: String,
    pub imported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub page_id: String,
    pub capture_id: String,
    pub site_id: String,
    pub site_name: String,
    pub url: String,
    pub title: String,
    pub captured_at: i64,
    pub snippet: String,
    pub score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDiffLine {
    pub r#type: String, // "equal", "add", "remove"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDiffSection {
    pub added_lines: i64,
    pub removed_lines: i64,
    pub lines: Vec<TextDiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedResourceItem {
    pub old: ResourceItem,
    pub new: ResourceItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDiffSection {
    pub added: Vec<ResourceItem>,
    pub removed: Vec<ResourceItem>,
    pub modified: Vec<ModifiedResourceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualDiffResult {
    pub diff_ratio: f64,
    pub changed_pixels: usize,
    pub total_pixels: usize,
    pub diff_image_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomTagCount {
    pub tag: String,
    pub old_count: usize,
    pub new_count: usize,
    pub diff: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomDiffResult {
    pub total_tags_old: usize,
    pub total_tags_new: usize,
    pub tag_changes: Vec<DomTagCount>,
    pub structure_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub text_diff: TextDiffSection,
    pub resource_diff: ResourceDiffSection,
    pub visual_diff: Option<VisualDiffResult>,
    pub dom_diff: Option<DomDiffResult>,
    pub change_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteCredential {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub cookies_json: String,
    pub storage_json: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub site_count: i64,
    pub page_count: i64,
    pub capture_count: i64,
    pub resource_count: i64,
    pub storage_bytes: i64,
    pub active_jobs: i64,
    pub storage_path: String,
    pub browser_path: String,
    pub browser_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagItem {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkItem {
    pub id: String,
    pub capture_id: String,
    pub note: String,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserScriptItem {
    pub id: String,
    pub site_id: Option<String>,
    pub name: String,
    pub script_content: String,
    pub enabled: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssFeedItem {
    pub id: String,
    pub site_id: String,
    pub feed_url: String,
    pub title: Option<String>,
    pub last_synced: Option<i64>,
}

