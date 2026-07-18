use clap::Parser;
use colored::Colorize;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

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
        "{}: hashmap={} path_tracing={}",
        "option".yellow(),
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
        use_hashmap: opt.use_hashmap,
        path_tracing: opt.path_tracing,
        scene_file: opt.scene_file,
        image_file: opt.img_file,
    };

    let res = load_scene(cfg);
    let mut job = res.unwrap();
    job.alloc_image();

    // CLI progress visibility on long renders (AGENTS.md item): the
    // progress callback fires from render worker threads; use it for
    //   (a) a periodic flush of the in-progress image to --img-file, so a
    //       multi-minute/hour render can be eyeballed mid-way, and
    //   (b) a percent/elapsed stdout line every 10%, so non-TTY runs
    //       (nohup, piped logs -- where indicatif hides itself) still
    //       show the render is alive. The indicatif bar itself covers the
    //       TTY case with percent/elapsed/ETA.
    // The flush holds the image lock for one PNG encode (~ms) per minute
    // -- negligible against the render worker that also locks per pixel.
    const FLUSH_INTERVAL: Duration = Duration::from_secs(60);
    let flush_img = job.image.clone();
    let flush_path = job.cfg.image_file.clone();
    let last_flush = Arc::new(Mutex::new(Instant::now()));
    let last_log_pct = Arc::new(Mutex::new(0u32));
    let render_start = Instant::now();

    let pb = Arc::new(ProgressBar::new(1000));
    pb.set_style(
        ProgressStyle::with_template("{percent}% {wide_bar} [{elapsed_precise}] [{eta_precise}]")
            .unwrap(),
    );
    let pb_clone = pb.clone();
    job.set_progress_func(Box::new(move |pct| {
        pb_clone.set_position((pct * 1000.0) as u64);

        {
            let mut last = last_flush.lock().unwrap();
            if last.elapsed() >= FLUSH_INTERVAL {
                *last = Instant::now();
                drop(last);
                if let Err(e) = flush_img.lock().unwrap().save_image(&flush_path) {
                    eprintln!("progress image flush failed: {e}");
                }
            }
        }

        // Suppress the decile chatter on renders faster than 5s -- fast
        // renders don't need a heartbeat, and this keeps test output clean.
        let pct100 = (pct * 100.0) as u32;
        let mut logged = last_log_pct.lock().unwrap();
        if pct100 >= *logged + 10 && render_start.elapsed().as_secs() >= 5 {
            *logged = pct100 - pct100 % 10;
            println!(
                "progress: {:3}% -- {:.1?} elapsed",
                *logged,
                render_start.elapsed()
            );
        }
    }));
    job.render_scene(exit_req);
    pb.finish_and_clear();
    job.print_stats();
    job.save_image()?;

    Ok(())
}
