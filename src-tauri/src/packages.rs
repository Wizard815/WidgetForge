//! `.forgewidget` packages: a widget folder zipped. Import is defensive because packages
//! come from strangers: no absolute paths, no `..`, no symlinks, size limits.

use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::manifest::{self, Issue, Manifest};
use crate::registry::{Origin, Registry};

pub const EXTENSION: &str = "forgewidget";
const MAX_FILE: u64 = 20 * 1024 * 1024;
const MAX_TOTAL: u64 = 100 * 1024 * 1024;
const MAX_FILES: usize = 2000;

#[derive(Debug, Serialize)]
pub struct ImportPreview {
    pub manifest: Manifest,
    pub warnings: Vec<Issue>,
    /// Installed version if this replaces an existing add-on.
    pub replaces_version: Option<String>,
    /// System/network permissions the new version asks for that the installed one did not.
    pub new_permissions: Vec<String>,
}

fn permission_list(m: &Manifest) -> Vec<String> {
    let mut v: Vec<String> = m.permissions.system.iter().map(|s| format!("system:{s}")).collect();
    v.extend(m.permissions.network.iter().map(|h| format!("network:{h}")));
    if m.permissions.storage {
        v.push("storage".into());
    }
    v
}

/// Extracts a package (zip file) or copies a folder into a fresh staging folder under
/// `staging_root`, returning the folder that holds `widget.yaml`.
fn stage(source: &Path, staging_root: &Path) -> Result<PathBuf> {
    let staging = staging_root.join(format!(".staging-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging)?;
    let res = if source.is_dir() { copy_dir(source, &staging, &mut 0, &mut 0) } else { unzip(source, &staging) };
    if let Err(e) = res {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }
    // The manifest may be at the root or inside exactly one top-level folder.
    if staging.join(manifest::MANIFEST_FILE).is_file() {
        return Ok(staging);
    }
    let subdirs: Vec<PathBuf> = fs::read_dir(&staging)?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    if subdirs.len() == 1 && subdirs[0].join(manifest::MANIFEST_FILE).is_file() {
        return Ok(subdirs[0].clone());
    }
    let _ = fs::remove_dir_all(&staging);
    Err(Error::msg("this package has no widget.yaml"))
}

fn staging_top(staged: &Path, staging_root: &Path) -> PathBuf {
    let mut p = staged.to_path_buf();
    while let Some(parent) = p.parent() {
        if parent == staging_root {
            return p;
        }
        p = parent.to_path_buf();
    }
    staged.to_path_buf()
}

fn unzip(file: &Path, dest: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(fs::File::open(file)?)?;
    if archive.len() > MAX_FILES {
        return Err(Error::msg("package has too many files"));
    }
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let shown = entry.name().map(|n| n.into_owned()).unwrap_or_default();
        let Some(rel) = entry.enclosed_name() else {
            return Err(Error::msg(format!("unsafe path in package: {shown}")));
        };
        if entry.is_symlink() {
            return Err(Error::msg(format!("symlinks are not allowed: {shown}")));
        }
        let out = dest.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        // Don't trust the size in the header: read at most the limit + 1 byte.
        (&mut entry).take(MAX_FILE + 1).read_to_end(&mut buf)?;
        if buf.len() as u64 > MAX_FILE {
            return Err(Error::msg(format!("{} is larger than 20 MB", rel.display())));
        }
        total += buf.len() as u64;
        if total > MAX_TOTAL {
            return Err(Error::msg("package is larger than 100 MB"));
        }
        fs::write(out, buf)?;
    }
    Ok(())
}

fn copy_dir(src: &Path, dest: &Path, count: &mut usize, total: &mut u64) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let to = dest.join(entry.file_name());
        if ft.is_symlink() {
            return Err(Error::msg(format!("symlinks are not allowed: {}", entry.path().display())));
        } else if ft.is_dir() {
            copy_dir(&entry.path(), &to, count, total)?;
        } else {
            *count += 1;
            *total += entry.metadata()?.len();
            if *count > MAX_FILES || *total > MAX_TOTAL {
                return Err(Error::msg("folder is too large to import"));
            }
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

/// Validates a package without installing it (for the confirmation dialog), or installs it
/// into `addons_dir/<id>` when `install` is true.
pub fn import(source: &Path, addons_dir: &Path, registry: &Registry, install: bool) -> Result<ImportPreview> {
    fs::create_dir_all(addons_dir)?;
    let staged = stage(source, addons_dir)?;
    let top = staging_top(&staged, addons_dir);
    let result = (|| {
        let (m, issues) = manifest::load_dir(&staged);
        let m = match m {
            Some(m) if !manifest::has_errors(&issues) => m,
            _ => {
                let text: Vec<String> = issues.iter().map(|i| i.to_string()).collect();
                return Err(Error::msg(format!("invalid widget:\n{}", text.join("\n"))));
            }
        };
        let existing = registry.get(&m.id);
        if let Some(e) = existing {
            if e.origin == Origin::Builtin {
                return Err(Error::msg(format!("'{}' is a built-in widget id", m.id)));
            }
        }
        let old_perms = existing.map(|e| permission_list(&e.manifest)).unwrap_or_default();
        let new_permissions = permission_list(&m).into_iter().filter(|p| !old_perms.contains(p)).collect();
        let preview = ImportPreview {
            replaces_version: existing.map(|e| e.manifest.version.clone()),
            new_permissions,
            warnings: issues,
            manifest: m,
        };
        if install {
            let target = addons_dir.join(&preview.manifest.id);
            if target.exists() {
                fs::remove_dir_all(&target)?;
            }
            fs::rename(&staged, &target)?;
        }
        Ok(preview)
    })();
    let _ = fs::remove_dir_all(&top);
    result
}

/// Zips a widget folder into `dest` (a `.forgewidget` file).
pub fn export(widget_dir: &Path, dest: &Path) -> Result<()> {
    let file = fs::File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![widget_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            let ft = e.file_type()?;
            if ft.is_symlink() {
                continue;
            }
            let rel = path.strip_prefix(widget_dir).map_err(|_| Error::msg("bad path"))?;
            let name = rel.to_string_lossy().replace('\\', "/");
            if ft.is_dir() {
                stack.push(path);
            } else {
                zip.start_file(name, opts)?;
                zip.write_all(&fs::read(&path)?)?;
            }
        }
    }
    zip.finish()?;
    Ok(())
}

/// Removes an installed add-on folder.
pub fn uninstall(addons_dir: &Path, widget_dir: &Path) -> Result<()> {
    // Only ever delete something that is directly inside the add-ons folder.
    let canon_root = fs::canonicalize(addons_dir)?;
    let canon = fs::canonicalize(widget_dir)?;
    if canon.parent() != Some(canon_root.as_path()) {
        return Err(Error::Permission("only add-ons can be uninstalled".into()));
    }
    fs::remove_dir_all(canon)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget(dir: &Path, id: &str, extra_perm: bool) {
        fs::create_dir_all(dir.join("assets")).unwrap();
        let perm = if extra_perm { "permissions: { system: [cpu, memory] }\n" } else { "permissions: { system: [cpu] }\n" };
        fs::write(
            dir.join("widget.yaml"),
            format!("schema: 1\nid: {id}\nname: W\nversion: 1.0.0\n{perm}ui:\n  blocks: [ {{ type: text, text: hi }} ]\n"),
        )
        .unwrap();
        fs::write(dir.join("assets/a.txt"), "asset").unwrap();
    }

    #[test]
    fn export_import_roundtrip_and_permission_diff() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let addons = tmp.path().join("addons");
        let builtin = tmp.path().join("builtin");
        widget(&src, "me.w", false);
        let pkg = tmp.path().join("w.forgewidget");
        export(&src, &pkg).unwrap();

        let reg = Registry::scan(&builtin, &addons);
        let preview = import(&pkg, &addons, &reg, false).unwrap();
        assert_eq!(preview.manifest.id, "me.w");
        assert!(!addons.join("me.w").exists(), "preview must not install");

        import(&pkg, &addons, &reg, true).unwrap();
        assert_eq!(fs::read_to_string(addons.join("me.w/assets/a.txt")).unwrap(), "asset");
        let leftovers: Vec<_> = fs::read_dir(&addons).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(leftovers.len(), 1, "staging folders cleaned up: {leftovers:?}");

        // Update asking for more permissions.
        let reg = Registry::scan(&builtin, &addons);
        let src2 = tmp.path().join("src2");
        widget(&src2, "me.w", true);
        let preview = import(&src2, &addons, &reg, false).unwrap();
        assert_eq!(preview.replaces_version.as_deref(), Some("1.0.0"));
        assert_eq!(preview.new_permissions, vec!["system:memory".to_string()]);

        uninstall(&addons, &addons.join("me.w")).unwrap();
        assert!(!addons.join("me.w").exists());
        assert!(uninstall(&addons, &src).is_err());
    }

    #[test]
    fn rejects_traversal_and_builtin_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let addons = tmp.path().join("addons");
        let builtin = tmp.path().join("builtin");
        widget(&builtin.join("x"), "forge.x", false);
        let reg = Registry::scan(&builtin, &addons);

        // A zip containing ../evil
        let pkg = tmp.path().join("evil.forgewidget");
        {
            let mut z = zip::ZipWriter::new(fs::File::create(&pkg).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("../evil.txt", o).unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        assert!(import(&pkg, &addons, &reg, true).is_err());
        assert!(!tmp.path().join("evil.txt").exists());

        // A package reusing a built-in id
        let src = tmp.path().join("src");
        widget(&src, "forge.x", false);
        let err = import(&src, &addons, &reg, true).unwrap_err().to_string();
        assert!(err.contains("built-in"), "{err}");
    }
}
