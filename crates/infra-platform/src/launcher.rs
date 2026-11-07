use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use hoarddeck_application::ports::AppLauncher;
use hoarddeck_domain::Slicer;

/// Numbers temporary copies, so two variants named `dragon.stl.txt` never overwrite each other.
static COPIES: AtomicU64 = AtomicU64::new(0);

/// Starts slicers with arguments as a list, so no quoting is involved.
pub struct SystemLauncher {
    /// Where disguised meshes (`dragon.stl.txt`) get a copy with a real `.stl` name; slicers go by the name.
    temp_dir: PathBuf,
}

impl SystemLauncher {
    #[must_use]
    pub fn new(temp_dir: PathBuf) -> Self {
        Self { temp_dir }
    }

    fn with_real_names(&self, files: &[String]) -> Result<Vec<String>, String> {
        files
            .iter()
            .map(|file| {
                let name = Path::new(file).file_name().and_then(|name| name.to_str()).unwrap_or_default();
                let Some(position) = name.to_ascii_lowercase().find(".stl.") else { return Ok(file.clone()) };
                fs::create_dir_all(&self.temp_dir).map_err(|e| e.to_string())?;
                let number = COPIES.fetch_add(1, Ordering::Relaxed);
                let copy = self.temp_dir.join(format!("{number}-{}.stl", &name[..position]));
                // Copying reads the library only; the copy lives in the index folder's temp/ and is replaced next time.
                fs::copy(file, &copy).map_err(|e| format!("{file}: {e}"))?;
                Ok(copy.display().to_string())
            })
            .collect()
    }
}

impl AppLauncher for SystemLauncher {
    fn launch(&self, slicer: &Slicer, files: &[String]) -> Result<(), String> {
        let files = self.with_real_names(files)?;
        let bundle =
            Path::new(&slicer.program).extension().is_some_and(|extension| extension.eq_ignore_ascii_case("app"));
        if cfg!(target_os = "macos") && bundle {
            return open_bundle(slicer, &files);
        }
        let mut child = Command::new(&slicer.program)
            .args(slicer.arguments(&files))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("{}: {error}", slicer.program))?;
        // Reap the slicer when it exits, so finished children do not linger as zombies.
        std::thread::spawn(move || child.wait());
        Ok(())
    }
}

/// `open -a App files… --args extra…`: files arrive as documents (also in a running instance), extra arguments go
/// to the app. `open` exits right after handing over, so its exit status tells whether the app could be opened.
fn open_bundle(slicer: &Slicer, files: &[String]) -> Result<(), String> {
    let mut command = Command::new("open");
    command.arg("-a").arg(&slicer.program).args(files);
    let extra = slicer.extra_args();
    if !extra.is_empty() {
        command.arg("--args").args(extra);
    }
    let output = command.stdin(Stdio::null()).output().map_err(|error| format!("open: {error}"))?;
    if output.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&output.stderr).trim().to_owned()) }
}
