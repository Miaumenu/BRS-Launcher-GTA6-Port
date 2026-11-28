//! Which settings a host gets: a preset is data, never code.
//!
//! A preset is one TOML file in `configs/presets/`. It says, for a class of
//! hardware, which settings to use — and it carries the evidence that someone
//! measured them. No preset ever creates a capability the driver did not
//! report: `[match]` only chooses between paths the host already allows.
//!
//! A preset is matched on **capabilities**, not on the vendor's name, because
//! hardware is not a tree. Two cards of different vendors can be alike for
//! this runtime and two of the same vendor can be worlds apart. See
//! `configs/presets/README.md` and `docs/gpu-portability.md`.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Who made the adapter, as the PCI vendor id names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Amd,
    Nvidia,
    Intel,
}

impl Vendor {
    /// The vendor of a PCI id, or `None` for one we do not name.
    pub fn from_pci_id(id: u32) -> Option<Self> {
        match id {
            0x1002 | 0x1022 => Some(Self::Amd),
            0x10DE => Some(Self::Nvidia),
            0x8086 => Some(Self::Intel),
            _ => None,
        }
    }
}

impl fmt::Display for Vendor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Amd => "amd",
            Self::Nvidia => "nvidia",
            Self::Intel => "intel",
        })
    }
}

/// A shader model, as `6.5`. Compared as a pair, so `6.10` is above `6.9`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShaderModel {
    pub major: u8,
    pub minor: u8,
}

impl ShaderModel {
    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }
}

impl fmt::Display for ShaderModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl std::str::FromStr for ShaderModel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let (major, minor) = s
            .split_once('.')
            .with_context(|| format!("a shader model reads as `6.6`, not `{s}`"))?;
        Ok(Self {
            major: major
                .trim()
                .parse()
                .with_context(|| format!("the major of `{s}`"))?,
            minor: minor
                .trim()
                .parse()
                .with_context(|| format!("the minor of `{s}`"))?,
        })
    }
}

impl<'de> Deserialize<'de> for ShaderModel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// What the driver reported about the host. Every field is optional because a
/// query can fail, and a capability we did not read is one we must not assume:
/// a preset that constrains an unknown capability does not match.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub vendor: Option<Vendor>,
    pub mesh_shader: Option<bool>,
    /// The highest shader model the device supports.
    pub shader_model: Option<ShaderModel>,
    /// `WaveLaneCountMin`: the narrowest wave the GPU runs.
    pub wave_lanes_min: Option<u32>,
    pub vram_mb: Option<u64>,
}

/// When a preset applies. An absent field means "does not matter"; a present
/// one must hold, and cannot hold when the capability was not read.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Match {
    pub vendor: Option<Vendor>,
    pub mesh_shader: Option<bool>,
    /// Holds when the host's highest shader model is at most this.
    pub shader_model_max: Option<ShaderModel>,
    /// Holds when the host's highest shader model is at least this.
    pub shader_model_min: Option<ShaderModel>,
    /// Holds when the host's narrowest wave is at least this many lanes.
    pub wave_lanes_min: Option<u32>,
    /// Holds when the host has less video memory than this.
    pub vram_mb_below: Option<u64>,
}

impl Match {
    /// How many conditions this names. The most specific match that holds
    /// wins, so a preset for one class of card beats a broad fallback.
    pub fn specificity(&self) -> usize {
        [
            self.vendor.is_some(),
            self.mesh_shader.is_some(),
            self.shader_model_max.is_some(),
            self.shader_model_min.is_some(),
            self.wave_lanes_min.is_some(),
            self.vram_mb_below.is_some(),
        ]
        .iter()
        .filter(|held| **held)
        .count()
    }

    /// Whether every condition holds for `caps`. A condition on a capability
    /// that was not read does not hold: we never guess hardware.
    pub fn holds(&self, caps: &Capabilities) -> bool {
        let known = |asked: Option<bool>| asked.unwrap_or(false);
        (match (self.vendor, caps.vendor) {
            (None, _) => true,
            (Some(want), Some(have)) => want == have,
            (Some(_), None) => false,
        }) && (match (self.mesh_shader, caps.mesh_shader) {
            (None, _) => true,
            (Some(want), Some(have)) => want == have,
            (Some(_), None) => false,
        }) && known(
            self.shader_model_max
                .map(|want| caps.shader_model.map(|have| have <= want))
                .map_or(Some(true), |held| held),
        ) && known(
            self.shader_model_min
                .map(|want| caps.shader_model.map(|have| have >= want))
                .map_or(Some(true), |held| held),
        ) && known(
            self.wave_lanes_min
                .map(|want| caps.wave_lanes_min.map(|have| have >= want))
                .map_or(Some(true), |held| held),
        ) && known(
            self.vram_mb_below
                .map(|want| caps.vram_mb.map(|have| have < want))
                .map_or(Some(true), |held| held),
        )
    }
}

/// The console model the title sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Console {
    SeriesX,
    SeriesS,
}

impl fmt::Display for Console {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SeriesX => "series-x",
            Self::SeriesS => "series-s",
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub console: Option<Console>,
}

/// The display the console reports to the title, `"<width>x<height>"`: as a
/// console plugged into a television of that size, the title renders and
/// upscales for it. The runtime takes 640x360 up to 7680x4320.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplaySize {
    pub width: u32,
    pub height: u32,
}

impl<'de> Deserialize<'de> for DisplaySize {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        let parsed = text.trim().split_once(['x', 'X']).and_then(|(w, h)| {
            Some(DisplaySize {
                width: w.trim().parse().ok()?,
                height: h.trim().parse().ok()?,
            })
        });
        match parsed {
            Some(s) if (640..=7680).contains(&s.width) && (360..=4320).contains(&s.height) => Ok(s),
            _ => Err(serde::de::Error::custom(format!(
                "display_size {text:?}: expected <width>x<height> between 640x360 and 7680x4320"
            ))),
        }
    }
}

impl fmt::Display for DisplaySize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// The size the title renders at inside, `"<width>x<height>"`, below the
/// 1920x1080 it picks on its own (`GAMBIARRA_RENDER_SIZE`). The runtime takes
/// 640x360 up to 1920x1080, both multiples of 8, and only on the one build
/// its patches were read from (else it says so and leaves the title as is).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSize {
    pub width: u32,
    pub height: u32,
}

impl<'de> Deserialize<'de> for RenderSize {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        let parsed = text.trim().split_once(['x', 'X']).and_then(|(w, h)| {
            Some(RenderSize {
                width: w.trim().parse().ok()?,
                height: h.trim().parse().ok()?,
            })
        });
        match parsed {
            Some(s)
                if (640..=1920).contains(&s.width)
                    && (360..=1080).contains(&s.height)
                    && s.width.is_multiple_of(8)
                    && s.height.is_multiple_of(8) =>
            {
                Ok(s)
            }
            _ => Err(serde::de::Error::custom(format!(
                "render_size {text:?}: expected <width>x<height> between 640x360 and 1920x1080, multiples of 8"
            ))),
        }
    }
}

impl fmt::Display for RenderSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// The settings a preset chooses. Each one is a switch the runtime already
/// reads from the environment; a preset sets them together instead of by hand.
///
/// Every field here must have an implementation. A setting with nowhere to go
/// belongs in the schema's prose, not in this struct, or a preset would
/// promise something the runtime ignores.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tuning {
    /// `GAMBIARRA_AMBIENT=<share>`: estimated indirect light, for nearly
    /// nothing; 1 is all of it. The `constant:<level>` form is not
    /// expressible here on purpose — a preset picks a share, not a debug aid.
    pub ambient: Option<f32>,
    /// `GAMBIARRA_BVH_BUILDER`: the real ray-traced structures. Costs about
    /// 40 ms a frame on the one card measured; see `docs/fork-ray-tracing.md`.
    pub bvh_builder: Option<bool>,
    /// `GAMBIARRA_TEXTURE_BUDGET_MB`.
    pub texture_budget_mb: Option<u32>,
    /// `GAMBIARRA_LEVELS_ON_DEMAND`.
    pub levels_on_demand: Option<bool>,
    /// `GAMBIARRA_DISPLAY_MODE` (the reported display's size; unset: 3840x2160).
    pub display_size: Option<DisplaySize>,
    /// `GAMBIARRA_RENDER_SIZE` (the size rendered at inside; unset: the title's own).
    pub render_size: Option<RenderSize>,
}

impl Tuning {
    /// The environment this tuning means, in the order a reader expects.
    ///
    /// The runtime reads each of these once, at first use, so they must be in
    /// the environment before the title runs — not after.
    pub fn env(&self) -> BTreeMap<&'static str, String> {
        let mut out = BTreeMap::new();
        if let Some(v) = self.ambient {
            out.insert("GAMBIARRA_AMBIENT", v.to_string());
        }
        if let Some(v) = self.bvh_builder {
            out.insert("GAMBIARRA_BVH_BUILDER", u8::from(v).to_string());
        }
        if let Some(v) = self.texture_budget_mb {
            out.insert("GAMBIARRA_TEXTURE_BUDGET_MB", v.to_string());
        }
        if let Some(v) = self.levels_on_demand {
            out.insert("GAMBIARRA_LEVELS_ON_DEMAND", u8::from(v).to_string());
        }
        if let Some(v) = self.display_size {
            out.insert("GAMBIARRA_DISPLAY_MODE", v.to_string());
        }
        if let Some(v) = self.render_size {
            out.insert("GAMBIARRA_RENDER_SIZE", v.to_string());
        }
        out
    }
}

/// How far a run got. The ladder is the one contributors have always used;
/// it is what most machines can report, since most will not reach a frame
/// rate at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Builds, and nothing beyond that was tried.
    Compiles,
    /// Creates the D3D12 device.
    Device,
    /// Presents at least one frame.
    Frame,
    /// Draws the loading art.
    LoadingArt,
    Menu,
    /// Renders the world: the first frame of 500 draws or more.
    World,
    Playable,
}

impl Stage {
    /// Whether the world rendered, which is what makes a frame rate mean
    /// anything. Below this a frame rate measures an empty screen.
    pub fn renders_world(self) -> bool {
        self >= Self::World
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Compiles => "compiles",
            Self::Device => "device",
            Self::Frame => "frame",
            Self::LoadingArt => "loading-art",
            Self::Menu => "menu",
            Self::World => "world",
            Self::Playable => "playable",
        })
    }
}

/// The evidence behind a preset. Without it a preset is someone's guess, so
/// `report` is required and must name a file that exists.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measured {
    pub gpu: String,
    pub vram_mb: Option<u64>,
    pub driver: Option<String>,
    pub os: Option<String>,
    pub stage_reached: Stage,
    /// Frames a second, once the world renders. Refused below that stage.
    pub fps: Option<f32>,
    /// The report in `reports/hardware/`, relative to the repository root.
    pub report: PathBuf,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Head {
    id: String,
    name: String,
    summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    preset: Head,
    #[serde(default)]
    r#match: Match,
    #[serde(default)]
    run: Run,
    #[serde(default)]
    tuning: Tuning,
    measured: Option<Measured>,
}

/// One preset, as read from one file.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub summary: Option<String>,
    pub matches: Match,
    pub run: Run,
    pub tuning: Tuning,
    pub measured: Option<Measured>,
    /// The file this came from, for error messages.
    pub source: Option<PathBuf>,
}

impl Preset {
    /// Reads one preset from TOML. `stem` is the file name without its
    /// extension, which the id must equal so that a preset is findable by
    /// the name it is referred to by.
    pub fn parse(text: &str, stem: Option<&str>) -> Result<Self> {
        let file: File = toml::from_str(text)?;
        let preset = Self {
            id: file.preset.id,
            name: file.preset.name,
            summary: file.preset.summary,
            matches: file.r#match,
            run: file.run,
            tuning: file.tuning,
            measured: file.measured,
            source: None,
        };
        preset.check(stem)?;
        Ok(preset)
    }

    /// What a preset must satisfy to be worth trusting.
    fn check(&self, stem: Option<&str>) -> Result<()> {
        if self.id.is_empty() {
            bail!("a preset needs an id");
        }
        if !self
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            bail!(
                "the id `{}` should be lowercase letters, digits and dashes, so that it is also \
                 a usable file name",
                self.id
            );
        }
        if let Some(stem) = stem
            && stem != self.id
        {
            bail!(
                "the preset in `{stem}.toml` calls itself `{}`; the id and the file name must \
                 agree",
                self.id
            );
        }
        // The rule that keeps invented numbers out: a frame rate below the
        // world is a frame rate of an empty screen.
        if let Some(m) = &self.measured
            && m.fps.is_some()
            && !m.stage_reached.renders_world()
        {
            bail!(
                "`{}` reports a frame rate but only reached `{}`; a frame rate counts from \
                 `world` up, because below it nothing of the world is drawn",
                self.id,
                m.stage_reached
            );
        }
        Ok(())
    }

    /// Reads one preset file.
    pub fn read(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let stem = path.file_stem().and_then(|s| s.to_str());
        let mut preset =
            Self::parse(&text, stem).with_context(|| format!("in {}", path.display()))?;
        preset.source = Some(path.to_path_buf());
        Ok(preset)
    }
}

/// Reads every `*.toml` in `dir`, skipping the schema and any file whose name
/// starts with `_`, sorted by id so that the order never depends on the
/// file system.
pub fn load_dir(dir: &Path) -> Result<Vec<Preset>> {
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("reading the presets in {}", dir.display()))?;
    let mut out = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("listing {}", dir.display()))?
            .path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.starts_with('_') || path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        out.push(Preset::read(&path)?);
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    if let Some(dup) = first_duplicate(&out) {
        bail!("two presets in {} call themselves `{dup}`", dir.display());
    }
    Ok(out)
}

fn first_duplicate(presets: &[Preset]) -> Option<&str> {
    presets
        .windows(2)
        .find(|pair| pair[0].id == pair[1].id)
        .map(|pair| pair[0].id.as_str())
}

/// The preset for a host: of those whose conditions all hold, the one that
/// names the most conditions. A tie goes to the first by id, so the choice is
/// the same on every machine.
///
/// Returns `None` when nothing holds; the caller falls back to `baseline`.
pub fn choose<'a>(presets: &'a [Preset], caps: &Capabilities) -> Option<&'a Preset> {
    presets
        .iter()
        .filter(|p| p.matches.holds(caps))
        .max_by_key(|p| (p.matches.specificity(), std::cmp::Reverse(p.id.as_str())))
}

/// The preset to use: the best match, else the one called `baseline`.
pub fn choose_or_baseline<'a>(
    presets: &'a [Preset],
    caps: &Capabilities,
) -> Option<(&'a Preset, bool)> {
    if let Some(found) = choose(presets, caps) {
        return Some((found, true));
    }
    presets
        .iter()
        .find(|p| p.id == "baseline")
        .map(|p| (p, false))
}

/// The source tree holding `path`, found by walking up to the marker file
/// [`crate::policy::ROOT_MARKER`]. Returns `None` outside a source tree.
fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(crate::policy::ROOT_MARKER).is_file())
        .map(Path::to_path_buf)
}

/// Where the presets live: `GAMBIARRA_PRESETS`, else `configs/presets` in the
/// source tree of the current directory, else of this binary.
///
/// A binary installed outside a source tree has no presets to read, which is
/// not an error for the caller to hide: the baseline is then the only answer,
/// and saying so beats pretending a host was matched.
pub fn presets_dir() -> Result<PathBuf> {
    if let Some(set) = std::env::var_os("GAMBIARRA_PRESETS").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(set));
    }
    let from_cwd = std::env::current_dir().ok().and_then(|d| find_root(&d));
    let from_exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().and_then(find_root));
    from_cwd
        .or(from_exe)
        .map(|root| root.join("configs").join("presets"))
        .context("cannot find configs/presets: run from the source tree or set GAMBIARRA_PRESETS")
}

#[cfg(test)]
mod tests {
    use super::*;

    const RX550: Capabilities = Capabilities {
        vendor: Some(Vendor::Amd),
        mesh_shader: Some(false),
        shader_model: Some(ShaderModel::new(6, 5)),
        wave_lanes_min: Some(64),
        vram_mb: Some(2027),
    };

    const RDNA2: Capabilities = Capabilities {
        vendor: Some(Vendor::Amd),
        mesh_shader: Some(true),
        shader_model: Some(ShaderModel::new(6, 6)),
        wave_lanes_min: Some(32),
        vram_mb: Some(8192),
    };

    fn preset(id: &str, matches: Match) -> Preset {
        Preset {
            id: id.to_string(),
            name: id.to_string(),
            summary: None,
            matches,
            run: Run::default(),
            tuning: Tuning::default(),
            measured: None,
            source: None,
        }
    }

    #[test]
    fn shader_model_compares_by_pair_not_by_text() {
        // As text, "6.10" sorts below "6.9"; as a model it is above it.
        let ten: ShaderModel = "6.10".parse().unwrap();
        let nine: ShaderModel = "6.9".parse().unwrap();
        assert!(ten > nine);
    }

    #[test]
    fn a_shader_model_needs_both_halves() {
        assert!("6".parse::<ShaderModel>().is_err());
        assert!("six.five".parse::<ShaderModel>().is_err());
        assert_eq!(
            "6.5".parse::<ShaderModel>().unwrap(),
            ShaderModel::new(6, 5)
        );
    }

    #[test]
    fn an_empty_match_holds_for_anything() {
        assert!(Match::default().holds(&RX550));
        assert!(Match::default().holds(&Capabilities::default()));
        assert_eq!(Match::default().specificity(), 0);
    }

    #[test]
    fn a_condition_on_an_unread_capability_does_not_hold() {
        // The whole point: we never guess at hardware we could not query.
        let asks_mesh = Match {
            mesh_shader: Some(false),
            ..Default::default()
        };
        assert!(asks_mesh.holds(&RX550));
        assert!(!asks_mesh.holds(&Capabilities::default()));

        let asks_lanes = Match {
            wave_lanes_min: Some(64),
            ..Default::default()
        };
        assert!(!asks_lanes.holds(&Capabilities::default()));
    }

    #[test]
    fn the_gcn_match_holds_for_the_measured_card_and_not_for_rdna2() {
        let gcn = Match {
            vendor: Some(Vendor::Amd),
            mesh_shader: Some(false),
            shader_model_max: Some(ShaderModel::new(6, 5)),
            wave_lanes_min: Some(64),
            ..Default::default()
        };
        assert!(gcn.holds(&RX550));
        assert!(!gcn.holds(&RDNA2));
    }

    #[test]
    fn the_most_specific_match_wins() {
        let presets = vec![
            preset("anything", Match::default()),
            preset(
                "amd-anything",
                Match {
                    vendor: Some(Vendor::Amd),
                    ..Default::default()
                },
            ),
            preset(
                "amd-gcn-no-mesh",
                Match {
                    vendor: Some(Vendor::Amd),
                    mesh_shader: Some(false),
                    wave_lanes_min: Some(64),
                    ..Default::default()
                },
            ),
        ];
        assert_eq!(choose(&presets, &RX550).unwrap().id, "amd-gcn-no-mesh");
        // RDNA2 fails the mesh and lane conditions, so the vendor one wins.
        assert_eq!(choose(&presets, &RDNA2).unwrap().id, "amd-anything");
    }

    #[test]
    fn a_tie_is_broken_by_id_so_every_machine_chooses_alike() {
        let a = Match {
            vendor: Some(Vendor::Amd),
            ..Default::default()
        };
        let b = Match {
            mesh_shader: Some(false),
            ..Default::default()
        };
        let one = vec![preset("aaa", a.clone()), preset("zzz", b.clone())];
        let other = vec![preset("zzz", b), preset("aaa", a)];
        assert_eq!(choose(&one, &RX550).unwrap().id, "aaa");
        assert_eq!(choose(&other, &RX550).unwrap().id, "aaa");
    }

    #[test]
    fn nothing_matching_falls_back_to_baseline() {
        let presets = vec![
            preset("baseline", Match::default()),
            preset(
                "nvidia-only",
                Match {
                    vendor: Some(Vendor::Nvidia),
                    ..Default::default()
                },
            ),
        ];
        let intel = Capabilities {
            vendor: Some(Vendor::Intel),
            ..Default::default()
        };
        let (chosen, matched) = choose_or_baseline(&presets, &intel).unwrap();
        assert_eq!(chosen.id, "baseline");
        assert!(matched, "baseline has an empty match, so it really matched");

        // With no baseline and nothing matching, there is nothing to use.
        let only_nvidia = vec![preset(
            "nvidia-only",
            Match {
                vendor: Some(Vendor::Nvidia),
                ..Default::default()
            },
        )];
        assert!(choose_or_baseline(&only_nvidia, &intel).is_none());
    }

    #[test]
    fn tuning_becomes_the_environment_the_runtime_reads() {
        let tuning = Tuning {
            ambient: Some(1.0),
            bvh_builder: Some(false),
            display_size: Some(DisplaySize {
                width: 1280,
                height: 720,
            }),
            ..Default::default()
        };
        let env = tuning.env();
        assert_eq!(env["GAMBIARRA_AMBIENT"], "1");
        assert_eq!(env["GAMBIARRA_BVH_BUILDER"], "0");
        // The size the display side reads (`gambiarra_gpu::dxgi`), not a window mode.
        assert_eq!(env["GAMBIARRA_DISPLAY_MODE"], "1280x720");
        // Nothing is invented for the fields that were not set.
        assert!(!env.contains_key("GAMBIARRA_TEXTURE_BUDGET_MB"));
        assert_eq!(Tuning::default().env().len(), 0);
    }

    /// A render size the loader would not take is refused when the preset is read.
    #[test]
    fn a_render_size_must_be_one_the_loader_takes() {
        let with = |size: &str| {
            let text =
                format!("[preset]\nid = \"x\"\nname = \"X\"\n[tuning]\nrender_size = \"{size}\"\n");
            Preset::parse(&text, Some("x"))
        };
        let tuning = with("1280x720").unwrap().tuning;
        assert_eq!(tuning.env()["GAMBIARRA_RENDER_SIZE"], "1280x720");
        for bad in ["2560x1440", "1281x720", "320x200", "720p"] {
            assert!(with(bad).is_err(), "{bad} was taken");
        }
    }

    /// A display size that the display side would not take is refused when the
    /// preset is read, not passed on as a value it ignores.
    #[test]
    fn a_display_size_must_be_one_the_runtime_takes() {
        let with = |size: &str| {
            let text = format!(
                "[preset]\nid = \"x\"\nname = \"X\"\n[tuning]\ndisplay_size = \"{size}\"\n"
            );
            Preset::parse(&text, Some("x"))
        };
        let tuning = with("1280x720").unwrap().tuning;
        assert_eq!(tuning.env()["GAMBIARRA_DISPLAY_MODE"], "1280x720");
        for bad in ["borderless", "320x200", "1280", "8000x5000"] {
            assert!(with(bad).is_err(), "{bad} was taken");
        }
    }

    #[test]
    fn an_unknown_field_is_refused_rather_than_silently_ignored() {
        let text = r#"
            [preset]
            id = "x"
            name = "X"
            [tuning]
            ray_tracing = true
        "#;
        let err = Preset::parse(text, Some("x")).unwrap_err().to_string();
        assert!(err.contains("ray_tracing"), "{err}");
    }

    #[test]
    fn the_id_must_match_the_file_name() {
        let text = r#"
            [preset]
            id = "one-thing"
            name = "One"
        "#;
        assert!(Preset::parse(text, Some("one-thing")).is_ok());
        let err = Preset::parse(text, Some("another"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("must"), "{err}");
    }

    #[test]
    fn an_id_with_spaces_or_capitals_is_refused() {
        let text = r#"
            [preset]
            id = "Not An Id"
            name = "X"
        "#;
        assert!(Preset::parse(text, None).is_err());
    }

    #[test]
    fn a_frame_rate_without_the_world_is_refused() {
        let text = r#"
            [preset]
            id = "x"
            name = "X"
            [measured]
            gpu = "Some Card"
            stage_reached = "loading-art"
            fps = 60.0
            report = "reports/hardware/x.md"
            date = "2026-11-27"
        "#;
        let err = Preset::parse(text, Some("x")).unwrap_err().to_string();
        assert!(err.contains("frame rate"), "{err}");
    }

    #[test]
    fn a_frame_rate_from_the_world_up_is_allowed() {
        let text = r#"
            [preset]
            id = "x"
            name = "X"
            [measured]
            gpu = "Some Card"
            stage_reached = "world"
            fps = 31.5
            report = "reports/hardware/x.md"
            date = "2026-11-27"
        "#;
        let parsed = Preset::parse(text, Some("x")).unwrap();
        assert_eq!(parsed.measured.unwrap().fps, Some(31.5));
    }

    #[test]
    fn no_frame_rate_is_fine_at_any_stage() {
        let text = r#"
            [preset]
            id = "x"
            name = "X"
            [measured]
            gpu = "Some Card"
            stage_reached = "compiles"
            report = "reports/hardware/x.md"
            date = "2026-11-27"
        "#;
        assert!(Preset::parse(text, Some("x")).is_ok());
    }

    #[test]
    fn the_ladder_knows_where_a_frame_rate_starts_counting() {
        assert!(!Stage::LoadingArt.renders_world());
        assert!(!Stage::Menu.renders_world());
        assert!(Stage::World.renders_world());
        assert!(Stage::Playable.renders_world());
        assert!(Stage::Menu > Stage::Frame);
    }

    #[test]
    fn vendors_come_from_the_pci_id() {
        assert_eq!(Vendor::from_pci_id(0x1002), Some(Vendor::Amd));
        assert_eq!(Vendor::from_pci_id(0x10DE), Some(Vendor::Nvidia));
        assert_eq!(Vendor::from_pci_id(0x8086), Some(Vendor::Intel));
        assert_eq!(Vendor::from_pci_id(0x1234), None);
    }

    #[test]
    fn every_preset_in_the_repository_is_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../configs/presets");
        let presets = load_dir(&dir).expect("the repository's presets must parse");
        assert!(
            presets.iter().any(|p| p.id == "baseline"),
            "there must be a baseline for hosts nothing else matches"
        );
        for p in &presets {
            // Evidence must exist, or the preset is a guess with a citation.
            if let Some(m) = &p.measured {
                let report = dir.join("../..").join(&m.report);
                assert!(
                    report.is_file(),
                    "`{}` cites {}, which is not in the repository",
                    p.id,
                    m.report.display()
                );
            }
        }
    }

    #[test]
    fn the_schema_is_a_valid_preset_too() {
        // So the gabarito cannot drift from what the parser accepts.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../configs/presets/_schema.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        Preset::parse(&text, None).expect("the schema must parse as a preset");
    }

    #[test]
    fn the_measured_card_chooses_its_own_preset_from_the_repository() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../configs/presets");
        let presets = load_dir(&dir).unwrap();
        let (chosen, matched) = choose_or_baseline(&presets, &RX550).unwrap();
        assert!(matched);
        assert_eq!(chosen.id, "amd-gcn-no-mesh");
    }

    #[test]
    fn an_unknown_host_falls_back_to_the_repositorys_baseline() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../configs/presets");
        let presets = load_dir(&dir).unwrap();
        let (chosen, _) = choose_or_baseline(&presets, &Capabilities::default()).unwrap();
        assert_eq!(chosen.id, "baseline");
    }
}
