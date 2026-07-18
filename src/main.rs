use clap::Parser;
use colored::Colorize;
use indicatif::ProgressBar;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use rayflex::render::RenderConfig;
use rayflex::scene::load_scene;

#[derive(Parser, Debug)]
#[command(name = "rayflex", about = "ray/path-tracer")]
struct Options {
    #[arg(long, default_value = "pic.png")]
    img_file: PathBuf,
    #[arg(short = 'l', long, default_value = "scene.json")]
    scene_file: PathBuf,
    #[arg(short = 'x', long, default_value = "0")]
    res_x: u32,
    #[arg(short = 'y', long, default_value = "0")]
    res_y: u32,
    #[arg(long, default_value = "2")]
    adaptive_max_depth: u32,
    #[arg(long, default_value = "6")]
    reflection_max_depth: u32,
    #[arg(short = 'g', long, help = "use gamma correction")]
    use_gamma: bool,
    #[arg(short = 'a', long)]
    use_adaptive_sampling: bool,
    #[arg(long, help = "scan per line vs box")]
    use_lines: bool,
    #[arg(long, help = "use hashmap to speed-up antialiasing")]
    use_hashmap: bool,
    #[arg(short = 'p', long, help = "do path tracing", default_value = "1")]
    path_tracing: u32,
    #[arg(short = 'u', long, help = "use ui")]
    use_ui: bool,
}

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

fn main() -> std::io::Result<()> {
    env_logger::init();

    let opt = Options::parse();
    let exit_req = Arc::new(AtomicBool::new(false));
    let exit_req_clone = exit_req.clone();

    ctrlc::set_handler(move || {
        exit_req_clone.store(true, Ordering::SeqCst);
    })
    .expect("ctrl-c");

    if opt.use_ui {
        rayflex::egui_main();
        return Ok(());
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
