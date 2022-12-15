use colored::Colorize;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::camera::Camera;
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
        if let Ok(mat) = materials {
            mat.iter().for_each(|m| {
                println!("material: {:?} -- {:?}", m.name, m);
            });
        } else {
            println!(
                "{} {:?}",
                "Error loading materials:".red().bold(),
                materials.unwrap_err()
            );
        }

        models.iter().for_each(|m| {
            let mesh = &m.mesh;
            let n = mesh.indices.len() / 3;
            println!(
                "-- model {} has {} triangles w/ {} vertices",
                m.name,
                n,
                mesh.positions.len()
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
                let mut triangle = Triangle::new([p0, p1, p2], 0);
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
