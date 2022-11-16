use structopt::StructOpt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;
use std::fs;
use serde_json;
use rand::Rng;

mod render;
use render::RenderJob;


static CTRLC_HIT : AtomicBool = AtomicBool::new(false);

#[derive(StructOpt, Debug)]
#[structopt(name="rtest", about="minimal raytracer")]
struct Options {
    #[structopt(long, default_value = "pic.png")]
    img_file: PathBuf,
    #[structopt(long, default_value = "scene.json")]
    scene_file: PathBuf,
     #[structopt(short="x", long, default_value = "0")]
    res_x: u32,
     #[structopt(short="y", long, default_value = "0")]
    res_y: u32,
     #[structopt(short="n", long, default_value = "0")]
    num_spheres_to_generate: u32,
     #[structopt(short="a", long, default_value = "0")]
    adaptive_sampling: u8,
     #[structopt(long, default_value = "2")]
    adaptive_max_depth: u32,
     #[structopt(long, default_value = "4")]
    reflection_max_depth: u32,
     #[structopt(short="r", long, default_value = "1")]
    use_reflection: u32,
     #[structopt(short="g", long, default_value = "0")]
    use_gamma: u32,
     #[structopt(short="b", long, default_value = "1")]
    use_box: u32,
}


fn generate_scene(num_spheres_to_generate: u32, scene_file: PathBuf, use_box: bool) -> std::io::Result<()> {
    let mut rng = rand::thread_rng();
    let mut json: serde_json::Value;

    println!("Generating scene w/ {} spheres", num_spheres_to_generate);
    json = serde_json::json!({
        "resolution": [ 400, 400 ],
        "camera.position": [ -2.5, 0.0, 1.0 ],
        "camera.direction": [ 1, 0, -0.10 ],
        "ambient.color": [ 1, 1, 1 ],
        "ambient.intensity": 0.2,
        "num_vec_lights": 1,
        "num_spot_lights": 2,
        "vec-light.0.vector": [ 0.5, 0.5, -0.5],
        "vec-light.0.intensity": 1.5,
        "vec-light.0.color": [ 1, 1, 1],
        "spot-light.0.position": [ 0.5, 2.5, 1],
        "spot-light.0.intensity": 200,
        "spot-light.0.color": [ 0.4, 0.4, 0.7],
        "spot-light.1.position": [ 0.5, -2, 0],
        "spot-light.1.intensity": 80,
        "spot-light.1.color": [ 0.8, 0.3, 0.3],
        "sphere.0.center" : [3, 0, -0.5],
        "sphere.0.radius" : 1.3,
        "sphere.0.color": [ 0.8, 0.7, 0.9],
        "sphere.0.checkered": true,
        "sphere.0.reflectivity" : 0.5,
        "num_planes" : 0
    });
    json["num_spheres"] = serde_json::json!(num_spheres_to_generate);

    if use_box {
        println!("using box!");
        json["num_planes"]        = serde_json::json!(6);
        json["plane.0.position" ] = serde_json::json!([0, 0, -1]); // bottom
        json["plane.0.normal" ]   = serde_json::json!([0, 0, 1]);
        json["plane.0.reflectivity" ] = serde_json::json!(0.1);
        json["plane.1.position" ] = serde_json::json!([0, 0, 3]); // top
        json["plane.1.normal" ]   = serde_json::json!([0, 0, -1]);
        json["plane.2.position" ] = serde_json::json!([4.5, 0, 0]); // front
        json["plane.2.normal" ]   = serde_json::json!([-1, 0, 0]);
        json["plane.2.color"]     = serde_json::json!([ 0.5, 0.9, 0.5]);
        json["plane.3.position" ] = serde_json::json!([0, 3, 0]); // left
        json["plane.3.normal" ]   = serde_json::json!([0, -1, 0]);
        json["plane.3.color"]     = serde_json::json!([ 1, 0.2, 0.2]);
        json["plane.4.position" ] = serde_json::json!([0, -3, 0]); // right
        json["plane.4.normal" ]   = serde_json::json!([0, 1, 0]);
        json["plane.4.color"]     = serde_json::json!([ 0.5, 0.5, 1]);
        json["plane.5.position" ] = serde_json::json!([-3, 0, 0]); // back
        json["plane.5.normal" ]   = serde_json::json!([1, 0, 0]);
        json["plane.5.color"]     = serde_json::json!([ 1, 1, 1]);
    }

    let line = false;
    for i in 1..num_spheres_to_generate {
        let mut x = rng.gen_range(2.0..5.0);
        let mut y = rng.gen_range(-2.0..2.0);
        let mut z = rng.gen_range(-2.0..2.0);
        let mut r = rng.gen_range(0.2..0.4);
        if line {
            x = i as f64 * 2.0;
            y = -1.0;
            z = -0.5;
            r = 0.7;
        }
        let cr = rng.gen_range(0.3..1.0);
        let cg = rng.gen_range(0.3..1.0);
        let cb = rng.gen_range(0.3..1.0);
        let albedo = rng.gen_range(0.5..1.0);
        let reflectivity = rng.gen_range(0.0..1.0);
        let checkered = rng.gen_range(0..100) % 2;
        let name  = format!("sphere.{}.center", i);
        let rname = format!("sphere.{}.radius", i);
        let cname = format!("sphere.{}.color", i);
        let aname = format!("sphere.{}.albedo", i);
        let tname = format!("sphere.{}.checkered", i);
        let refname = format!("sphere.{}.reflectivity", i);
        json[name]  = serde_json::json!([x, y, z ]);
        json[rname] = serde_json::json!(r);
        json[cname] = serde_json::json!([cr, cg, cb]);
        json[aname] = serde_json::json!(albedo);
        json[tname] = serde_json::json!(checkered > 0);
        json[refname] = serde_json::json!(reflectivity);
    }
    let s0 = serde_json::to_string_pretty(&json)?;
    println!("Writing scene file {}", scene_file.display());
    return fs::write(&scene_file, s0);
}

fn print_opt(opt: &Options) {
    println!("gamma-correction: {} adaptive-sampling: {} max-depth: {}", opt.use_gamma, opt.adaptive_sampling, opt.adaptive_max_depth);
    println!("reflection: {} max-depth: {}", opt.use_reflection, opt.reflection_max_depth);
}

fn main() -> std::io::Result<()> {
    let opt = Options::from_args();

    let mut job = RenderJob::new(opt.reflection_max_depth, opt.use_reflection > 0, opt.adaptive_sampling > 0, opt.adaptive_max_depth, opt.res_x, opt.res_y, opt.use_gamma > 0);

     ctrlc::set_handler(move || {
         CTRLC_HIT.store(true, Ordering::SeqCst);
     })
     .expect("Error setting Ctrl-C handler");

    if opt.num_spheres_to_generate != 0 {
        return generate_scene(opt.num_spheres_to_generate, opt.scene_file, opt.use_box > 0);
    }

    print_opt(&opt);
    println!("num_threads: {}", rayon::current_num_threads());

    job.load_scene(opt.scene_file)?;
    job.render_scene();
    job.save_image(opt.img_file)?;

    Ok(())
}
