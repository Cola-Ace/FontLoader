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

    pub fn load_fonts_with_progress<F>(&self, paths: Vec<String>, mut progress: F) -> BatchResult
    where
        F: FnMut(usize),
    {
        let mut data = self.data.lock().unwrap();
        let mut result = BatchResult::default();
        let mut completed = 0;

        for requested_path in paths {
            'item: {
                let canonical = match PathBuf::from(&requested_path).canonicalize() {
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
                result.changed.push(canonical_path);
            }
            completed += 1;
            progress(completed);
        }

        if !result.changed.is_empty() {
            let _ = broadcast_font_change();
            if let Err(error) = self.write_active(&data) {
                result
                    .errors
                    .push(operation_error("", "session_log_failed", error.to_string()));
            }
        }
        result.snapshot = snapshot_from(&data);
        result
    }

    pub fn unload_fonts_with_progress<F>(&self, paths: Vec<String>, mut progress: F) -> BatchResult
    where
        F: FnMut(usize),
    {
        let mut data = self.data.lock().unwrap();
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
            if let Err(error) = self.write_active(&data) {
                result
                    .errors
                    .push(operation_error("", "session_log_failed", error.to_string()));
            }
        }
        result.snapshot = snapshot_from(&data);
        result
    }

    pub fn restore_last_session_with_progress<F>(&self, progress: F) -> BatchResult
    where
        F: FnMut(usize),
    {
        let paths = {
            let mut data = self.data.lock().unwrap();
            let paths = std::mem::take(&mut data.restore_candidates);
            let _ = self.session.write_last(&[]);
            paths
        };
        self.load_fonts_with_progress(paths, progress)
    }

    pub fn dismiss_last_session(&self) -> AppSnapshot {
        let mut data = self.data.lock().unwrap();
        data.restore_candidates.clear();
        let _ = self.session.write_last(&[]);
        snapshot_from(&data)
    }

    pub fn prepare_exit(&self, force: bool) -> ExitResult {
        self.prepare_exit_with_progress(force, |_| {})
    }

    pub fn prepare_exit_with_progress<F>(&self, force: bool, progress: F) -> ExitResult
    where
        F: FnMut(usize),
    {
        let paths = {
            let data = self.data.lock().unwrap();
            data.fonts
                .values()
                .map(|entry| entry.canonical_path.clone())
                .collect::<Vec<_>>()
        };
        let _ = self.session.write_last(&paths);
        let batch = self.unload_fonts_with_progress(paths, progress);
        let mut data = self.data.lock().unwrap();

        if batch.errors.is_empty() || force {
            data.exit_errors.clear();
            ExitResult {
                exiting: true,
                errors: batch.errors,
            }
        } else {
            data.exit_errors = batch.errors.clone();
            ExitResult {
                exiting: false,
                errors: batch.errors,
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
    use super::{normalized_key, FontCore};
    use std::{fs, path::PathBuf};

    #[test]
    fn path_keys_are_windows_case_insensitive() {
        assert_eq!(
            normalized_key("C:/Fonts/Example.TTF"),
            normalized_key("c:\\fonts\\example.ttf"),
        );
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
