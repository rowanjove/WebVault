use crate::database::Database;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const ALLOWED_PREFIXES: &[&str] = &["archives/", "screenshots/", "browser_profiles/"];
const SKIP_PROFILE_DIRS: &[&str] = &["Cache", "Code Cache", "GPUCache", "GrShaderCache", "ShaderCache"];
const ALLOWED_FILES: &[&str] = &["app.db", "app.db-wal", "app.db-shm"];

pub fn export_backup(db: &Database, output_path: &Path) -> Result<()> {
    {
        let conn = db.lock_conn()?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create backup at {:?}", output_path))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let db_file = db.base_dir.join("app.db");
    if db_file.exists() {
        add_file_to_zip(&mut zip, &db_file, "app.db", options)?;
    }

    for dir_name in ["archives", "screenshots", "browser_profiles"] {
        let dir = db.base_dir.join(dir_name);
        if dir.exists() {
            add_dir_to_zip(&mut zip, &dir, dir_name, options)?;
        }
    }

    zip.finish()?;
    Ok(())
}

pub fn import_backup(db: &Database, zip_path: &Path) -> Result<()> {
    let temp = db
        .base_dir
        .join("temp")
        .join(format!("restore_{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&temp)?;

    let unpack_result = unpack_backup_zip(zip_path, &temp);
    if unpack_result.is_err() {
        let _ = std::fs::remove_dir_all(&temp);
        return unpack_result;
    }

    let new_db = temp.join("app.db");
    if !new_db.exists() {
        let _ = std::fs::remove_dir_all(&temp);
        anyhow::bail!("Backup is missing app.db");
    }
    rusqlite::Connection::open(&new_db).context("Backup app.db could not be opened")?;

    let _warc_guard = db.lock_warc()?;
    db.close_for_replace()?;

    let staging = db
        .base_dir
        .join("temp")
        .join(format!("restore_bak_{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&staging)?;

    let move_aside = |name: &str| -> Result<()> {
        let src = db.base_dir.join(name);
        if !src.exists() {
            return Ok(());
        }
        let dest = staging.join(name);
        if std::fs::rename(&src, &dest).is_ok() {
            return Ok(());
        }
        if src.is_dir() {
            copy_dir_all(&src, &dest).with_context(|| format!("Failed to copy {:?} aside", src))?;
            std::fs::remove_dir_all(&src).with_context(|| format!("Failed to remove {:?}", src))?;
        } else {
            std::fs::copy(&src, &dest).with_context(|| format!("Failed to copy {:?} aside", src))?;
            std::fs::remove_file(&src).with_context(|| format!("Failed to remove {:?}", src))?;
        }
        Ok(())
    };

    let rollback = || {
        for name in ["app.db", "app.db-wal", "app.db-shm", "archives", "screenshots", "browser_profiles"] {
            let live = db.base_dir.join(name);
            let bak = staging.join(name);
            if live.exists() {
                let _ = if live.is_dir() {
                    std::fs::remove_dir_all(&live)
                } else {
                    std::fs::remove_file(&live)
                };
            }
            if bak.exists() {
                let _ = std::fs::rename(&bak, &live);
            }
        }
        let _ = db.reopen();
    };

    if let Err(e) = (|| -> Result<()> {
        for name in ["app.db", "app.db-wal", "app.db-shm", "archives", "screenshots", "browser_profiles"] {
            move_aside(name)?;
        }
        std::fs::copy(&new_db, db.base_dir.join("app.db")).context("Failed to replace app.db")?;
        for extra in ["app.db-wal", "app.db-shm"] {
            let src = temp.join(extra);
            if src.exists() {
                std::fs::copy(&src, db.base_dir.join(extra))?;
            }
        }
        for dir_name in ["archives", "screenshots", "browser_profiles"] {
            let src = temp.join(dir_name);
            if src.exists() {
                copy_dir_all(&src, &db.base_dir.join(dir_name))?;
            }
        }
        db.reopen().context("Failed to reopen database after restore")?;
        Ok(())
    })() {
        rollback();
        let _ = std::fs::remove_dir_all(&temp);
        return Err(e);
    }

    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_dir_all(&temp);
    Ok(())
}

fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    path: &Path,
    name: &str,
    options: SimpleFileOptions,
) -> Result<()> {
    zip.start_file(name, options)?;
    let mut f = File::open(path)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        zip.write_all(&buf[..n])?;
    }
    Ok(())
}

fn add_dir_to_zip(
    zip: &mut ZipWriter<File>,
    dir: &Path,
    zip_prefix: &str,
    options: SimpleFileOptions,
) -> Result<()> {
    fn walk(
        zip: &mut ZipWriter<File>,
        dir: &Path,
        zip_prefix: &str,
        options: SimpleFileOptions,
    ) -> Result<()> {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIP_PROFILE_DIRS.iter().any(|d| name.eq_ignore_ascii_case(d)) {
                continue;
            }
            let zip_name = format!("{}/{}", zip_prefix, name);
            if path.is_dir() {
                walk(zip, &path, &zip_name, options)?;
            } else {
                add_file_to_zip(zip, &path, &zip_name, options)?;
            }
        }
        Ok(())
    }
    walk(zip, dir, zip_prefix, options)
}

fn unpack_backup_zip(zip_path: &Path, dest: &Path) -> Result<()> {
    let file = File::open(zip_path).with_context(|| format!("Failed to open backup {:?}", zip_path))?;
    let mut archive = ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let raw_name = entry.name().replace('\\', "/");
        if raw_name.ends_with('/') {
            continue;
        }
        if !is_allowed_entry(&raw_name) {
            continue;
        }
        let out_path = safe_dest_path(dest, &raw_name)?;
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

fn is_allowed_entry(name: &str) -> bool {
    ALLOWED_FILES.iter().any(|f| name == *f)
        || ALLOWED_PREFIXES.iter().any(|p| name.starts_with(p) && !name.contains(".."))
}

fn safe_dest_path(base: &Path, name: &str) -> Result<PathBuf> {
    if name.contains("..") || Path::new(name).is_absolute() {
        anyhow::bail!("Invalid backup entry path: {}", name);
    }
    let joined = base.join(name);
    Ok(joined)
}

fn copy_dir_all(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_roundtrip_restores_archive_files() {
        let temp = std::env::temp_dir().join(format!("webvault_bak_{}", uuid::Uuid::new_v4().simple()));
        let db = Database::init(&temp).unwrap();
        let site = db.create_site("Backup Site", "https://example.com").unwrap();
        let archive_dir = temp.join("archives").join(&site.id);
        std::fs::create_dir_all(&archive_dir).unwrap();
        std::fs::write(archive_dir.join("data.warc.gz"), b"archive-bytes").unwrap();
        let profile_dir = temp.join("browser_profiles").join(&site.id);
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(profile_dir.join("Cookies"), b"cookie-bytes").unwrap();

        let zip_path = temp.join("backup.zip");
        export_backup(&db, &zip_path).unwrap();

        std::fs::remove_dir_all(&archive_dir).unwrap();
        db.delete_site(&site.id).unwrap();
        assert!(db.list_sites().unwrap().is_empty());

        import_backup(&db, &zip_path).unwrap();
        let sites = db.list_sites().unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].name, "Backup Site");
        let restored = temp.join("archives").join(&sites[0].id).join("data.warc.gz");
        assert_eq!(std::fs::read(&restored).unwrap(), b"archive-bytes");
        let restored_profile = temp.join("browser_profiles").join(&sites[0].id).join("Cookies");
        assert_eq!(std::fs::read(&restored_profile).unwrap(), b"cookie-bytes");

        let _ = std::fs::remove_dir_all(&temp);
    }
}
