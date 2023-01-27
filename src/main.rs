#[cfg(not(target_arch = "wasm32"))]
use colored::Colorize;
#[cfg(not(target_arch = "wasm32"))]
use indicatif::ProgressBar;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use structopt::StructOpt;

use raymax::render::RenderConfig;
use raymax::scene::generate_scene;
use raymax::scene::load_scene;

#[cfg(not(target_arch = "wasm32"))]
#[derive(StructOpt, Debug)]
#[structopt(name = "rtest", about = "minimal raytracer")]
struct Options {
    #[structopt(long, default_value = "pic.png")]
    img_file: PathBuf,
    #[structopt(short = "l", long, default_value = "scene.json")]
    scene_file: PathBuf,
    #[structopt(short = "x", long, default_value = "0")]
    res_x: u32,
    #[structopt(short = "y", long, default_value = "0")]
    res_y: u32,
    #[structopt(short = "n", long, default_value = "0")]
    num_spheres_to_generate: u32,
    #[structopt(long, default_value = "2")]
    adaptive_max_depth: u32,
    #[structopt(long, default_value = "6")]
    reflection_max_depth: u32,
    #[structopt(short = "b", long, default_value = "1")]
    add_box: u32,
    #[structopt(short = "g", long, help = "use gamma correction")]
    use_gamma: bool,
    #[structopt(short = "a", long)]
    use_adaptive_sampling: bool,
    #[structopt(long, help = "scan per line vs box")]
    use_lines: bool,
    #[structopt(long, help = "use hashmap to speed-up antialiasing")]
    use_hashmap: bool,
    #[structopt(short = "-p", long, help = "do path tracing", default_value = "1")]
    path_tracing: u32,
    #[structopt(short = "-u", long, help = "use ui")]
    use_ui: bool,
}

#[cfg(not(target_arch = "wasm32"))]
fn print_opt(opt: &Options) {
    println!(
        "{}: gamma={} sampling-depth={} reflection-depth={}",
        "option".yellow(),
        opt.use_gamma,
        opt.adaptive_max_depth,
        opt.reflection_max_depth,
    );
    println!(
        "{}: lines={} hashmap={} path_tracing={}",
        "option".yellow(),
        opt.use_lines,
        opt.use_hashmap,
        opt.path_tracing,
    );
    let s = format!("num_threads: {}", rayon::current_num_threads()).red();
    println!("{s}");
}

#[cfg(target_arch = "wasm32")]
fn main() -> std::io::Result<()> {
    raymax::egui_main();
    return Ok(());
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let opt = Options::from_args();
    let exit_req = Arc::new(AtomicBool::new(false));
    let exit_req_clone = exit_req.clone();

    ctrlc::set_handler(move || {
        exit_req_clone.store(true, Ordering::SeqCst);
    })
    .expect("ctrl-c");

    if opt.use_ui {
        raymax::egui_main();
        return Ok(());
    }

    if opt.num_spheres_to_generate != 0 {
        return generate_scene(opt.num_spheres_to_generate, opt.scene_file, opt.add_box > 0);
    }

    print_opt(&opt);

    let cfg = RenderConfig {
        use_adaptive_sampling: opt.use_adaptive_sampling,
        use_gamma: opt.use_gamma,
        reflection_max_depth: opt.reflection_max_depth,
        adaptive_max_depth: opt.adaptive_max_depth,
        res_x: opt.res_x,
        res_y: opt.res_y,
        use_lines: opt.use_lines,
        use_hashmap: opt.use_hashmap,
        path_tracing: opt.path_tracing,
        scene_file: opt.scene_file,
        image_file: opt.img_file,
    };

    let res = load_scene(cfg);
    let mut job = res.unwrap();

    let pb = Arc::new(ProgressBar::new(1000));
    let pb_clone = pb.clone();
    job.set_progress_func(Box::new(move |pct| {
                pb_clone.set_position((pct * 1000.0) as u64);
    }));
    job.alloc_image();
    job.render_scene(exit_req);
    pb.finish_and_clear();
    job.print_stats();
    job.save_image()?;

    Ok(())
}
