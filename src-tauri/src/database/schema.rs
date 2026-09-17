pub const CREATE_TABLES_SQL: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS sites (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_url TEXT NOT NULL,
    normalized_host TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    enabled INTEGER DEFAULT 1
);

CREATE TABLE IF NOT EXISTS crawl_profiles (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    scope_type TEXT NOT NULL DEFAULT 'host',
    include_rules TEXT DEFAULT '[]',
    exclude_rules TEXT DEFAULT '[]',
    max_depth INTEGER DEFAULT 2,
    max_pages INTEGER DEFAULT 500,
    max_size_mb INTEGER DEFAULT 2048,
    max_duration_minutes INTEGER DEFAULT 60,
    concurrency INTEGER DEFAULT 2,
    autoscroll INTEGER DEFAULT 1,
    autoplay INTEGER DEFAULT 0,
    autofetch INTEGER DEFAULT 1,
    popup_policy TEXT DEFAULT 'crawl_if_in_scope',
    iframe_policy TEXT DEFAULT 'same_origin',
    ad_policy TEXT DEFAULT 'preserve',
    media_policy TEXT DEFAULT 'manifest_only',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS crawl_jobs (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    profile_id TEXT,
    status TEXT NOT NULL DEFAULT 'queued',
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    pages_discovered INTEGER DEFAULT 0,
    pages_captured INTEGER DEFAULT 0,
    resources_captured INTEGER DEFAULT 0,
    bytes_written INTEGER DEFAULT 0,
    error_count INTEGER DEFAULT 0,
    trigger_type TEXT DEFAULT 'manual'
);

CREATE TABLE IF NOT EXISTS crawl_queue (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES crawl_jobs(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    normalized_url TEXT NOT NULL,
    parent_url TEXT,
    depth INTEGER DEFAULT 0,
    priority INTEGER DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending',
    retry_count INTEGER DEFAULT 0,
    discovered_at INTEGER NOT NULL,
    started_at INTEGER,
    finished_at INTEGER,
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS pages (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    normalized_url TEXT NOT NULL,
    title TEXT,
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    capture_count INTEGER DEFAULT 0,
    UNIQUE(site_id, normalized_url)
);

CREATE TABLE IF NOT EXISTS captures (
    id TEXT PRIMARY KEY,
    page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    job_id TEXT,
    captured_at INTEGER NOT NULL,
    status_code INTEGER NOT NULL,
    mime_type TEXT,
    warc_file TEXT NOT NULL,
    warc_offset INTEGER DEFAULT 0,
    warc_length INTEGER DEFAULT 0,
    screenshot_path TEXT,
    text_hash TEXT,
    dom_hash TEXT,
    visual_hash TEXT,
    resource_count INTEGER DEFAULT 0,
    missing_resource_count INTEGER DEFAULT 0,
    capture_score REAL DEFAULT 100.0
);

CREATE TABLE IF NOT EXISTS resources (
    id TEXT PRIMARY KEY,
    capture_id TEXT NOT NULL REFERENCES captures(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    normalized_url TEXT NOT NULL,
    mime_type TEXT,
    status_code INTEGER NOT NULL,
    size INTEGER DEFAULT 0,
    sha256 TEXT,
    warc_file TEXT,
    warc_offset INTEGER DEFAULT 0,
    warc_length INTEGER DEFAULT 0,
    resource_type TEXT DEFAULT 'other'
);

CREATE TABLE IF NOT EXISTS links (
    source_page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    target_url TEXT NOT NULL,
    relation TEXT DEFAULT 'anchor',
    discovered_at INTEGER NOT NULL,
    PRIMARY KEY (source_page_id, target_url)
);

CREATE TABLE IF NOT EXISTS monitor_rules (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    schedule TEXT NOT NULL DEFAULT '1h',
    strategy TEXT NOT NULL DEFAULT 'content_hash',
    selector TEXT,
    keyword TEXT,
    enabled INTEGER DEFAULT 1,
    last_checked INTEGER,
    next_check INTEGER,
    last_status TEXT
);

CREATE TABLE IF NOT EXISTS change_events (
    id TEXT PRIMARY KEY,
    page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    old_capture_id TEXT,
    new_capture_id TEXT,
    text_changed INTEGER DEFAULT 0,
    dom_changed INTEGER DEFAULT 0,
    visual_changed INTEGER DEFAULT 0,
    resource_changed INTEGER DEFAULT 0,
    change_score REAL DEFAULT 0.0,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS wayback_captures (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    status_code TEXT,
    mime_type TEXT,
    digest TEXT,
    length TEXT,
    source TEXT DEFAULT 'internet_archive',
    imported INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS site_credentials (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name TEXT NOT NULL DEFAULT 'default',
    cookies_json TEXT NOT NULL DEFAULT '[]',
    storage_json TEXT NOT NULL DEFAULT '{}',
    updated_at INTEGER NOT NULL,
    UNIQUE(site_id, name)
);

CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL DEFAULT '#6366f1'
);

CREATE TABLE IF NOT EXISTS page_tags (
    page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY(page_id, tag_id)
);

CREATE TABLE IF NOT EXISTS bookmarks (
    id TEXT PRIMARY KEY,
    capture_id TEXT NOT NULL REFERENCES captures(id) ON DELETE CASCADE,
    note TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    UNIQUE(capture_id)
);

CREATE TABLE IF NOT EXISTS user_scripts (
    id TEXT PRIMARY KEY,
    site_id TEXT REFERENCES sites(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    script_content TEXT NOT NULL,
    enabled INTEGER DEFAULT 1,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS rss_feeds (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    feed_url TEXT NOT NULL,
    title TEXT,
    last_synced INTEGER,
    UNIQUE(site_id, feed_url)
);

CREATE VIRTUAL TABLE IF NOT EXISTS fts_pages USING fts5(
    page_id UNINDEXED,
    capture_id UNINDEXED,
    url,
    title,
    clean_text,
    meta_desc,
    tokenize = 'unicode61'
);

CREATE INDEX IF NOT EXISTS idx_pages_site ON pages(site_id);
CREATE INDEX IF NOT EXISTS idx_pages_norm ON pages(normalized_url);
CREATE INDEX IF NOT EXISTS idx_captures_page ON captures(page_id);
CREATE INDEX IF NOT EXISTS idx_captures_time ON captures(captured_at);
CREATE INDEX IF NOT EXISTS idx_resources_capture ON resources(capture_id);
CREATE INDEX IF NOT EXISTS idx_queue_job ON crawl_queue(job_id, status);
CREATE INDEX IF NOT EXISTS idx_monitor_next ON monitor_rules(enabled, next_check);
CREATE INDEX IF NOT EXISTS idx_page_tags_page ON page_tags(page_id);
CREATE INDEX IF NOT EXISTS idx_page_tags_tag ON page_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_bookmarks_cap ON bookmarks(capture_id);
"#;
