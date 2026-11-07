//! `hoarddeck`: read-only spike tool. See README.md.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use hoarddeck_app::App;
use hoarddeck_application::ModelSummary;
use hoarddeck_application::browse::{BrowseRequest, Sort};
use hoarddeck_application::ports::EventSink;
use hoarddeck_application::scan::{ScanMode, ScanPhase, ScanProgress};
use hoarddeck_application::search::SearchRequest;
use hoarddeck_domain::{LibraryId, ModelId, RelPath};

const USAGE: &str = "usage: hoarddeck [--index DIR] [--threads N] <command>
  libraries                          list libraries
  add <name> <folder>                register a folder (read-only)
  scan <library> [--full]            walk, group and index a library; --full relists unchanged folders
  search <library|all> <query...>    e.g. search 1 wolverin type:stl in:Miniatures
  browse <library> [folder] [--all]  a folder page; --all = everything inside
  model <id>                         one model with its files
  plates <file.3mf> [--binary]       build plates of a 3MF (read-only); --binary writes the viewer format";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut args: Vec<String>) -> Result<(), String> {
    let index_dir = take_option(&mut args, "--index")?.map_or_else(
        || hoarddeck_infra_fs::resolve_index_dir().map_err(|e| e.to_string()),
        |dir| Ok(PathBuf::from(dir)),
    )?;
    let threads = take_option(&mut args, "--threads")?
        .map_or(Ok(6), |n| n.parse().map_err(|_| format!("--threads: not a number: {n}")))?;
    let recursive = take_flag(&mut args, "--all");
    let binary = take_flag(&mut args, "--binary");
    if args.first().map(String::as_str) == Some("plates") {
        return print_plates(args.get(1).ok_or(USAGE)?, binary);
    }
    let mode = if take_flag(&mut args, "--full") { ScanMode::Full } else { ScanMode::Incremental };
    let app = App::open(&index_dir, threads, Arc::new(StderrProgress::default())).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let arg = |i: usize| args.get(i).map(String::as_str);
    match (arg(0), arg(1)) {
        (Some("libraries"), _) => {
            for library in app.libraries.list().map_err(|e| e.to_string())? {
                println!(
                    "{:>3}  {:<24} {}  [{:?}]",
                    library.id.0,
                    library.name,
                    library.root,
                    library.settings.access()
                );
            }
        }
        (Some("add"), Some(name)) => {
            let folder = arg(2).ok_or(USAGE)?;
            let folder =
                std::fs::canonicalize(folder).map_or_else(|_| folder.to_owned(), |path| path.display().to_string());
            let library = app.add_library.execute(name, &folder).map_err(|e| e.to_string())?;
            println!("added library {} ({}), read-only", library.id.0, library.name);
        }
        (Some("scan"), Some(id)) => {
            let report = app.scan_library.execute(library_id(id)?, mode).map_err(|e| e.to_string())?;
            println!("{report:#?}");
        }
        (Some("search"), Some(library)) => {
            let library = if library == "all" { None } else { Some(library_id(library)?) };
            let request = SearchRequest { text: args[2..].join(" "), library, scope: None, offset: 0, limit: 25 };
            let result = app.search_models.execute(&request).map_err(|e| e.to_string())?;
            print_models(&result.models);
            println!("{} of {} matches", result.models.len(), result.total);
        }
        (Some("browse"), Some(id)) => {
            let folder = RelPath::new(arg(2).unwrap_or_default().trim_matches('/')).map_err(|e| e.to_string())?;
            let request = BrowseRequest {
                library: library_id(id)?,
                folder,
                recursive,
                sort: Sort::Name,
                cursor: None,
                limit: 50,
            };
            let page = app.browse_folder.execute(&request).map_err(|e| e.to_string())?;
            for folder in &page.folders {
                println!("[{}]  {} models  {}", folder.display_name, folder.total_models, folder.path);
            }
            print_models(&page.page.models);
        }
        (Some("model"), Some(id)) => {
            let id = id.parse().map(ModelId).map_err(|_| format!("not a model id: {id}"))?;
            let detail = app.get_model.execute(id).map_err(|e| e.to_string())?.ok_or("no such model")?;
            print_models(std::slice::from_ref(&detail.summary));
            for file in &detail.files {
                println!(
                    "    {:<8} {:>12} B  {}  {}",
                    file.kind.code(),
                    file.size,
                    file.path,
                    file.variant.as_deref().unwrap_or("")
                );
            }
        }
        _ => return Err(USAGE.to_owned()),
    }
    eprintln!("took {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
    Ok(())
}

/// Needs no index: reads the 3MF directly.
fn print_plates(file: &str, binary: bool) -> Result<(), String> {
    let plates = hoarddeck_infra_mesh::read_plates(std::path::Path::new(file)).map_err(|e| e.to_string())?;
    if binary {
        return std::io::Write::write_all(&mut std::io::stdout(), &hoarddeck_infra_mesh::encode_plates(&plates))
            .map_err(|e| e.to_string());
    }
    for plate in &plates {
        println!("plate {}: {} triangles", plate.number, plate.positions.len() / 9);
    }
    Ok(())
}

fn print_models(models: &[ModelSummary]) {
    for model in models {
        let formats: Vec<_> = model.formats.iter().map(|f| f.code()).collect();
        let released = model.released.map(|r| r.to_string()).unwrap_or_default();
        println!(
            "{:>7}  {:<40} {:<8} {:<7} {} files  {}",
            model.id.0,
            model.title,
            formats.join(","),
            released,
            model.file_count,
            model.path
        );
    }
}

fn library_id(text: &str) -> Result<LibraryId, String> {
    text.parse().map(LibraryId).map_err(|_| format!("not a library id: {text}"))
}

fn take_option(args: &mut Vec<String>, name: &str) -> Result<Option<String>, String> {
    let Some(position) = args.iter().position(|arg| arg == name) else { return Ok(None) };
    args.remove(position);
    if position < args.len() { Ok(Some(args.remove(position))) } else { Err(format!("{name} needs a value")) }
}

fn take_flag(args: &mut Vec<String>, name: &str) -> bool {
    let before = args.len();
    args.retain(|arg| arg != name);
    args.len() != before
}

/// Prints at most one progress line per second while walking.
#[derive(Default)]
struct StderrProgress {
    last: Mutex<Option<Instant>>,
}

impl EventSink for StderrProgress {
    fn scan_progress(&self, progress: &ScanProgress) {
        let Ok(mut last) = self.last.lock() else { return };
        let due = last.is_none_or(|at| at.elapsed().as_secs() >= 1);
        if progress.phase != ScanPhase::Walking || due {
            eprintln!("{:?}: {} folders, {} entries", progress.phase, progress.dirs, progress.files);
            *last = Some(Instant::now());
        }
        if progress.phase == ScanPhase::Done && progress.failed > 0 {
            eprintln!("{} folders or files could not be read:", progress.failed);
            for failure in &progress.failures {
                eprintln!("  {failure}");
            }
        }
    }
}
