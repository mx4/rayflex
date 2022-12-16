use colored::Colorize;
use rand::Rng;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::camera::Camera;
use crate::color::RGB;
use crate::image::Image;
use crate::light::AmbientLight;
use crate::light::Light;
use crate::light::SpotLight;
use crate::light::VectorLight;
use crate::material::Material;
use crate::render::RenderConfig;
use crate::render::RenderJob;
use crate::vec3::Float;
use crate::vec3::Point;
use crate::ProgressFunc;
use crate::Vec3;

use crate::three_d::Mesh;
use crate::three_d::Object;
use crate::three_d::Plane;
use crate::three_d::Sphere;
use crate::three_d::Triangle;

#[derive(Default)]
struct Scene {
    num_planes: u32,
    num_spheres: u32,
    num_triangles: usize,
    num_triangles_in_all_objs: usize,
    num_materials: u32,
    num_vec_lights: u32,
    num_spot_lights: u32,
    num_objs: u32,
    lights: Vec<Arc<dyn Light + 'static + Send + Sync>>,
    materials: Vec<Arc<Material>>,
    objects: Vec<Arc<dyn Object + 'static + Send + Sync>>,
}

fn load_materials(scene: &mut Scene, json: &serde_json::Value) -> std::io::Result<()> {
    loop {
        let s = format!("material.{}", scene.num_materials);
        match serde_json::from_value::<Material>(json[&s].clone()) {
            Err(_error) => break,
            Ok(mat) => {
                scene.materials.push(Arc::new(mat));
                scene.num_materials += 1;
            }
        }
    }
    Ok(())
}

fn load_mesh(scene: &mut Scene, json: &serde_json::Value) -> std::io::Result<()> {
    loop {
        let name = format!("obj.{}.path", scene.num_objs);
        if json[&name].is_null() {
            break;
        }
        let path = json[&name].as_str().unwrap();
        let rxname = format!("obj.{}.rotx", scene.num_objs);
        let ryname = format!("obj.{}.roty", scene.num_objs);
        let rzname = format!("obj.{}.rotz", scene.num_objs);
        let mut angle_x = 0.0;
        let mut angle_y = 0.0;
        let mut angle_z = 0.0;
        let mut angle_x_rad = 0.0;
        let mut angle_y_rad = 0.0;
        let mut angle_z_rad = 0.0;
        let mut num_triangles_in_obj = 0;
        if let Some(alpha) = json[&rxname].as_f64() {
            angle_x = alpha;
            angle_x_rad = angle_x.to_radians() as Float;
        }
        if let Some(alpha) = json[&ryname].as_f64() {
            angle_y = alpha;
            angle_y_rad = angle_y.to_radians() as Float;
        }
        if let Some(alpha) = json[&rzname].as_f64() {
            angle_z = alpha;
            angle_z_rad = angle_z.to_radians() as Float;
        }

        let opt = tobj::LoadOptions {
            triangulate: true, // converts polygon into triangles
            ignore_lines: true,
            ignore_points: true,
            ..Default::default()
        };
        let (models, materials) = tobj::load_obj(path, &opt).expect("tobj");
        let base_mat_idx = scene.num_materials;
        if let Ok(mat) = materials.clone() {
            mat.iter().for_each(|m| {
                println!("-- material {} -- {:?}", m.name.green(), m);
                let mat = Material {
                    ke: RGB::zero(),
                    shininess: m.shininess, // floating point?
                    ks: RGB::new(m.specular[0], m.specular[1], m.specular[2]),
                    checkered: false,
                    kd: RGB::new(m.diffuse[0], m.diffuse[1], m.diffuse[2]),
                };
                scene.materials.push(Arc::new(mat));
                scene.num_materials += 1;
            });
        } else {
            println!(
                "{} {:?}",
                "Error loading materials:".red().bold(),
                materials.clone().unwrap_err()
            );
        }

        models.iter().for_each(|m| {
            let mesh = &m.mesh;
            let n = mesh.indices.len() / 3;

            let mut material_str = "".to_owned();
            if mesh.material_id.is_some() && materials.is_ok() {
                material_str = materials.as_ref().unwrap()[mesh.material_id.unwrap()].name.clone();
            }

            println!(
                "-- model {:12} has {} triangles w/ {} vertices -- {}",
                m.name.blue(),
                n,
                mesh.positions.len(),
                material_str.green()
            );
            assert!(mesh.indices.len() % 3 == 0);
            scene.num_triangles_in_all_objs += n;
            num_triangles_in_obj += n;
            let mut triangles = Vec::with_capacity(n);
            let mut num_skipped = 0;
            for i in 0..n {
                let i0 = mesh.indices[3 * i] as usize;
                let i1 = mesh.indices[3 * i + 1] as usize;
                let i2 = mesh.indices[3 * i + 2] as usize;
                let x0 = mesh.positions[3 * i0] as Float;
                let y0 = mesh.positions[3 * i0 + 1] as Float;
                let z0 = mesh.positions[3 * i0 + 2] as Float;
                let x1 = mesh.positions[3 * i1] as Float;
                let y1 = mesh.positions[3 * i1 + 1] as Float;
                let z1 = mesh.positions[3 * i1 + 2] as Float;
                let x2 = mesh.positions[3 * i2] as Float;
                let y2 = mesh.positions[3 * i2 + 1] as Float;
                let z2 = mesh.positions[3 * i2 + 2] as Float;
                let mut p0 = Point::new(x0, y0, z0);
                let mut p1 = Point::new(x1, y1, z1);
                let mut p2 = Point::new(x2, y2, z2);

                if p0 == p1 || p0 == p2 || p1 == p2 {
                    num_skipped += 1;
                    continue;
                }
                p0 = p0.rotx(angle_x_rad).roty(angle_y_rad).rotz(angle_z_rad);
                p1 = p1.rotx(angle_x_rad).roty(angle_y_rad).rotz(angle_z_rad);
                p2 = p2.rotx(angle_x_rad).roty(angle_y_rad).rotz(angle_z_rad);
                let mut mat_id = 0;
                if let Some(id) = mesh.material_id {
                    mat_id = base_mat_idx as usize + id;
                }
                let mut triangle = Triangle::new([p0, p1, p2], mat_id);
                triangle.mesh_id = triangles.len();
                triangles.push(triangle);
            }
            if num_skipped > 0 {
                println!("-- skipped {} malformed triangles", num_skipped);
            }
            scene.objects.push(Arc::new(Mesh::new(triangles, 0)));
            scene.num_objs += 1;
        });
        println!(
            "-- loaded {} w/ {} triangles -- rotx={} roty={} rotz={}",
            path.green(),
            num_triangles_in_obj,
            angle_x,
            angle_y,
            angle_z
        );
    }
    println!(
        "-- mesh={} triangles={} spheres={} planes={} materials={}",
        scene.num_objs,
        scene.num_triangles + scene.num_triangles_in_all_objs,
        scene.num_spheres,
        scene.num_planes,
        scene.num_materials
    );

    Ok(())
}

fn load_spheres(scene: &mut Scene, json: &serde_json::Value) -> std::io::Result<()> {
    loop {
        let s = format!("sphere.{}", scene.num_spheres);
        match serde_json::from_value::<Sphere>(json[s].clone()) {
            Err(_error) => break,
            Ok(o) => {
                scene.objects.push(Arc::new(o));
                scene.num_spheres += 1;
            }
        }
    }
    Ok(())
}

fn load_triangles(scene: &mut Scene, json: &serde_json::Value) -> std::io::Result<()> {
    loop {
        let s = format!("triangle.{}", scene.num_triangles);
        match serde_json::from_value::<Triangle>(json[s].clone()) {
            Err(_error) => break,
            Ok(o) => {
                scene.objects.push(Arc::new(o));
                scene.num_triangles += 1;
            }
        }
    }
    Ok(())
}

fn load_planes(scene: &mut Scene, json: &serde_json::Value) -> std::io::Result<()> {
    loop {
        let s = format!("plane.{}", scene.num_planes);
        match serde_json::from_value::<Plane>(json[s].clone()) {
            Err(_error) => break,
            Ok(p) => {
                scene.objects.push(Arc::new(p));
                scene.num_planes += 1;
            }
        }
    }
    Ok(())
}

fn load_lights(scene: &mut Scene, json: &serde_json::Value) -> std::io::Result<()> {
    loop {
        let s = format!("spot-light.{}", scene.num_spot_lights);
        match serde_json::from_value::<SpotLight>(json[&s].clone()) {
            Err(_error) => break,
            Ok(mut spot) => {
                spot.name = s;
                scene.lights.push(Arc::new(spot));
                scene.num_spot_lights += 1;
            }
        }
    }
    loop {
        let s = format!("vec-light.{}", scene.num_vec_lights);
        match serde_json::from_value::<VectorLight>(json[&s].clone()) {
            Err(_error) => break,
            Ok(mut v) => {
                v.name = s;
                v.dir = v.dir.normalize();
                scene.lights.push(Arc::new(v));
                scene.num_vec_lights += 1;
            }
        }
    }
    if let Ok(ambient) = serde_json::from_value::<AmbientLight>(json["ambient"].clone()) {
        scene.lights.push(Arc::new(ambient));
    }
    Ok(())
}

fn load_resolution(cfg: &mut RenderConfig, json: &serde_json::Value) -> std::io::Result<()> {
    if cfg.res_x == 0 && cfg.res_y == 0 {
        if let Some(array) = json[&"resolution".to_string()].as_array() {
            cfg.res_x = array[0].as_u64().unwrap() as u32;
            cfg.res_y = array[1].as_u64().unwrap() as u32;
        }
    }
    let res_str = format!("{}x{}", cfg.res_x, cfg.res_y).bold();
    let mut smp_str = "".cyan();
    if cfg.use_adaptive_sampling {
        smp_str = " w/ adaptive sampling".cyan();
    }
    println!("-- img resolution: {}{}", res_str, smp_str);
    Ok(())
}

pub fn load_scene(cfg: RenderConfig) -> std::io::Result<RenderJob> {
    let mut cfg = cfg;
    if !cfg.scene_file.is_file() {
        println!("file '{}' not found.", cfg.scene_file.display());
        println!("pwd={}", std::env::current_dir()?.display());
        panic!("scene file {} not present.", cfg.scene_file.display());
    }
    println!(
        "loading scene file {}",
        cfg.scene_file.display().to_string().bold()
    );

    let data = fs::read_to_string(&cfg.scene_file)?;
    let json: serde_json::Value = serde_json::from_str(&data)?;
    let mut scene: Scene = Default::default();

    load_resolution(&mut cfg, &json)?;

    let mut camera: Camera = serde_json::from_value(json["camera"].clone()).unwrap();
    camera.aspect = cfg.res_x as Float / cfg.res_y as Float;
    camera.init();

    load_materials(&mut scene, &json)?;
    load_lights(&mut scene, &json)?;
    load_planes(&mut scene, &json)?;
    load_spheres(&mut scene, &json)?;
    load_triangles(&mut scene, &json)?;
    load_mesh(&mut scene, &json)?;

    camera.display();
    scene.lights.iter().for_each(|light| light.display());

    let job = RenderJob {
        camera,
        image: Arc::new(Mutex::new(Image::new(false, 0, 0))),
        objects: scene.objects,
        lights: scene.lights,
        materials: scene.materials,
        cfg,
        progress_total: Mutex::new(0),
        progress_func: ProgressFunc {
            func: Box::new(|_| {}),
        },
        start_ts: Instant::now(),
        total_stats: Mutex::new(Default::default()),
    };
    Ok(job)
}

pub fn generate_scene(
    num_spheres_to_generate: u32,
    scene_file: PathBuf,
    add_box: bool,
) -> std::io::Result<()> {
    let mut rng = rand::thread_rng();
    let mut json: serde_json::Value;
    let num_materials = 10;
    let res_x = 400;
    let res_y = 400;

    println!(
        "Generating scene w/ {} spheres {} materials",
        num_spheres_to_generate, num_materials
    );
    json = serde_json::json!({ "resolution": [ res_x, res_y ] });

    {
        let spot0 = SpotLight {
            name: "spot-light.0".to_owned(),
            pos: Vec3::new(0.5, 2.5, 1.0),
            rgb: RGB::new(1.0, 1.0, 1.0),
            intensity: 5.0,
        };
        json[&spot0.name] = serde_json::to_value(&spot0).unwrap();
    }
    {
        let spot0 = SpotLight {
            name: "spot-light.1".to_owned(),
            pos: Vec3::new(0.5, -2.0, 0.0),
            rgb: RGB::new(0.8, 0.3, 0.8),
            intensity: 5.0,
        };
        json[&spot0.name] = serde_json::to_value(&spot0).unwrap();
    }
    {
        // white
        let mat = Material {
            ks: RGB::zero(),
            shininess: 10.0,
            checkered: false,
            ke: RGB::zero(),
            kd: RGB::new(1.0, 1.0, 1.0),
        };
        json["material.0"] = serde_json::to_value(mat).unwrap();
        // white glossy
        let mat = Material {
            ke: RGB::zero(),
            ks: RGB::new(0.5, 0.5, 0.5),
            shininess: 10.0,
            checkered: false,
            kd: RGB::new(1.0, 1.0, 1.0),
        };
        json["material.1"] = serde_json::to_value(mat).unwrap();
        // red
        let mat = Material {
            ke: RGB::zero(),
            ks: RGB::zero(),
            shininess: 10.0,
            checkered: false,
            kd: RGB::new(1.0, 0.0, 0.0),
        };
        json["material.2"] = serde_json::to_value(mat).unwrap();
        // green
        let mat = Material {
            ke: RGB::zero(),
            shininess: 10.0,
            ks: RGB::zero(),
            checkered: false,
            kd: RGB::new(0.0, 1.0, 0.0),
        };
        json["material.3"] = serde_json::to_value(mat).unwrap();
        // blue
        let mat = Material {
            ke: RGB::zero(),
            shininess: 10.0,
            ks: RGB::zero(),
            checkered: false,
            kd: RGB::new(0.0, 0.0, 1.0),
        };
        json["material.4"] = serde_json::to_value(mat).unwrap();

        for i in 5..10 {
            let name = format!("material.{}", i);
            let mat = Material {
                ke: RGB::zero(),
                shininess: 10.0,
                ks: RGB {
                    r: rng.gen_range(0.0..0.9),
                    g: rng.gen_range(0.0..0.9),
                    b: rng.gen_range(0.0..0.9),
                },
                checkered: rng.gen_range(0..2) == 0,
                kd: RGB {
                    r: rng.gen_range(0.0..1.0),
                    g: rng.gen_range(0.0..1.0),
                    b: rng.gen_range(0.0..1.0),
                },
            };
            json[name] = serde_json::to_value(mat).unwrap();
        }
    }

    {
        let ambient = AmbientLight {
            rgb: RGB::new(1.0, 1.0, 1.0),
            intensity: 0.1,
        };
        json["ambient"] = serde_json::to_value(&ambient).unwrap();
    }
    {
        let vec0 = VectorLight {
            name: "vec-light.0".to_owned(),
            rgb: RGB::new(1.0, 1.0, 1.0),
            dir: Vec3::new(0.5, 0.5, -0.5),
            intensity: 0.0,
        };
        json["vec-light.0"] = serde_json::to_value(vec0).unwrap();
    }
    {
        let camera = Camera::new(
            Point::new(-3.0, 0.0, 0.0),      // position
            Point::new(2.0, 0.0, 0.5),       // look_at
            Vec3::new(0.0, 0.0, 1.0),        // up
            55.0,                            // vfov
            res_x as Float / res_y as Float, // aspect
        );
        json["camera"] = serde_json::to_value(camera).unwrap();
    }
    if false {
        let orig = Vec3::new(1.5, -1.5, -1.5);
        let sz = 0.5;
        let a = Point::zero() * sz + orig; // a
        let b = Point::new(1.0, 0.0, 0.0) * sz + orig; // b
        let d = Point::new(0.0, 0.0, 1.0) * sz + orig; // d
        let c = Point::new(1.0, 0.0, 1.0) * sz + orig; // c
        let ap = Point::new(0.0, 1.0, 0.0) * sz + orig; //
        let bp = Point::new(1.0, 1.0, 0.0) * sz + orig; //
        let dp = Point::new(0.0, 1.0, 1.0) * sz + orig; //
        let cp = Point::new(1.0, 1.0, 1.0) * sz + orig; //

        let t0 = Triangle::new([a, b, c], 0);
        let t1 = Triangle::new([a, c, d], 0);
        let t2 = Triangle::new([a, d, dp], 0);
        let t3 = Triangle::new([ap, a, dp], 0);
        let t4 = Triangle::new([ap, bp, cp], 0);
        let t5 = Triangle::new([ap, cp, dp], 0);
        let t6 = Triangle::new([d, c, cp], 0);
        let t7 = Triangle::new([d, cp, dp], 0);
        let t8 = Triangle::new([a, bp, b], 0);
        let t9 = Triangle::new([a, ap, bp], 0);

        json["triangle.0"] = serde_json::to_value(t0).unwrap();
        json["triangle.1"] = serde_json::to_value(t1).unwrap();
        json["triangle.2"] = serde_json::to_value(t2).unwrap();
        json["triangle.3"] = serde_json::to_value(t3).unwrap();
        json["triangle.4"] = serde_json::to_value(t4).unwrap();
        json["triangle.5"] = serde_json::to_value(t5).unwrap();
        json["triangle.6"] = serde_json::to_value(t6).unwrap();
        json["triangle.7"] = serde_json::to_value(t7).unwrap();
        json["triangle.8"] = serde_json::to_value(t8).unwrap();
        json["triangle.9"] = serde_json::to_value(t9).unwrap();
        json["num_triangles"] = serde_json::json!(10);
    }

    if add_box {
        println!("using box!");
        json["num_planes"] = serde_json::json!(5);
        let p0 = Plane {
            point: Point::new(0.0, 0.0, -1.0), // bottom
            normal: Vec3::new(0.0, 0.0, 1.0),
            material_id: 1,
        };
        json["plane.0"] = serde_json::to_value(&p0).unwrap();
        let p1 = Plane {
            point: Point::new(0.0, 0.0, 3.0), // top
            normal: Vec3::new(0.0, 0.0, -1.0),
            material_id: 0,
        };
        json["plane.1"] = serde_json::to_value(&p1).unwrap();
        let p2 = Plane {
            point: Point::new(0.0, -3.0, 0.0), // right
            normal: Vec3::new(0.0, 1.0, 0.0),
            material_id: 3,
        };
        json["plane.2"] = serde_json::to_value(&p2).unwrap();
        let p3 = Plane {
            point: Point::new(0.0, 3.0, 3.0), // left
            normal: Vec3::new(0.0, -1.0, 0.0),
            material_id: 2,
        };
        json["plane.3"] = serde_json::to_value(&p3).unwrap();
        let p4 = Plane {
            point: Point::new(4.5, 0.0, 0.0), // front
            normal: Vec3::new(-1.0, 0.0, 0.0),
            material_id: 0,
        };
        json["plane.4"] = serde_json::to_value(&p4).unwrap();
    }

    for i in 0..num_spheres_to_generate {
        let center = Point {
            x: rng.gen_range(2.0..5.0),
            y: rng.gen_range(-2.0..2.0),
            z: rng.gen_range(-2.0..2.0),
        };
        let sphere = Sphere {
            center,
            radius: rng.gen_range(0.2..0.4),
            material_id: rng.gen_range(0..10),
        };
        let name = format!("sphere.{}", i);
        json[name] = serde_json::to_value(&sphere).unwrap();
    }
    let s0 = serde_json::to_string_pretty(&json)?;
    println!("Writing scene file {}", scene_file.display());
    fs::write(&scene_file, s0)
}
