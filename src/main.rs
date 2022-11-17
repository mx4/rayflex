use structopt::StructOpt;
use colored::Colorize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;
use std::fs;
use serde_json;
use rand::Rng;

use raymax::vec3::Point;
use raymax::vec3::Vec3;
use raymax::color::RGB;
use raymax::camera::Camera;
use raymax::light::VectorLight;
use raymax::light::SpotLight;
use raymax::light::AmbientLight;
use raymax::three_d::Triangle;
use raymax::three_d::Material;

mod render;
use render::RenderJob;
use render::RenderConfig;


static CTRLC_HIT : AtomicBool = AtomicBool::new(false);

#[derive(StructOpt, Debug)]
#[structopt(name="rtest", about="minimal raytracer")]
struct Options {
    #[structopt(long, default_value = "pic.png")]
    img_file: PathBuf,
    #[structopt(short="l", long, default_value = "scene.json")]
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
     #[structopt(long, default_value = "6")]
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
        "num_vec_lights": 1,
        "num_spot_lights": 2,
        "sphere.0.center" : [3, 0, -0.5],
        "sphere.0.radius" : 1.3,
        "sphere.0.color": [ 0.8, 0.7, 0.9],
        "sphere.0.checkered": true,
        "sphere.0.reflectivity" : 0.5,
        "num_objs": 1,
        "obj.0.path" : "obj/teapot.obj",
        "num_planes" : 0
    });

    {
        let sname = "spot-light.0";
        let spot0 = SpotLight {
            name: sname.to_string(),
            pos: Vec3 { x: 0.5, y: 2.5, z: 1.0 },
            rgb: RGB { r: 1.0, g: 1.0, b: 1.0 },
            intensity: 150.0,
        };
        json[&sname] = serde_json::to_value(spot0).unwrap();
    }
    {
        let sname = "spot-light.1";
        let spot0 = SpotLight {
            name: sname.to_string(),
            pos: Vec3 { x: 0.5, y: -2.0, z: 0.0 },
            rgb: RGB { r: 0.8, g: 0.3, b: 0.8 },
            intensity: 80.0,
        };
        json[&sname] = serde_json::to_value(spot0).unwrap();
    }
    {
        let ambient = AmbientLight{
            name: "ambient".to_owned(),
            rgb: RGB { r: 1.0, g: 1.0, b: 1.0 },
            intensity: 0.1
        };
        json["ambient"] = serde_json::to_value(&ambient).unwrap();
    }
    {
        let vec0 = VectorLight{
            name: "vec-light.0".to_owned(),
            rgb: RGB  { r: 1.0, g: 1.0, b: 1.0 },
            dir: Vec3 { x: 0.5, y: 0.5, z: -0.5 },
            intensity: 1.5
        };
        json["vec-light.0"] = serde_json::to_value(vec0).unwrap();
    }
    {
        let camera = Camera::new(
            Point { x: -5.0, y: 0.0, z: 1.0 },
            Vec3  { x: 1.0,  y: 0.0, z: -0.1 }
        );
        json["camera"] = serde_json::to_value(camera).unwrap();
    }
    json["num_spheres"] = serde_json::json!(num_spheres_to_generate);
    {
        let material = Material {
            albedo: 0.8,
            reflectivity: 0.1,
            rgb: RGB { r: 0.1, g: 1.0, b: 0.1 },
            checkered: false,
        };
        let orig = Vec3{ x: 1.5, y: -1.5, z: -1.0};
        let sz = 0.5;
        let a  = Point{ x: 0.0, y: 0.0, z: 0.0 } * sz + orig; // a
        let b  = Point{ x: 1.0, y: 0.0, z: 0.0 } * sz + orig; // b
        let d  = Point{ x: 0.0, y: 0.0, z: 1.0 } * sz + orig; // d
        let c  = Point{ x: 1.0, y: 0.0, z: 1.0 } * sz + orig; // c
        let ap = Point{ x: 0.0, y: 1.0, z: 0.0 } * sz + orig; //
        let bp = Point{ x: 1.0, y: 1.0, z: 0.0 } * sz + orig; //
        let dp = Point{ x: 0.0, y: 1.0, z: 1.0 } * sz + orig; //
        let cp = Point{ x: 1.0, y: 1.0, z: 1.0 } * sz + orig; //

        let t0 = Triangle {
            name: "t0".to_owned(),
            material : material.clone(),
            points : [ a.clone(), b.clone(), c.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        let t1 = Triangle {
            name: "t1".to_owned(),
            material : material.clone(),
            points : [ a.clone(), c.clone(), d.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        let t2 = Triangle {
            name: "t2".to_owned(),
            material : material.clone(),
            points : [ a.clone(), d.clone(), dp.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        let t3 = Triangle {
            name: "t3".to_owned(),
            material : material.clone(),
            points : [ ap.clone(), a.clone(), dp.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        let t4 = Triangle {
            name: "t4".to_owned(),
            material : material.clone(),
            points : [ ap.clone(), bp.clone(), cp.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        let t5 = Triangle {
            name: "t5".to_owned(),
            material : material.clone(),
            points : [ ap.clone(), cp.clone(), dp.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        let t6 = Triangle {
            name: "t6".to_owned(),
            material : material.clone(),
            points : [ d.clone(), c.clone(), cp.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        let t7 = Triangle {
            name: "t7".to_owned(),
            material : material.clone(),
            points : [ d.clone(), cp.clone(), dp.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        let t8 = Triangle {
            name: "t8".to_owned(),
            material : material.clone(),
            points : [ a.clone(), bp.clone(), b.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        let t9 = Triangle {
            name: "t9".to_owned(),
            material : material.clone(),
            points : [ a.clone(), ap.clone(), bp.clone() ],
            has_normal: false, normal: Vec3::new(),
        };
        json[t0.name] = serde_json::to_value(&t0).unwrap();
        json[t1.name] = serde_json::to_value(&t1).unwrap();
        json[t2.name] = serde_json::to_value(&t2).unwrap();
        json[t3.name] = serde_json::to_value(&t3).unwrap();
        json[t4.name] = serde_json::to_value(&t4).unwrap();
        json[t5.name] = serde_json::to_value(&t5).unwrap();
        json[t6.name] = serde_json::to_value(&t6).unwrap();
        json[t7.name] = serde_json::to_value(&t7).unwrap();
        json[t8.name] = serde_json::to_value(&t8).unwrap();
        json[t9.name] = serde_json::to_value(&t9).unwrap();
        json["num_triangles"] = serde_json::json!(10);
    }

    if use_box {
        println!("using box!");
        json["num_planes"]        = serde_json::json!(5);
        json["plane.0.position" ] = serde_json::json!([0, 0, -1]); // bottom
        json["plane.0.normal" ]   = serde_json::json!([0, 0, 1]);
        json["plane.0.reflectivity" ] = serde_json::json!(0.1);
        json["plane.1.position" ] = serde_json::json!([0, 0, 3]); // top
        json["plane.1.normal" ]   = serde_json::json!([0, 0, -1]);
        json["plane.2.position" ] = serde_json::json!([4.5, 0, 0]); // front
        json["plane.2.normal" ]   = serde_json::json!([-1, 0, 0]);
        json["plane.2.color"]     = serde_json::json!([ 1.0, 1.0, 1.0]);
        json["plane.3.position" ] = serde_json::json!([0, 3, 0]); // left
        json["plane.3.normal" ]   = serde_json::json!([0, -1, 0]);
        json["plane.3.color"]     = serde_json::json!([ 1, 0.1, 0.1]);
        json["plane.4.position" ] = serde_json::json!([0, -3, 0]); // right
        json["plane.4.normal" ]   = serde_json::json!([0, 1, 0]);
        json["plane.4.color"]     = serde_json::json!([ 0.2, 1, 0.2]);
//        json["plane.5.position" ] = serde_json::json!([-3, 0, 0]); // back
//        json["plane.5.normal" ]   = serde_json::json!([1, 0, 0]);
//        json["plane.5.color"]     = serde_json::json!([ 1, 1, 1]);
    }

    let line = false;
    for i in 1..num_spheres_to_generate {
        let mut vec = Point {
            x : rng.gen_range(2.0..5.0),
            y : rng.gen_range(-2.0..2.0),
            z : rng.gen_range(-2.0..2.0),
        };
        let mut r = rng.gen_range(0.2..0.4);
        if line {
            vec.x = i as f64 * 2.0;
            vec.y = -1.0;
            vec.z = -0.5;
            r = 0.7;
        }
        let rgb = RGB {
            r : rng.gen_range(0.3..1.0),
            g : rng.gen_range(0.3..1.0),
            b : rng.gen_range(0.3..1.0),
        };
        let albedo = rng.gen_range(0.5..1.0);
        let reflectivity = rng.gen_range(0.0..1.0);
        let checkered = rng.gen_range(0..100) % 2;
        let name  = format!("sphere.{}.center", i);
        let rname = format!("sphere.{}.radius", i);
        let cname = format!("sphere.{}.color", i);
        let aname = format!("sphere.{}.albedo", i);
        let tname = format!("sphere.{}.checkered", i);
        let refname = format!("sphere.{}.reflectivity", i);
        json[name]  = serde_json::to_value(vec).unwrap();
        json[rname] = serde_json::json!(r);
        json[cname] = serde_json::to_value(rgb).unwrap();
        json[aname] = serde_json::json!(albedo);
        json[tname] = serde_json::json!(checkered > 0);
        json[refname] = serde_json::json!(reflectivity);
    }
    let s0 = serde_json::to_string_pretty(&json)?;
    println!("Writing scene file {}", scene_file.display());
    return fs::write(&scene_file, s0);
}

fn print_opt(opt: &Options) {
    println!("use_gamma: {} sampling-max-depth: {} use_reflection: {} max-depth: {}", opt.use_gamma, opt.adaptive_max_depth, opt.use_reflection, opt.reflection_max_depth);
    let s = format!("num_threads: {}", rayon::current_num_threads()).red();
    println!("{s}");
}

fn main() -> std::io::Result<()> {
    let opt = Options::from_args();

     ctrlc::set_handler(|| { CTRLC_HIT.store(true, Ordering::SeqCst); }).expect("ctrl-c");

    let cfg = RenderConfig {
        use_reflection: opt.use_reflection > 0,
        use_adaptive_sampling: opt.adaptive_sampling > 0,
        use_gamma: opt.use_gamma > 0,
        reflection_max_depth: opt.reflection_max_depth,
        adaptive_max_depth: opt.adaptive_max_depth,
        res_x: opt.res_x,
        res_y: opt.res_y,
    };

    if opt.num_spheres_to_generate != 0 {
        return generate_scene(opt.num_spheres_to_generate, opt.scene_file, opt.use_box > 0);
    }

    print_opt(&opt);

    let mut job = RenderJob::new(cfg);

    job.load_scene(opt.scene_file)?;
    job.render_scene();
    job.save_image(opt.img_file)?;

    Ok(())
}
