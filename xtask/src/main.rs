//! Asset renderer — regenerates the images in `assets/` directly from the
//! `rayflex` library (in-process, no subprocess per image).
//!
//! The `ASSETS` table below is the single source of truth for the per-scene
//! render parameters (resolution, samples, ray- vs path-tracing) that
//! previously lived scattered across README examples and the UI. Run it
//! whenever a change might affect the tracer's output:
//!
//!   cargo xtask                 # render every asset
//!   cargo xtask gold-gallery    # render just one
//!   cargo xtask --fast          # quick low-res/low-sample preview of all
//!
//! Note: renders are NOT deterministic (the sampler seeds from entropy), so
//! regenerated PNGs differ from the committed ones by Monte Carlo noise even
//! with no code change — a git diff is not a reliable "did output change"
//! signal without a seeded RNG.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use indicatif::{ProgressBar, ProgressStyle};
use rayflex::render::{RenderConfig, RenderJob};
use rayflex::scene::load_scene;

/// One rendered asset: `scenes/<scene>.json` -> `assets/<scene>.png`.
struct Asset {
    scene: &'static str,
    res_x: u32,
    res_y: u32,
    /// Path-tracing samples per pixel. `1` means classic ray tracing (the
    /// tracer path that uses spot/vec lights + adaptive antialiasing);
    /// `> 1` means Monte Carlo path tracing at that sample count.
    spp: u32,
}

/// Cap secondary-ray recursion. 8 is enough for the mirror-heavy scenes.
const REFLECTION_DEPTH: u32 = 8;

/// All assets share a fixed width (960) so they render at a uniform width in
/// the README; the height is the width divided by each scene's authored
/// aspect ratio (the camera FOV is derived from res_x/res_y, so the ratio
/// must match the composition or the shot re-frames). 960 is a multiple of
/// 24, so 1:1, 3:2, and 16:10 all yield round heights (960 / 640 / 600).
const ASSETS: &[Asset] = &[
    // Path-traced (emissive scenes) — spp drives noise; tone-map auto-applies.
    Asset { scene: "gold-gallery", res_x: 960, res_y: 600, spp: 800 },  // 16:10; was 1500 when its key light was wound backwards (NEE got nothing) -- now converges ~3.7x faster
    Asset { scene: "cornell-box", res_x: 960, res_y: 960, spp: 1500 },  // 1:1
    Asset { scene: "suzanne-bust", res_x: 960, res_y: 960, spp: 1200 }, // 1:1, mirror; was 2200 to fight the backwards-wound key light (now fixed -> NEE works)
    Asset { scene: "torus-knot", res_x: 960, res_y: 960, spp: 1500 },   // 1:1, diffuse + NEE -- converges faster
    Asset { scene: "toybox", res_x: 960, res_y: 720, spp: 600 },        // 4:3, textured diffuse toys in a bright studio -- converges fast
    Asset { scene: "rayflex-pt", res_x: 960, res_y: 640, spp: 2000 },   // 3:2
    // Ray-traced (spot/vec lit) — spp = 1, adaptive antialiasing on.
    Asset { scene: "teapot", res_x: 960, res_y: 960, spp: 1 },        // 1:1
    Asset { scene: "trolley", res_x: 960, res_y: 960, spp: 1 },       // 1:1
    Asset { scene: "buddha", res_x: 960, res_y: 960, spp: 1 },        // 1:1
    Asset { scene: "sphere-tunnel", res_x: 960, res_y: 960, spp: 1 }, // 1:1
    Asset { scene: "rayflex", res_x: 960, res_y: 640, spp: 1 },       // 3:2
    Asset { scene: "backpack", res_x: 960, res_y: 960, spp: 1 },      // 1:1, textured OBJ+MTL showcase
];

/// Repo root = the parent of this crate (xtask/..). Used to anchor the CWD so
/// scene/asset/OBJ relative paths resolve no matter where cargo is invoked.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("resolve repo root")
}

fn render(asset: &Asset, fast: bool) {
    // In fast mode: halve the resolution and cap path-traced scenes to a few
    // samples, for a quick composition/regression preview.
    let (res_x, res_y) = if fast {
        (asset.res_x / 2, asset.res_y / 2)
    } else {
        (asset.res_x, asset.res_y)
    };
    let ray_traced = asset.spp == 1;
    let spp = if fast && !ray_traced { 64 } else { asset.spp };

    let cfg = RenderConfig {
        path_tracing: spp,
        use_lines: false,
        // Adaptive antialiasing (ray-tracing only) needs the sample cache.
        use_hashmap: ray_traced,
        use_adaptive_sampling: ray_traced,
        // Match the original assets: gamma-encode the path-traced (HDR)
        // scenes, but not the ray-traced ones (their Phong/spot-light look
        // was authored without gamma).
        use_gamma: !ray_traced,
        adaptive_max_depth: 2,
        reflection_max_depth: REFLECTION_DEPTH,
        res_x,
        res_y,
        scene_file: PathBuf::from(format!("scenes/{}.json", asset.scene)),
        image_file: PathBuf::from(format!("assets/{}.png", asset.scene)),
    };

    let mode = if ray_traced {
        "ray".to_string()
    } else {
        format!("path {spp}spp")
    };
    println!("→ {} ({res_x}x{res_y}, {mode})", asset.scene);

    let start = Instant::now();
    let mut job: RenderJob = load_scene(cfg).expect("load scene");

    let bar = ProgressBar::new(1000);
    bar.set_style(
        ProgressStyle::with_template("  [{bar:40}] {percent}% {elapsed}")
            .unwrap()
            .progress_chars("=> "),
    );
    let bar_clone = bar.clone();
    job.set_progress_func(Box::new(move |pct| {
        bar_clone.set_position((pct * 1000.0) as u64);
    }));

    job.alloc_image();
    job.render_scene(Arc::new(AtomicBool::new(false)));
    bar.finish_and_clear();
    job.save_image().expect("save image");

    println!(
        "  assets/{}.png  ({:.1}s)",
        asset.scene,
        start.elapsed().as_secs_f32()
    );
}

fn main() {
    // Anchor at the repo root so `scenes/`, `assets/`, and the OBJ paths
    // embedded in scene files (which are CWD-relative) all resolve.
    std::env::set_current_dir(repo_root()).expect("chdir to repo root");

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: cargo xtask [scene] [--fast]");
        println!(
            "scenes: {}",
            ASSETS
                .iter()
                .map(|a| a.scene)
                .collect::<Vec<_>>()
                .join(", ")
        );
        return;
    }

    let fast = args.iter().any(|a| a == "--fast");
    let target = args.iter().find(|a| !a.starts_with('-'));

    let selected: Vec<&Asset> = match target {
        Some(name) => match ASSETS.iter().find(|a| a.scene == name) {
            Some(a) => vec![a],
            None => {
                eprintln!(
                    "unknown scene '{name}'. known: {}",
                    ASSETS
                        .iter()
                        .map(|a| a.scene)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                std::process::exit(1);
            }
        },
        None => ASSETS.iter().collect(),
    };

    println!(
        "rendering {} asset(s){}",
        selected.len(),
        if fast { " [fast]" } else { "" }
    );
    let start = Instant::now();
    for a in &selected {
        render(a, fast);
    }
    println!(
        "done: {} asset(s) in {:.1}s",
        selected.len(),
        start.elapsed().as_secs_f32()
    );
}
