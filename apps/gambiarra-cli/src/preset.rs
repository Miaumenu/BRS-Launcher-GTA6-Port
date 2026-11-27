//! The `preset` command, and applying a preset to a run.
//!
//! A preset is chosen from what the driver reports (see [`crate::probe`]) and
//! applied by putting its settings in the environment before the title runs,
//! which is where the runtime already reads them from.
//!
//! What the user set by hand always wins. A preset is a good default for a
//! class of hardware, never an override of someone's explicit choice.

use anyhow::{Context, Result};
use clap::Subcommand;
use gambiarra_core::preset::{self, Capabilities, Preset};

use crate::probe;

#[derive(Subcommand)]
pub enum Command {
    /// List every preset, and mark the one this host would get.
    List,
    /// Show one preset in full, with the host's capabilities and why it
    /// matched. Without an id, the one this host would get.
    Show {
        /// The preset's id, as `list` shows it.
        id: Option<String>,
    },
}

pub fn run(command: Command) -> Result<()> {
    let dir = preset::presets_dir()?;
    let presets = preset::load_dir(&dir)?;
    let caps = probe::probe();
    match command {
        Command::List => list(&presets, &caps),
        Command::Show { id } => show(&presets, &caps, id.as_deref()),
    }
}

fn list(presets: &[Preset], caps: &Capabilities) -> Result<()> {
    println!("host        : {}", probe::describe(caps));
    let chosen = preset::choose_or_baseline(presets, caps).map(|(p, _)| p.id.as_str());
    match chosen {
        Some(id) => println!("this host   : {id}"),
        None => println!("this host   : (nothing matches and there is no baseline)"),
    }
    println!();
    for p in presets {
        let mark = if Some(p.id.as_str()) == chosen {
            "->"
        } else {
            "  "
        };
        let stage = p
            .measured
            .as_ref()
            .map_or("not measured".to_string(), |m| m.stage_reached.to_string());
        println!("{mark} {:<20} {:<14} {}", p.id, stage, p.name);
    }
    Ok(())
}

fn show(presets: &[Preset], caps: &Capabilities, id: Option<&str>) -> Result<()> {
    let chosen = match id {
        Some(id) => presets
            .iter()
            .find(|p| p.id == id)
            .with_context(|| format!("no preset called `{id}`; `preset list` shows them"))?,
        None => {
            let (p, _) = preset::choose_or_baseline(presets, caps).context(
                "nothing matches this host and there is no baseline preset to fall back to",
            )?;
            p
        }
    };
    println!("id          : {}", chosen.id);
    println!("name        : {}", chosen.name);
    if let Some(s) = &chosen.summary {
        println!(
            "summary     : {}",
            s.trim().replace('\n', "\n              ")
        );
    }
    if let Some(src) = &chosen.source {
        println!("file        : {}", src.display());
    }
    println!("host        : {}", probe::describe(caps));
    println!(
        "matches host: {}  ({} condition(s))",
        chosen.matches.holds(caps),
        chosen.matches.specificity()
    );
    if let Some(console) = chosen.run.console {
        println!("console     : {console}");
    }
    let env = chosen.tuning.env();
    if env.is_empty() {
        println!("settings    : (none)");
    } else {
        for (k, v) in &env {
            println!("settings    : {k}={v}");
        }
    }
    match &chosen.measured {
        Some(m) => {
            println!("measured on : {} ({})", m.gpu, m.date);
            println!("reached     : {}", m.stage_reached);
            println!(
                "fps         : {}",
                m.fps
                    .map_or("(the world never rendered)".to_string(), |f| f.to_string())
            );
            println!("report      : {}", m.report.display());
        }
        None => println!("measured on : (nothing; this preset describes no machine)"),
    }
    Ok(())
}

/// What applying a preset did, for the log line.
pub struct Applied {
    pub id: String,
    /// Whether `[match]` actually held, as opposed to falling back.
    pub matched: bool,
    /// The console the preset asks for, if any and if the user gave none.
    pub console: Option<preset::Console>,
    /// The variables this set, and the ones it left alone because the user
    /// had already set them.
    pub set: Vec<(&'static str, String)>,
    pub kept: Vec<&'static str>,
}

/// Chooses a preset for this host and puts its settings in the environment.
///
/// Must be called before anything reads those variables: the runtime reads
/// each one once, at first use, and caches it.
///
/// `wanted` forces one preset by id. `None` chooses from the host's
/// capabilities. A missing presets directory is not fatal — the run simply
/// gets no preset, which is what a binary outside a source tree should do.
pub fn apply(wanted: Option<&str>) -> Result<Option<Applied>> {
    let Ok(dir) = preset::presets_dir() else {
        return Ok(None);
    };
    if !dir.is_dir() {
        return Ok(None);
    }
    let presets = preset::load_dir(&dir)?;
    let caps = probe::probe();
    let (chosen, matched) = match wanted {
        Some(id) => (
            presets
                .iter()
                .find(|p| p.id == id)
                .with_context(|| format!("no preset called `{id}`; `preset list` shows them"))?,
            true,
        ),
        None => match preset::choose_or_baseline(&presets, &caps) {
            Some(found) => found,
            None => return Ok(None),
        },
    };
    let mut set = Vec::new();
    let mut kept = Vec::new();
    for (key, value) in chosen.tuning.env() {
        // The user's own choice outranks the preset's default.
        if std::env::var_os(key).is_some_and(|v| !v.is_empty()) {
            kept.push(key);
            continue;
        }
        // SAFETY: called once at start-up, before any thread is started and
        // before the runtime reads any of these variables.
        unsafe { std::env::set_var(key, &value) };
        set.push((key, value));
    }
    Ok(Some(Applied {
        id: chosen.id.clone(),
        matched,
        console: chosen.run.console,
        set,
        kept,
    }))
}

#[cfg(test)]
mod tests {
    use gambiarra_core::preset::{self, Capabilities, ShaderModel, Vendor};

    /// The repository's own presets must choose sensibly for a real host, so
    /// that `preset list` is never wrong about the machine in front of it.
    #[test]
    fn the_repositorys_presets_choose_the_gcn_preset_for_a_gcn_host() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../configs/presets");
        let presets = preset::load_dir(&dir).unwrap();
        let gcn = Capabilities {
            vendor: Some(Vendor::Amd),
            mesh_shader: Some(false),
            shader_model: Some(ShaderModel::new(6, 5)),
            wave_lanes_min: Some(64),
            vram_mb: Some(2027),
        };
        let (chosen, matched) = preset::choose_or_baseline(&presets, &gcn).unwrap();
        assert_eq!(chosen.id, "amd-gcn-no-mesh");
        assert!(matched);
        // And it carries settings worth applying.
        assert!(!chosen.tuning.env().is_empty());
    }
}
