//! Name patterns and comment-preserving scoped preferences.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use toml_edit::{Array, DocumentMut, Item, Table, value};
pub fn matches(pattern: &str, name: &str, directory: bool) -> bool {
    if pattern == "<other-folders>" {
        return directory;
    }
    if let Some(literal) = pattern.strip_prefix('=') {
        return equal(literal, name);
    }
    let (pattern, name) = if cfg!(windows) {
        (pattern.to_lowercase(), name.to_lowercase())
    } else {
        (pattern.into(), name.into())
    };
    let (p, n) = (
        pattern.chars().collect::<Vec<_>>(),
        name.chars().collect::<Vec<_>>(),
    );
    let (mut i, mut j, mut star, mut retry) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && p[i] != '%' && p[i] == n[j] {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '%' {
            star = Some(i);
            i += 1;
            retry = j;
        } else if let Some(s) = star {
            retry += 1;
            j = retry;
            i = s + 1;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '%' {
        i += 1;
    }
    i == p.len()
}
fn equal(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}
pub fn exact(pattern: &str, name: &str) -> bool {
    pattern != "<other-folders>"
        && (pattern.starts_with('=') || !pattern.contains('%'))
        && equal(pattern.strip_prefix('=').unwrap_or(pattern), name)
}
pub fn save_sequence(config: &Path, directory: &Path, names: &[String]) -> Result<(), String> {
    edit(config, |doc| {
        if doc.get("directory_order").is_none() {
            doc["directory_order"] = Item::Table(Table::new());
        }
        let key = directory
            .to_str()
            .ok_or("Cannot save a non-UTF8 directory in TOML")?;
        let rules = doc["directory_order"]
            .as_table_like_mut()
            .ok_or("directory_order must be a table")?;
        // Resolve existing ~/ or canonical path keys rather than leaving competing aliases.
        let existing = rules
            .iter()
            .find(|(k, _)| {
                let path = crate::config::expand_home(k);
                path.is_absolute() && fs::canonicalize(&path).unwrap_or(path) == directory
            })
            .map(|(k, _)| k.to_owned())
            .unwrap_or_else(|| key.to_owned());
        if !rules.contains_key(&existing) {
            rules.insert(&existing, Item::Table(Table::new()));
        }
        let rule = rules
            .get_mut(&existing)
            .and_then(Item::as_table_like_mut)
            .ok_or("Directory rule must be a table")?;
        let mut sequence = Array::new();
        for name in names {
            sequence.push(format!("={name}"));
        }
        sequence.push("%");
        rule.insert("sequence", value(sequence));
        Ok(())
    })
}
pub fn save_hidden(config: &Path, paths: &[PathBuf]) -> Result<(), String> {
    edit(config, |doc| {
        if doc.get("browser").is_none() {
            doc["browser"] = Item::Table(Table::new());
        }
        let table = doc["browser"]
            .as_table_like_mut()
            .ok_or("browser must be a table")?;
        let mut values = Array::new();
        for path in paths {
            values.push(path.to_str().ok_or("Cannot persist a non-UTF8 pathname")?);
        }
        table.insert("excluded_paths", value(values));
        Ok(())
    })
}
fn edit(
    config: &Path,
    change: impl FnOnce(&mut DocumentMut) -> Result<(), String>,
) -> Result<(), String> {
    let target = if config.exists() {
        fs::canonicalize(config).map_err(|e| e.to_string())?
    } else {
        std::path::absolute(config).map_err(|e| e.to_string())?
    };
    let original = match fs::read_to_string(&target) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut document = original.parse::<DocumentMut>().map_err(|e| e.to_string())?;
    change(&mut document)?;
    let updated = document.to_string();
    let _: toml::Value = toml::from_str(&updated).map_err(|e| e.to_string())?;
    let parent = target.parent().ok_or("No config parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    static N: AtomicU64 = AtomicU64::new(0);
    let temp = parent.join(format!(
        ".filex-config-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        if let Ok(metadata) = fs::metadata(&target) {
            file.set_permissions(metadata.permissions())
                .map_err(|e| e.to_string())?;
        }
        file.write_all(updated.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        // Refuse to overwrite edits made while the preference was being prepared.
        if fs::read_to_string(&target).unwrap_or_default() != original {
            return Err("Config changed externally; reload and retry".into());
        }
        crate::files::replace_staged(&temp, &target)
    })();
    let _ = fs::remove_file(temp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        model::{Browser, Layout},
    };
    use std::rc::Rc;
    use unicode_width::UnicodeWidthStr;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "filex-prefs-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(fs::canonicalize(p).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn percent_patterns_are_anchored_unicode_and_can_be_literal() {
        for (pattern, name, yes) in [
            ("d-%", "d-one", true),
            ("d-%", "d-", true),
            ("d-%", "ad-one", false),
            ("%-d", "one-d", true),
            ("%-d", "one-d-x", false),
            ("%中%", "a中文", true),
            ("a%b%c", "abc", true),
            ("a%b%c", "abdc", true),
            ("a%b%c", "ac", false),
            ("=100%.txt", "100%.txt", true),
            ("=100%.txt", "100x.txt", false),
            ("%", "anything", true),
        ] {
            assert_eq!(matches(pattern, name, false), yes, "{pattern} {name}");
        }
        assert!(matches("<other-folders>", "folder", true));
        assert!(!matches("<other-folders>", "file", false));
    }
    #[test]
    fn sequence_first_match_and_legacy_exact_exceptions_work() {
        let f = Fixture::new();
        let p = f.0.join("config");
        fs::write(
            &p,
            "[order]\nsequence=['README.md','d-%','%-d','%']\ntop=['other']\n",
        )
        .unwrap();
        let c = Config::load(&p).unwrap();
        assert_eq!(c.order_rank(&f.0, "README.md", false), (-2, 0));
        assert_eq!(c.order_rank(&f.0, "d-one-d", true), (-2, 1));
        assert_eq!(c.order_rank(&f.0, "one-d", false), (-2, 2));
        assert_eq!(c.order_rank(&f.0, "other", false), (-2, 3));
        fs::write(&p, "[order]\ntop=['d-%']\nbottom=['d-last']\n").unwrap();
        let c = Config::load(&p).unwrap();
        assert_eq!(c.order_rank(&f.0, "d-last", true), (1, 0));
        assert_eq!(c.order_rank(&f.0, "d-first", false), (-1, 0));
    }
    #[test]
    fn recursive_order_inherits_fields_and_uses_nearest_ancestor() {
        let f = Fixture::new();
        let site = f.0.join("site");
        let nested = site.join("nested");
        let deep = nested.join("deep");
        fs::create_dir_all(&deep).unwrap();
        let config = f.0.join("config");
        fs::write(&config, format!(
            "[order]\nbottom=['node_modules']\n[recursive_order.'{}']\nsequence=['README.md','%.html','%.css','%.js','%']\n[recursive_order.'{}']\ntop=['nested-pin']\n[directory_order.'{}']\nbottom=[]\n",
            site.display(), nested.display(), deep.display()
        )).unwrap();
        let c = Config::load(&config).unwrap();
        for directory in [&site, &nested, &deep] {
            assert_eq!(c.order_rank(directory, "README.md", false), (-2, 0));
            assert_eq!(c.order_rank(directory, "z.html", false), (-2, 1));
            assert_eq!(c.order_rank(directory, "a.css", false), (-2, 2));
            assert_eq!(c.order_rank(directory, "a.js", false), (-2, 3));
        }
        assert_eq!(
            c.order_rank(&f.0.join("site-other"), "z.html", false),
            (0, 0)
        );
        assert_eq!(c.order_rank(&f.0, "z.html", false), (0, 0));
        // An exact, empty field clears inheritance while unrelated fields still inherit.
        fs::write(&config, format!(
            "[order]\nbottom=['node_modules']\n[recursive_order.'{}']\nsequence=['%.html','%.css','%.js']\n[recursive_order.'{}']\nsequence=['%.js','%.html','%.css']\n[directory_order.'{}']\nsequence=[]\ntop=['special.txt']\n",
            site.display(), nested.display(), deep.display()
        )).unwrap();
        let c = Config::load(&config).unwrap();
        assert_eq!(c.order_rank(&site, "z.html", false), (-2, 0));
        assert_eq!(c.order_rank(&nested, "a.js", false), (-2, 0));
        assert_eq!(c.order_rank(&deep, "z.html", false), (0, 0));
        assert_eq!(c.order_rank(&deep, "special.txt", false), (-1, 0));
        assert_eq!(c.order_rank(&deep, "node_modules", true), (1, 0));
    }
    #[test]
    fn recursive_order_applies_to_tree_and_manual_sequence_stays_local() {
        let f = Fixture::new();
        let site = f.0.join("site");
        let nested = site.join("nested");
        let deeper = nested.join("deeper");
        fs::create_dir_all(&deeper).unwrap();
        for directory in [&site, &nested, &deeper] {
            for name in ["a.js", "m.css", "z.html"] {
                fs::write(directory.join(name), "").unwrap();
            }
        }
        let config = f.0.join("config");
        fs::write(
            &config,
            format!(
                "[recursive_order.'{}']\nsequence=['%.html','%.css','%.js','%']\n",
                site.display()
            ),
        )
        .unwrap();
        for directory in [&site, &nested, &deeper] {
            let b = Browser::new(directory, Config::load(&config).unwrap()).unwrap();
            let names = b
                .root
                .borrow()
                .children
                .iter()
                .map(|n| {
                    n.borrow()
                        .path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<Vec<_>>();
            assert_eq!(&names[..3], &["z.html", "m.css", "a.js"]);
        }
        save_sequence(
            &config,
            &nested,
            &["a.js".into(), "m.css".into(), "z.html".into()],
        )
        .unwrap();
        let c = Config::load(&config).unwrap();
        assert_eq!(c.order_rank(&nested, "a.js", false), (-2, 0));
        assert_eq!(c.order_rank(&deeper, "a.js", false), (-2, 2));
        assert!(
            fs::read_to_string(config)
                .unwrap()
                .contains("recursive_order")
        );
    }
    #[test]
    fn recursive_order_rejects_relative_paths_and_unknown_fields() {
        let f = Fixture::new();
        let config = f.0.join("config");
        fs::write(&config, "[recursive_order.site]\nsequence=['%.html']\n").unwrap();
        assert!(Config::load(&config).unwrap_err().contains("absolute"));
        fs::write(
            &config,
            format!(
                "[recursive_order.'{}']\nextensions=['.html']\n",
                f.0.display()
            ),
        )
        .unwrap();
        assert!(
            Config::load(&config)
                .unwrap_err()
                .contains("Unknown ordering field")
        );
    }
    #[test]
    fn preferences_preserve_comments_symlinks_and_unrelated_values() {
        let f = Fixture::new();
        let target = f.0.join("config");
        let entry = f.0.join("entry.conf");
        fs::write(&target,"# keep this comment\n[browser]\nmax_entries = 80 # keep inline\n[order]\ntop=['index.html']\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &entry).unwrap();
        #[cfg(windows)]
        fs::copy(&target, &entry).unwrap();
        save_sequence(&entry, &f.0, &["100%.txt".into(), "d-one".into()]).unwrap();
        save_hidden(&entry, &[f.0.join("hidden")]).unwrap();
        let text = fs::read_to_string(&entry).unwrap();
        assert!(text.contains("# keep this comment"));
        assert!(text.contains("# keep inline"));
        let conf = Config::load(&entry).unwrap();
        assert_eq!(conf.max_entries, 80);
        assert_eq!(conf.order.top, ["index.html"]);
        assert_eq!(conf.order_rank(&f.0, "100%.txt", false), (-2, 0));
        assert_eq!(conf.excluded_paths, [f.0.join("hidden")]);
        #[cfg(unix)]
        assert!(
            fs::symlink_metadata(&entry)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    #[test]
    fn grouped_subtrees_are_disjoint_and_edges_do_not_cross_labels() {
        let f = Fixture::new();
        let a = f.0.join("a");
        let b = f.0.join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::create_dir(a.join("sub")).unwrap();
        for p in [
            f.0.join("README.md"),
            f.0.join("file.conf"),
            a.join("a.txt"),
            a.join("sub/deep.txt"),
            b.join("b.txt"),
        ] {
            fs::write(p, "test").unwrap();
        }
        let mut browser = Browser::new(&f.0, Config::default()).unwrap();
        assert!(!browser.conf.grouped);
        browser.conf.grouped = true;
        browser.expand_all().unwrap();
        let layout = Layout::new(&browser);
        let cells = layout.cells(0, 0, layout.width, layout.height);
        for item in &layout.items {
            for x in item.x..item.x + item.label.width() {
                assert!(
                    !cells.contains_key(&(x, item.y)),
                    "edge crosses {}",
                    item.label
                );
            }
        }
        let root_file = layout
            .items
            .iter()
            .find(|i| i.node.borrow().path == f.0.join("README.md"))
            .unwrap();
        let directory = layout
            .items
            .iter()
            .find(|i| i.node.borrow().path == a)
            .unwrap();
        assert!(root_file.x > directory.x);
        assert!(root_file.y > layout.items[0].y);
        let deep = layout
            .items
            .iter()
            .find(|i| i.node.borrow().path == a.join("sub/deep.txt"))
            .unwrap();
        let route = layout.path_cells(
            &deep.node,
            browser.conf.gap,
            0,
            0,
            layout.width,
            layout.height,
        );
        assert!(!route.is_empty());
        assert!(route.iter().all(|p| cells.contains_key(p)));
        let retained = browser.selected.clone();
        browser.conf.grouped = false;
        let classic = Layout::new(&browser);
        assert!(Rc::ptr_eq(&retained, &browser.selected));
        assert!(classic.height < layout.height);
    }
    #[test]
    fn exclusions_are_independent_of_dotfiles_and_search() {
        let f = Fixture::new();
        fs::create_dir(f.0.join("cache")).unwrap();
        fs::write(f.0.join("cache/secret.txt"), "x").unwrap();
        fs::write(f.0.join(".dot"), "x").unwrap();
        let mut c = Config::default();
        c.excluded_paths.push(f.0.join("cache"));
        let mut b = Browser::new(&f.0, c).unwrap();
        b.conf.hidden = true;
        b.refresh();
        assert!(
            b.visible()
                .iter()
                .any(|n| n.borrow().path == f.0.join(".dot"))
        );
        assert!(
            !b.visible()
                .iter()
                .any(|n| n.borrow().path == f.0.join("cache"))
        );
        b.search("secret");
        assert!(b.results.is_empty());
        assert!(f.0.join("cache/secret.txt").exists());
    }
}
