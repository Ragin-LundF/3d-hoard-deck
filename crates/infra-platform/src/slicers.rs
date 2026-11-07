use std::path::Path;

use hoarddeck_application::ports::SlicerDetector;
use hoarddeck_domain::Slicer;

/// Built-in presets; only those installed at a default location are returned.
pub struct InstalledSlicers;

struct Preset {
    name: &'static str,
    extensions: &'static [&'static str],
    macos: &'static str,
    windows: &'static str,
    linux: &'static str,
}

const FDM: &[&str] = &["stl", "3mf", "obj", "step", "stp"];

const PRESETS: [Preset; 4] = [
    Preset {
        name: "Bambu Studio",
        extensions: FDM,
        macos: "/Applications/BambuStudio.app",
        windows: r"C:\Program Files\Bambu Studio\bambu-studio.exe",
        linux: "/usr/bin/bambu-studio",
    },
    Preset {
        name: "OrcaSlicer",
        extensions: FDM,
        macos: "/Applications/OrcaSlicer.app",
        windows: r"C:\Program Files\OrcaSlicer\orca-slicer.exe",
        linux: "/usr/bin/orca-slicer",
    },
    Preset {
        name: "PrusaSlicer",
        extensions: FDM,
        macos: "/Applications/PrusaSlicer.app",
        windows: r"C:\Program Files\Prusa3D\PrusaSlicer\prusa-slicer.exe",
        linux: "/usr/bin/prusa-slicer",
    },
    Preset {
        name: "Lychee Slicer",
        extensions: &["stl", "obj", "lys"],
        macos: "/Applications/LycheeSlicer.app",
        windows: r"C:\Program Files\LycheeSlicer\LycheeSlicer.exe",
        linux: "/usr/bin/lycheeslicer",
    },
];

impl SlicerDetector for InstalledSlicers {
    fn installed(&self) -> Vec<Slicer> {
        PRESETS
            .iter()
            .filter_map(|preset| {
                let program = if cfg!(target_os = "macos") {
                    preset.macos
                } else if cfg!(windows) {
                    preset.windows
                } else {
                    preset.linux
                };
                let extensions: Vec<String> = preset.extensions.iter().map(|e| (*e).to_owned()).collect();
                Path::new(program)
                    .exists()
                    .then(|| Slicer::new(preset.name, program, Vec::new(), &extensions).ok())
                    .flatten()
            })
            .collect()
    }
}
