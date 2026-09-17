use crate::archive::warc::WarcReader;
use crate::database::models::{DiffResult, DomDiffResult, DomTagCount, ResourceItem};
use crate::database::Database;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::collections::{HashMap, HashSet};

pub struct DiffEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDiffLine {
    pub r#type: String, // "equal", "add", "remove"
    pub content: String,
}

impl DiffEngine {
    pub fn compare(
        db: &Database,
        old_capture_id: &str,
        new_capture_id: &str,
    ) -> Result<DiffResult> {
        let (_old_cap, old_res, old_text) = db.get_capture_details(old_capture_id)?;
        let (_new_cap, new_res, new_text) = db.get_capture_details(new_capture_id)?;

        // Text diff
        let diff = TextDiff::from_lines(&old_text, &new_text);
        let mut diff_lines = Vec::new();
        let mut added_lines = 0;
        let mut removed_lines = 0;
        let mut total_lines = 0;

        for change in diff.iter_all_changes() {
            total_lines += 1;
            match change.tag() {
                ChangeTag::Equal => {
                    diff_lines.push(crate::database::models::TextDiffLine {
                        r#type: "equal".to_string(),
                        content: change.value().trim_end_matches(['\r', '\n']).to_string(),
                    });
                }
                ChangeTag::Delete => {
                    removed_lines += 1;
                    diff_lines.push(crate::database::models::TextDiffLine {
                        r#type: "remove".to_string(),
                        content: change.value().trim_end_matches(['\r', '\n']).to_string(),
                    });
                }
                ChangeTag::Insert => {
                    added_lines += 1;
                    diff_lines.push(crate::database::models::TextDiffLine {
                        r#type: "add".to_string(),
                        content: change.value().trim_end_matches(['\r', '\n']).to_string(),
                    });
                }
            }
        }

        let text_change_ratio = if total_lines > 0 {
            ((added_lines + removed_lines) as f64 / total_lines as f64).min(1.0)
        } else {
            0.0
        };

        // Resource diff
        let mut old_res_map = HashMap::new();
        for r in old_res {
            old_res_map.insert(r.url.clone(), r);
        }

        let mut added_res = Vec::new();
        let mut modified_res = Vec::new();
        let mut remaining_old_urls: HashSet<String> = old_res_map.keys().cloned().collect();

        for r in new_res {
            if let Some(old_r) = old_res_map.get(&r.url) {
                remaining_old_urls.remove(&r.url);
                if old_r.sha256 != r.sha256 || old_r.status_code != r.status_code {
                    modified_res.push(crate::database::models::ModifiedResourceItem {
                        old: old_r.clone(),
                        new: r,
                    });
                }
            } else {
                added_res.push(r);
            }
        }

        let removed_res: Vec<ResourceItem> = remaining_old_urls
            .into_iter()
            .filter_map(|u| old_res_map.remove(&u))
            .collect();

        // Visual diff
        let visual_diff = Self::compute_visual_diff(db, &_old_cap, &_new_cap);

        // DOM diff
        let dom_diff = Self::compute_dom_diff(db, &_old_cap, &_new_cap);

        // Change score (0 - 100)
        let res_change_count = added_res.len() + removed_res.len() + modified_res.len();
        let res_change_score = (res_change_count as f64 * 3.0).min(30.0);
        let text_change_score = (text_change_ratio * 30.0).min(30.0);
        let visual_score = visual_diff.as_ref().map(|v| (v.diff_ratio * 20.0).min(20.0)).unwrap_or(0.0);
        let dom_score = dom_diff.as_ref().map(|d| ((100.0 - d.structure_score) * 0.2).min(20.0)).unwrap_or(0.0);
        let change_score = (res_change_score + text_change_score + visual_score + dom_score).round().min(100.0);

        Ok(DiffResult {
            text_diff: crate::database::models::TextDiffSection {
                added_lines,
                removed_lines,
                lines: diff_lines,
            },
            resource_diff: crate::database::models::ResourceDiffSection {
                added: added_res,
                removed: removed_res,
                modified: modified_res,
            },
            visual_diff,
            dom_diff,
            change_score,
        })
    }

    fn extract_html_from_capture(db: &Database, cap: &crate::database::models::CaptureItem) -> Option<String> {
        if cap.warc_file.is_empty() {
            return None;
        }
        let warc_path = db.base_dir.join(&cap.warc_file);
        if !warc_path.exists() {
            return None;
        }
        let record = WarcReader::read_record_at(&warc_path, cap.warc_offset as u64, cap.warc_length as u64).ok()?;
        let (_status, _headers, body) = WarcReader::parse_http_response(&record.content).ok()?;
        Some(String::from_utf8_lossy(&body).to_string())
    }

    pub fn compute_dom_diff(
        db: &Database,
        old_cap: &crate::database::models::CaptureItem,
        new_cap: &crate::database::models::CaptureItem,
    ) -> Option<DomDiffResult> {
        let old_html = Self::extract_html_from_capture(db, old_cap);
        let new_html = Self::extract_html_from_capture(db, new_cap);

        if old_html.is_none() && new_html.is_none() {
            return None;
        }

        let old_str = old_html.unwrap_or_default();
        let new_str = new_html.unwrap_or_default();

        Self::compute_dom_diff_from_html(&old_str, &new_str)
    }

    pub fn compute_dom_diff_from_html(
        old_str: &str,
        new_str: &str,
    ) -> Option<DomDiffResult> {
        let tag_regex = regex::Regex::new(r"<([a-zA-Z][a-zA-Z0-9]*)\b").ok()?;

        let mut old_counts: HashMap<String, usize> = HashMap::new();
        let mut old_seq = Vec::new();
        for cap in tag_regex.captures_iter(old_str) {
            let tag = cap[1].to_lowercase();
            if tag != "script" && tag != "style" {
                *old_counts.entry(tag.clone()).or_insert(0) += 1;
                old_seq.push(tag);
            }
        }

        let mut new_counts: HashMap<String, usize> = HashMap::new();
        let mut new_seq = Vec::new();
        for cap in tag_regex.captures_iter(new_str) {
            let tag = cap[1].to_lowercase();
            if tag != "script" && tag != "style" {
                *new_counts.entry(tag.clone()).or_insert(0) += 1;
                new_seq.push(tag);
            }
        }

        let total_tags_old = old_seq.len();
        let total_tags_new = new_seq.len();

        let all_tags: HashSet<String> = old_counts.keys().chain(new_counts.keys()).cloned().collect();
        let mut tag_changes = Vec::new();

        for tag in all_tags {
            let oc = *old_counts.get(&tag).unwrap_or(&0);
            let nc = *new_counts.get(&tag).unwrap_or(&0);
            if oc != nc {
                tag_changes.push(DomTagCount {
                    tag,
                    old_count: oc,
                    new_count: nc,
                    diff: nc as i64 - oc as i64,
                });
            }
        }

        tag_changes.sort_by(|a, b| b.diff.abs().cmp(&a.diff.abs()));

        let old_seq_str = old_seq.join("\n");
        let new_seq_str = new_seq.join("\n");
        let seq_diff = TextDiff::from_lines(&old_seq_str, &new_seq_str);
        let mut matching_tags = 0;
        let max_len = total_tags_old.max(total_tags_new);
        for change in seq_diff.iter_all_changes() {
            if change.tag() == ChangeTag::Equal {
                matching_tags += 1;
            }
        }
        let structure_score = if max_len > 0 {
            ((matching_tags as f64 / max_len as f64) * 100.0).round()
        } else {
            100.0
        };

        Some(DomDiffResult {
            total_tags_old,
            total_tags_new,
            tag_changes,
            structure_score,
        })
    }

    pub fn compute_visual_diff(
        db: &Database,
        old_cap: &crate::database::models::CaptureItem,
        new_cap: &crate::database::models::CaptureItem,
    ) -> Option<crate::database::models::VisualDiffResult> {
        let old_rel = old_cap.screenshot_path.as_ref()?;
        let new_rel = new_cap.screenshot_path.as_ref()?;

        let old_path = db.base_dir.join(old_rel);
        let new_path = db.base_dir.join(new_rel);

        if !old_path.exists() || !new_path.exists() {
            return None;
        }

        let img_old = image::open(&old_path).ok()?.to_rgba8();
        let img_new = image::open(&new_path).ok()?.to_rgba8();

        let (w1, h1) = img_old.dimensions();
        let (w2, h2) = img_new.dimensions();
        let w = w1.min(w2);
        let h = h1.min(h2);

        if w == 0 || h == 0 {
            return None;
        }

        let mut diff_img = image::RgbaImage::new(w, h);
        let mut changed_pixels = 0;
        let total_pixels = (w * h) as usize;

        for y in 0..h {
            for x in 0..w {
                let p1 = img_old.get_pixel(x, y);
                let p2 = img_new.get_pixel(x, y);

                let dr = (p1[0] as i32 - p2[0] as i32).abs();
                let dg = (p1[1] as i32 - p2[1] as i32).abs();
                let db_diff = (p1[2] as i32 - p2[2] as i32).abs();

                if dr + dg + db_diff > 30 {
                    changed_pixels += 1;
                    diff_img.put_pixel(x, y, image::Rgba([255, 30, 80, 255]));
                } else {
                    let gray = ((p2[0] as u32 + p2[1] as u32 + p2[2] as u32) / 3) as u8;
                    diff_img.put_pixel(x, y, image::Rgba([gray / 3, gray / 3, gray / 3, 200]));
                }
            }
        }

        let diff_ratio = changed_pixels as f64 / total_pixels as f64;

        use std::io::Cursor;
        let mut buffer = Cursor::new(Vec::new());
        let _ = diff_img.write_to(&mut buffer, image::ImageFormat::Png);
        use base64::Engine;
        let base64_png = base64::engine::general_purpose::STANDARD.encode(buffer.into_inner());

        Some(crate::database::models::VisualDiffResult {
            diff_ratio: (diff_ratio * 100.0).round() / 100.0,
            changed_pixels,
            total_pixels,
            diff_image_base64: Some(base64_png),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dom_diff_detection() {
        let html_v1 = r#"<html><head><title>Test</title></head><body><div><p>Hello</p></div></body></html>"#;
        let html_v2 = r#"<html><head><title>Test</title></head><body><div><p>Hello</p><p>World</p><span>New</span></div></body></html>"#;

        let res = DiffEngine::compute_dom_diff_from_html(html_v1, html_v2).expect("Expected DomDiffResult");
        assert_eq!(res.total_tags_old, 6); // html, head, title, body, div, p
        assert!(res.total_tags_new > res.total_tags_old);
        
        let p_change = res.tag_changes.iter().find(|t| t.tag == "p").expect("Expected p tag change");
        assert_eq!(p_change.old_count, 1);
        assert_eq!(p_change.new_count, 2);
        assert_eq!(p_change.diff, 1);

        let span_change = res.tag_changes.iter().find(|t| t.tag == "span").expect("Expected span tag change");
        assert_eq!(span_change.old_count, 0);
        assert_eq!(span_change.new_count, 1);
        assert_eq!(span_change.diff, 1);
    }
}

