use std::path::{Path, PathBuf};

/// Replace the user's home directory prefix with `~` for display.
pub fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        if rest == Path::new("") {
            return "~".to_string();
        }
        let mut buf = PathBuf::from("~");
        buf.push(rest);
        return buf.display().to_string();
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_home_with_tilde() {
        let home = dirs::home_dir().expect("home dir exists");
        let input = home.join("Documents/slides.pdf");
        assert_eq!(display_path(&input), "~/Documents/slides.pdf");
    }

    #[test]
    fn leaves_non_home_path_unchanged() {
        let input = Path::new("/tmp/other/file.mp4");
        assert_eq!(display_path(input), "/tmp/other/file.mp4");
    }

    #[test]
    fn handles_home_dir_itself() {
        let home = dirs::home_dir().expect("home dir exists");
        assert_eq!(display_path(&home), "~");
    }
}
