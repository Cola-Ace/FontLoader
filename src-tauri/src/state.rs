use crate::{
    font::{broadcast_font_change, parse_font, register_font, unregister_font},
    models::{AppSnapshot, BatchResult, ExitResult, FontEntry, OperationError},
    session::SessionFiles,
};
use chrono::Utc;
use std::{
    collections::{BTreeMap, HashSet},
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Default)]
struct CoreData {
    fonts: BTreeMap<String, FontEntry>,
    restore_candidates: Vec<String>,
    exit_errors: Vec<OperationError>,
}

pub struct FontCore {
    data: Mutex<CoreData>,
    session: SessionFiles,
}

impl FontCore {
    pub fn new(app_data_dir: PathBuf) -> io::Result<Self> {
        let session = SessionFiles::new(app_data_dir)?;
        let active = session.read_active().unwrap_or_default();
        let previous = session.read_last().unwrap_or_default();
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();

        for path in active.iter().chain(previous.iter()) {
            let key = normalized_key(path);
            if seen.insert(key) {
                candidates.push(path.clone());
            }
        }
        if !active.is_empty() && candidates != previous {
            session.write_last(&candidates)?;
        }

        let mut cleaned_any = false;
        for path in &active {
            if unregister_font(Path::new(path)).is_ok() {
                cleaned_any = true;
            }
        }
        if cleaned_any {
            let _ = broadcast_font_change();
        }
        session.write_active(&[])?;

        Ok(Self {
            data: Mutex::new(CoreData {
                restore_candidates: candidates,
                ..CoreData::default()
            }),
            session,
        })
    }

    pub fn snapshot(&self) -> AppSnapshot {
        let data = self.data.lock().unwrap();
        snapshot_from(&data)
    }

    pub fn load_fonts_with_progress<F>(&self, paths: Vec<String>, progress: F) -> BatchResult
    where
        F: FnMut(usize),
    {
        let mut data = self.data.lock().unwrap();
        self.load_fonts(&mut data, paths, progress)
    }

    fn load_fonts<F>(&self, data: &mut CoreData, paths: Vec<String>, mut progress: F) -> BatchResult
    where
        F: FnMut(usize),
    {
        let mut result = BatchResult::default();
        let mut loaded_paths = HashSet::new();
        let mut completed = 0;

        for requested_path in paths {
            'item: {
                let canonical = match dunce::canonicalize(&requested_path) {
                    Ok(path) => path,
                    Err(error) => {
                        result.errors.push(operation_error(
                            requested_path,
                            "file_missing",
                            error.to_string(),
                        ));
                        break 'item;
                    }
                };
                let canonical_path = canonical.to_string_lossy().into_owned();
                let key = normalized_key(&canonical_path);
                if data.fonts.contains_key(&key) {
                    loaded_paths.insert(normalized_key(&requested_path));
                    result.skipped.push(canonical_path);
                    break 'item;
                }

                let parsed = match parse_font(&canonical) {
                    Ok(parsed) => parsed,
                    Err(message) => {
                        result.errors.push(operation_error(
                            canonical_path,
                            "invalid_font",
                            message,
                        ));
                        break 'item;
                    }
                };
                if let Err(message) = register_font(&canonical) {
                    result.errors.push(operation_error(
                        canonical_path,
                        "system_load_failed",
                        message,
                    ));
                    break 'item;
                }

                let entry = FontEntry {
                    canonical_path: canonical_path.clone(),
                    file_name: canonical
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("未知文件")
                        .to_string(),
                    format: parsed.format,
                    faces: parsed.faces,
                    loaded_at: Utc::now().to_rfc3339(),
                };
                data.fonts.insert(key, entry);
                loaded_paths.insert(normalized_key(&requested_path));
                result.changed.push(canonical_path);
            }
            completed += 1;
            progress(completed);
        }

        if !result.changed.is_empty() {
            let _ = broadcast_font_change();
        }
        let remaining = data
            .restore_candidates
            .iter()
            .filter(|path| {
                let key = normalized_key(path);
                !loaded_paths.contains(&key) && !data.fonts.contains_key(&key)
            })
            .cloned()
            .collect::<Vec<_>>();
        let candidates_changed = remaining.len() != data.restore_candidates.len();
        let saved = if !result.changed.is_empty() || candidates_changed {
            self.write_active(data).and_then(|()| {
                if candidates_changed {
                    self.session.write_last(&remaining)?;
                    data.restore_candidates = remaining;
                }
                Ok(())
            })
        } else {
            Ok(())
        };
        if let Err(error) = saved {
            result
                .errors
                .push(operation_error("", "session_log_failed", error.to_string()));
        }
        result.snapshot = snapshot_from(data);
        result
    }

    pub fn unload_fonts_with_progress<F>(&self, paths: Vec<String>, progress: F) -> BatchResult
    where
        F: FnMut(usize),
    {
        let mut data = self.data.lock().unwrap();
        self.unload_fonts(&mut data, paths, progress)
    }

    fn unload_fonts<F>(
        &self,
        data: &mut CoreData,
        paths: Vec<String>,
        mut progress: F,
    ) -> BatchResult
    where
        F: FnMut(usize),
    {
        let mut result = BatchResult::default();
        let mut completed = 0;

        for requested_path in paths {
            'item: {
                let key = normalized_key(&requested_path);
                let Some(entry) = data.fonts.get(&key).cloned() else {
                    result.skipped.push(requested_path);
                    break 'item;
                };
                if let Err(message) = unregister_font(Path::new(&entry.canonical_path)) {
                    result.errors.push(operation_error(
                        entry.canonical_path,
                        "system_unload_failed",
                        message,
                    ));
                    break 'item;
                }

                data.fonts.remove(&key);
                result.changed.push(entry.canonical_path);
            }
            completed += 1;
            progress(completed);
        }

        if !result.changed.is_empty() {
            let _ = broadcast_font_change();
            if let Err(error) = self.write_active(data) {
                result
                    .errors
                    .push(operation_error("", "session_log_failed", error.to_string()));
            }
        }
        result.snapshot = snapshot_from(data);
        result
    }

    pub fn restore_last_session_with_progress<F>(&self, progress: F) -> BatchResult
    where
        F: FnMut(usize),
    {
        let mut data = self.data.lock().unwrap();
        let paths = data.restore_candidates.clone();
        self.load_fonts(&mut data, paths, progress)
    }

    pub fn dismiss_last_session(&self) -> io::Result<AppSnapshot> {
        let mut data = self.data.lock().unwrap();
        self.session.write_last(&[])?;
        data.restore_candidates.clear();
        Ok(snapshot_from(&data))
    }

    pub fn prepare_exit(&self, force: bool) -> ExitResult {
        self.prepare_exit_with_progress(force, |_| {})
    }

    pub fn prepare_exit_with_progress<F>(&self, force: bool, progress: F) -> ExitResult
    where
        F: FnMut(usize),
    {
        let mut data = self.data.lock().unwrap();
        let paths = data
            .fonts
            .values()
            .map(|entry| entry.canonical_path.clone())
            .collect::<Vec<_>>();
        let mut restore_paths = paths.clone();
        restore_paths.extend(
            data.restore_candidates
                .iter()
                .filter(|path| !data.fonts.contains_key(&normalized_key(path)))
                .cloned(),
        );
        let mut errors = Vec::new();
        if let Err(error) = self.session.write_last(&restore_paths) {
            errors.push(operation_error("", "session_log_failed", error.to_string()));
            if !force {
                data.exit_errors = errors.clone();
                return ExitResult {
                    exiting: false,
                    errors,
                };
            }
        }
        let batch = self.unload_fonts(&mut data, paths, progress);
        errors.extend(batch.errors);

        if errors.is_empty() || force {
            data.exit_errors.clear();
            ExitResult {
                exiting: true,
                errors,
            }
        } else {
            let mut seen: HashSet<_> = data
                .restore_candidates
                .iter()
                .map(|path| normalized_key(path))
                .collect();
            data.restore_candidates.extend(
                batch
                    .changed
                    .into_iter()
                    .filter(|path| seen.insert(normalized_key(path))),
            );
            data.exit_errors = errors.clone();
            ExitResult {
                exiting: false,
                errors,
            }
        }
    }

    fn write_active(&self, data: &CoreData) -> io::Result<()> {
        let paths = data
            .fonts
            .values()
            .map(|entry| entry.canonical_path.clone())
            .collect::<Vec<_>>();
        self.session.write_active(&paths)
    }
}

fn snapshot_from(data: &CoreData) -> AppSnapshot {
    AppSnapshot {
        fonts: data.fonts.values().cloned().collect(),
        restore_candidates: data.restore_candidates.clone(),
        exit_errors: data.exit_errors.clone(),
    }
}

fn normalized_key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

fn operation_error(path: impl Into<String>, code: &str, message: String) -> OperationError {
    OperationError {
        path: path.into(),
        code: code.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::{normalized_key, FontCore, SessionFiles};
    use std::{fs, path::PathBuf};

    #[test]
    fn path_keys_are_windows_case_insensitive() {
        assert_eq!(
            normalized_key("C:/Fonts/Example.TTF"),
            normalized_key("c:\\fonts\\example.ttf"),
        );
    }

    #[test]
    fn keeps_pending_history_across_empty_exits_until_dismissed() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_path_buf();
        let files = SessionFiles::new(root.clone()).unwrap();
        let missing = root.join("missing.ttf").to_string_lossy().into_owned();
        files.write_active(std::slice::from_ref(&missing)).unwrap();

        let core = FontCore::new(root.clone()).unwrap();
        assert_eq!(core.snapshot().restore_candidates, [missing.clone()]);
        let failed = core.restore_last_session_with_progress(|_| {});
        assert_eq!(failed.errors[0].code, "file_missing");
        assert_eq!(failed.snapshot.restore_candidates, [missing.clone()]);
        assert!(core.prepare_exit(false).exiting);

        let restarted = FontCore::new(root.clone()).unwrap();
        assert_eq!(restarted.snapshot().restore_candidates, [missing]);
        assert!(restarted
            .dismiss_last_session()
            .unwrap()
            .restore_candidates
            .is_empty());
        assert!(restarted.prepare_exit(false).exiting);
        assert!(FontCore::new(root)
            .unwrap()
            .snapshot()
            .restore_candidates
            .is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn restores_only_pending_fonts_after_a_partial_failure() {
        let fonts_dir = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("Fonts");
        let source = fs::read_dir(fonts_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("ttf"))
            })
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_path_buf();
        let files = SessionFiles::new(root.clone()).unwrap();
        let first = root.join("first.ttf");
        let second = root.join("second.ttf");
        let first_path = first.to_string_lossy().into_owned();
        let second_path = second.to_string_lossy().into_owned();
        fs::copy(&source, &first).unwrap();
        files
            .write_last(&[first_path, second_path.clone()])
            .unwrap();

        let core = FontCore::new(root.clone()).unwrap();
        let partial = core.restore_last_session_with_progress(|_| {});
        assert_eq!(partial.changed.len(), 1);
        assert_eq!(partial.errors[0].code, "file_missing");
        assert_eq!(partial.snapshot.restore_candidates, [second_path.clone()]);
        assert_eq!(files.read_last().unwrap(), [second_path.clone()]);

        assert!(core.prepare_exit(false).exiting);
        let core = FontCore::new(root.clone()).unwrap();
        assert_eq!(core.snapshot().restore_candidates.len(), 2);
        let partial = core.restore_last_session_with_progress(|_| {});
        assert_eq!(partial.changed.len(), 1);
        assert_eq!(partial.snapshot.restore_candidates, [second_path]);
        fs::copy(source, second).unwrap();
        let retried = core.restore_last_session_with_progress(|_| {});
        assert!(retried.errors.is_empty());
        assert_eq!(retried.changed.len(), 1);
        assert!(retried.skipped.is_empty());
        assert!(retried.snapshot.restore_candidates.is_empty());

        let unloaded = core.unload_fonts_with_progress(
            retried
                .snapshot
                .fonts
                .into_iter()
                .map(|font| font.canonical_path)
                .collect(),
            |_| {},
        );
        assert!(unloaded.errors.is_empty());
        assert!(core.prepare_exit(false).exiting);
        assert!(FontCore::new(root)
            .unwrap()
            .snapshot()
            .restore_candidates
            .is_empty());
    }

    #[test]
    fn reports_progress_for_each_requested_path() {
        let directory = tempfile::tempdir().unwrap();
        let core = FontCore::new(directory.path().join("state")).unwrap();
        let mut load_progress = Vec::new();

        let result = core.load_fonts_with_progress(
            vec!["missing-a.ttf".to_string(), "missing-b.ttf".to_string()],
            |completed| load_progress.push(completed),
        );

        assert_eq!(load_progress, vec![1, 2]);
        assert_eq!(result.errors.len(), 2);

        let mut unload_progress = Vec::new();
        core.unload_fonts_with_progress(
            vec!["missing-a.ttf".to_string(), "missing-b.ttf".to_string()],
            |completed| unload_progress.push(completed),
        );
        assert_eq!(unload_progress, vec![1, 2]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_font_files_load_and_unload() {
        let fonts_dir = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("Fonts");
        let directory = tempfile::tempdir().unwrap();
        let core = FontCore::new(directory.path().join("state")).unwrap();
        let mut tested_formats = Vec::new();

        for extension in ["ttf", "otf", "ttc"] {
            let source = fs::read_dir(&fonts_dir)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|path| {
                    path.extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
                });
            let Some(source) = source else {
                continue;
            };
            let destination = directory.path().join(format!("sample.{extension}"));
            fs::copy(source, &destination).unwrap();

            let loaded = core
                .load_fonts_with_progress(vec![destination.to_string_lossy().into_owned()], |_| {});
            assert!(loaded.errors.is_empty(), "{extension}: {:?}", loaded.errors);
            assert_eq!(loaded.changed.len(), 1);

            let unloaded = core.unload_fonts_with_progress(loaded.changed.clone(), |_| {});
            assert!(
                unloaded.errors.is_empty(),
                "{extension}: {:?}",
                unloaded.errors
            );
            assert_eq!(unloaded.changed.len(), 1);
            tested_formats.push(extension);
        }

        assert!(tested_formats.contains(&"ttf"));
    }
}
