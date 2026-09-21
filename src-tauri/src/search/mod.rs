use crate::database::models::SearchResultItem;
use crate::database::Database;
use anyhow::Result;

pub struct SearchEngine;

impl SearchEngine {
    pub fn index_page(
        db: &Database,
        page_id: &str,
        capture_id: &str,
        url: &str,
        title: &str,
        clean_text: &str,
        meta_desc: &str,
    ) -> Result<()> {
        let conn = db.lock_conn()?;
        conn.execute(
            r#"
            INSERT INTO fts_pages (page_id, capture_id, url, title, clean_text, meta_desc)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            rusqlite::params![
                page_id,
                capture_id,
                url,
                cjk_space_separate(title),
                cjk_space_separate(clean_text),
                cjk_space_separate(meta_desc)
            ],
        )?;
        Ok(())
    }

    pub fn search(db: &Database, query: &str, site_id: Option<&str>) -> Result<Vec<SearchResultItem>> {
        let conn = db.lock_conn()?;

        // Clean user query and extract site: prefix if present
        let mut fts_terms = Vec::new();
        let mut site_filter = site_id.map(|s| s.to_string());

        for word in query.split_whitespace() {
            if let Some(site) = word.strip_prefix("site:") {
                site_filter = Some(site.to_string());
            } else if let Some(title_term) = word.strip_prefix("title:") {
                let clean = title_term.replace('"', "").replace('*', "");
                if !clean.is_empty() {
                    fts_terms.push(format!("title:{}", quote_fts_term(&clean)));
                }
            } else if !word.is_empty() {
                let clean = word.replace('"', "").replace('*', "");
                if !clean.is_empty() {
                    fts_terms.push(quote_fts_term(&clean));
                }
            }
        }

        if fts_terms.is_empty() {
            return Ok(Vec::new());
        }

        let match_query = fts_terms.join(" AND ");

        let mut sql = String::from(
            r#"
            SELECT 
                f.page_id, f.capture_id, p.site_id, s.name as site_name,
                f.url, COALESCE(p.title, f.title), c.captured_at,
                snippet(fts_pages, 4, char(1), char(2), '...', 25) as snippet,
                bm25(fts_pages) as rank
            FROM fts_pages f
            JOIN captures c ON f.capture_id = c.id
            JOIN pages p ON f.page_id = p.id
            JOIN sites s ON p.site_id = s.id
            WHERE fts_pages MATCH ?1
            "#,
        );

        if site_filter.is_some() {
            sql.push_str(" AND (p.site_id = ?2 OR s.normalized_host LIKE '%' || ?2 || '%') ");
        }

        sql.push_str(" ORDER BY rank LIMIT 50");

        let mut results = Vec::new();
        if let Some(ref site) = site_filter {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params![match_query, site], |row| {
                Ok(SearchResultItem {
                    page_id: row.get(0)?,
                    capture_id: row.get(1)?,
                    site_id: row.get(2)?,
                    site_name: row.get(3)?,
                    url: row.get(4)?,
                    title: row.get(5)?,
                    captured_at: row.get(6)?,
                    snippet: row.get(7)?,
                    score: Some(row.get(8)?),
                })
            })?;
            for r in rows {
                let mut item = r?;
                item.snippet = compact_cjk_display(&sanitize_snippet(&item.snippet));
                results.push(item);
            }
        } else {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params![match_query], |row| {
                Ok(SearchResultItem {
                    page_id: row.get(0)?,
                    capture_id: row.get(1)?,
                    site_id: row.get(2)?,
                    site_name: row.get(3)?,
                    url: row.get(4)?,
                    title: row.get(5)?,
                    captured_at: row.get(6)?,
                    snippet: row.get(7)?,
                    score: Some(row.get(8)?),
                })
            })?;
            for r in rows {
                let mut item = r?;
                item.snippet = compact_cjk_display(&sanitize_snippet(&item.snippet));
                results.push(item);
            }
        }
        Ok(results)
    }
}

pub fn sanitize_snippet(raw: &str) -> String {
    let escaped = html_escape(raw);
    escaped.replace('\u{0001}', "<mark>").replace('\u{0002}', "</mark>")
}

pub fn cjk_space_separate(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_cjk = false;
    for c in s.chars() {
        let is_cjk = is_cjk_char(c);
        if is_cjk {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push(c);
            out.push(' ');
            prev_cjk = true;
        } else {
            if prev_cjk && !c.is_whitespace() && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push(c);
            prev_cjk = false;
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_cjk_char(c: char) -> bool {
    matches!(
        c,
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{2A6DF}'
    )
}

fn quote_fts_term(term: &str) -> String {
    let spaced = cjk_space_separate(term);
    format!("\"{}\"", spaced.replace('"', ""))
}

pub fn compact_cjk_display(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for i in 0..chars.len() {
        if chars[i] == ' ' && i > 0 && i + 1 < chars.len() {
            let prev = chars[i - 1];
            let next = chars[i + 1];
            if is_cjk_char(prev) && is_cjk_char(next) {
                continue;
            }
        }
        out.push(chars[i]);
    }
    out
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_engine_indexing_and_title_filter() {
        let temp_dir = std::env::temp_dir().join(format!("webvault_search_test_{}", uuid::Uuid::new_v4().simple()));
        let db = Database::init(&temp_dir).expect("Init test db");

        let site = db.create_site("Rust Tech", "https://rust-lang.org").unwrap();
        let page_id = "page_1";
        let cap_id = "cap_1";

        // Insert mock page and capture
        {
            let conn = db.lock_conn().unwrap();
            conn.execute(
                "INSERT INTO pages (id, site_id, url, normalized_url, title, first_seen, last_seen) VALUES (?1, ?2, ?3, ?3, ?4, 100, 100)",
                rusqlite::params![page_id, site.id, "https://rust-lang.org/learn", "Learn Rust Fast"],
            ).unwrap();
            conn.execute(
                "INSERT INTO captures (id, page_id, job_id, captured_at, status_code, warc_file) VALUES (?1, ?2, 'job1', 100, 200, 'data.warc')",
                rusqlite::params![cap_id, page_id],
            ).unwrap();
        }

        SearchEngine::index_page(
            &db,
            page_id,
            cap_id,
            "https://rust-lang.org/learn",
            "Learn Rust Fast",
            "Rust is a systems programming language that is extremely fast and memory-safe.",
            "description",
        ).unwrap();

        // 1. Plain search
        let res1 = SearchEngine::search(&db, "memory-safe", None).unwrap();
        assert_eq!(res1.len(), 1);
        assert_eq!(res1[0].title, "Learn Rust Fast");

        // 2. title: prefix search
        let res2 = SearchEngine::search(&db, "title:Fast", None).unwrap();
        assert_eq!(res2.len(), 1);

        SearchEngine::index_page(
            &db,
            page_id,
            cap_id,
            "https://rust-lang.org/learn",
            "中文标题",
            "今天天气很好而且适合出门归档网页",
            "",
        ).unwrap();
        let zh = SearchEngine::search(&db, "天气", None).unwrap();
        assert!(!zh.is_empty(), "CJK character indexing must match Chinese substrings");

        assert_eq!(
            sanitize_snippet(&format!("hello\u{0001}<img src=x>\u{0002}world")),
            "hello<mark>&lt;img src=x&gt;</mark>world"
        );
        assert_eq!(compact_cjk_display("哔 哩 哔 哩 ~-bilibili"), "哔哩哔哩 ~-bilibili");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
