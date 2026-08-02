use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

#[derive(Debug, Default, Serialize, Deserialize)]
struct PathLog {
    paths: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SessionFiles {
    active_path: PathBuf,
    last_path: PathBuf,
}

impl SessionFiles {
    pub fn new(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self {
            active_path: root.join("active-session.json"),
            last_path: root.join("last-session.json"),
        })
    }

    pub fn read_active(&self) -> io::Result<Vec<String>> {
        read_paths(&self.active_path)
    }

    pub fn read_last(&self) -> io::Result<Vec<String>> {
        read_paths(&self.last_path)
    }

    pub fn write_active(&self, paths: &[String]) -> io::Result<()> {
        write_paths(&self.active_path, paths)
    }

    pub fn write_last(&self, paths: &[String]) -> io::Result<()> {
        write_paths(&self.last_path, paths)
    }
}

fn read_paths(path: &Path) -> io::Result<Vec<String>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<PathLog>(&bytes)
            .map(|log| log.paths)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn write_paths(path: &Path, paths: &[String]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "日志路径无父目录"))?;
    fs::create_dir_all(parent)?;

    let data = serde_json::to_vec_pretty(&PathLog {
        paths: paths.to_vec(),
    })?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(&data)?;
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|error| error.error)
}

#[cfg(test)]
mod tests {
    use super::SessionFiles;

    #[test]
    fn session_files_round_trip() {
        let directory = tempfile::tempdir().unwrap();
        let files = SessionFiles::new(directory.path().to_path_buf()).unwrap();
        let paths = vec![
            "C:\\Fonts\\one.ttf".to_string(),
            "C:\\Fonts\\two.otf".to_string(),
        ];

        files.write_active(&paths).unwrap();
        files.write_last(&paths[..1]).unwrap();

        assert_eq!(files.read_active().unwrap(), paths);
        assert_eq!(files.read_last().unwrap(), paths[..1]);
    }
}
