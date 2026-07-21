use colored::Colorize;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::ProgressFunc;
use crate::Vec3;
use crate::camera::Camera;
use crate::color::RGB;
use crate::image::Image;
use crate::light::AmbientLight;
use crate::light::Light;
use crate::light::SpotLight;
use crate::light::VectorLight;
use crate::material::Material;
use crate::material::Texture;
use crate::render::NeeLight;
use crate::render::RenderConfig;
use crate::render::RenderJob;
use crate::vec3::Float;
use crate::vec3::Point;
use crate::vec3::Vec2;

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
    /// Emissive spheres/triangles registered for next-event estimation,
    /// collected as their concrete geometry is loaded (the boxed
    /// `dyn Object` can't be downcast back).
    nee_lights: Vec<NeeLight>,
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
        // Uniform scale (default 1.0) and translation offset (default 0),
        // applied per vertex as p' = R(scale * p) + translate: scale about
        // the origin, then rotate, then translate into place (SRT order).
        // Lets a mesh be sized and positioned regardless of its native
        // coordinates (e.g. shrink/place a downloaded model).
        let scale = json[&format!("obj.{}.scale", scene.num_objs)]
            .as_f64()
            .unwrap_or(1.0) as Float;
        let translate: Vec3 =
            serde_json::from_value(json[&format!("obj.{}.translate", scene.num_objs)].clone())
                .unwrap_or_else(|_| Vec3::zero());

        let opt = tobj::LoadOptions {
            triangulate: true, // converts polygon into triangles
            ignore_lines: true,
            ignore_points: true,
            // Unify position/normal/texcoord indices into one index buffer
            // (mesh.indices) so a vertex's normal is at the same index as
            // its position. Without this, normals (if present) carry their
            // own separate index stream (mesh.normal_indices) that our
            // per-face loop below doesn't consult.
            single_index: true,
            ..Default::default()
        };
        let (models, materials) = tobj::load_obj(path, &opt).expect("tobj");
        let base_mat_idx = scene.num_materials;
        // .mtl paths (map_Kd etc.) are relative to the OBJ's own directory,
        // not the process's current directory.
        let obj_dir = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
        if let Ok(mat) = materials.clone() {
            mat.iter().for_each(|m| {
                println!("-- material {} -- {:?}", m.name.green(), m);
                let map_kd = if m.diffuse_texture.is_empty() {
                    None
                } else {
                    Texture::load(&obj_dir.join(&m.diffuse_texture))
                };
                let mat = Material {
                    ke: RGB::zero(),
                    shininess: m.shininess, // floating point?
                    ks: RGB::new(m.specular[0], m.specular[1], m.specular[2]),
                    checkered: false,
                    kd: RGB::new(m.diffuse[0], m.diffuse[1], m.diffuse[2]),
                    kt: RGB::zero(),
                    ior: 1.0,
                    map_kd,
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
        // How many .mtl materials actually loaded for this OBJ. tobj still
        // reports per-face material ids even when the referenced .mtl file
        // is missing, so we must range-check against this before trusting
        // them (else base_mat_idx + id indexes past the material list).
        let num_mtl_mats = (scene.num_materials - base_mat_idx) as usize;

        // Merge every submesh of this OBJ into ONE mesh with a single BVH.
        // tobj splits an OBJ by group (`g`), not just by material, so Sponza
        // alone produced 361 separate Mesh objects -- and find_closest_hit
        // tests every object's root AABB on every ray, so that split cost
        // ~300 AABB tests/ray. For contrast, buddha (4x MORE triangles, but
        // one mesh) needs ~3.6. Merging gives one spatial tree that prunes by
        // location instead of by authoring group. The per-triangle
        // material_id is resolved below, so merging costs nothing at shading
        // time (Mesh::get_material_id already reads it per hit triangle).
        let mut all_triangles: Vec<Triangle> = Vec::new();

        models.iter().for_each(|m| {
            let mesh = &m.mesh;
            let n = mesh.indices.len() / 3;

            let mut material_str = "".to_owned();
            if mesh.material_id.is_some() && materials.is_ok() {
                material_str = materials.as_ref().unwrap()[mesh.material_id.unwrap()]
                    .name
                    .clone();
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
            all_triangles.reserve(n);
            let mut num_skipped = 0;
            // With single_index, a present vn stream is unified 1:1 with
            // positions (same index, same vertex count) -- see LoadOptions
            // above. Meshes without `vn` (teapot, trolley, buddha, cow) get
            // no smooth normals and keep the flat per-face look.
            let has_normals =
                !mesh.normals.is_empty() && mesh.normals.len() == mesh.positions.len();
            let vertex_normal = |idx: usize| -> Vec3 {
                Vec3::new(
                    mesh.normals[3 * idx] as Float,
                    mesh.normals[3 * idx + 1] as Float,
                    mesh.normals[3 * idx + 2] as Float,
                )
                // Rotate to match the geometry (no translate: normals are
                // directions. No scale: uniform scale doesn't change a
                // normal's direction, and get_normal() renormalizes anyway).
                .rotx(angle_x_rad)
                .roty(angle_y_rad)
                .rotz(angle_z_rad)
            };
            // Same single_index unification as normals above, but for `vt`.
            // UVs aren't spatial, so unlike positions/normals they need no
            // rotation/scale/translation.
            let has_uvs =
                !mesh.texcoords.is_empty() && mesh.texcoords.len() / 2 == mesh.positions.len() / 3;
            let vertex_uv = |idx: usize| -> Vec2 {
                Vec2 {
                    x: mesh.texcoords[2 * idx] as Float,
                    y: mesh.texcoords[2 * idx + 1] as Float,
                }
            };
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
                p0 = (p0 * scale).rotx(angle_x_rad).roty(angle_y_rad).rotz(angle_z_rad) + translate;
                p1 = (p1 * scale).rotx(angle_x_rad).roty(angle_y_rad).rotz(angle_z_rad) + translate;
                p2 = (p2 * scale).rotx(angle_x_rad).roty(angle_y_rad).rotz(angle_z_rad) + translate;
                // Default to material.0; use the triangle's .mtl material
                // only when that material actually loaded (see num_mtl_mats).
                let mut mat_id = 0;
                if let Some(id) = mesh.material_id {
                    if id < num_mtl_mats {
                        mat_id = base_mat_idx as usize + id;
                    }
                }
                let mut triangle = Triangle::new([p0, p1, p2], mat_id);
                // Index into the MERGED list -- aabb.rs uses mesh_id as the
                // triangle's identity when building/reporting hits.
                triangle.mesh_id = all_triangles.len();
                if has_normals {
                    triangle.normals =
                        Some([vertex_normal(i0), vertex_normal(i1), vertex_normal(i2)]);
                }
                if has_uvs {
                    triangle.uvs = Some([vertex_uv(i0), vertex_uv(i1), vertex_uv(i2)]);
                }
                all_triangles.push(triangle);
            }
            if num_skipped > 0 {
                println!("-- skipped {num_skipped} malformed triangles");
            }
        });
        scene.objects.push(Arc::new(Mesh::new(all_triangles, 0)));
        // One object per OBJ file now. This also keeps num_objs in step with
        // the obj.N.* key numbering: it used to advance once per submesh, so
        // a multi-group obj.0 (like Sponza, 361 submeshes) pushed the counter
        // past obj.1 and silently skipped every later mesh in the scene.
        scene.num_objs += 1;
        println!(
            "-- loaded {} w/ {} triangles -- rotx={} roty={} rotz={} scale={} translate={:?}",
            path.green(),
            num_triangles_in_obj,
            angle_x,
            angle_y,
            angle_z,
            scale,
            translate
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
                let obj_idx = scene.objects.len();
                if let Some(mat) = scene.materials.get(o.material_id) {
                    if !mat.ke.is_zero() {
                        scene
                            .nee_lights
                            .push(NeeLight::from_sphere(obj_idx, o.center, o.radius, mat.ke));
                    }
                }
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
                let obj_idx = scene.objects.len();
                if let Some(mat) = scene.materials.get(o.material_id) {
                    if !mat.ke.is_zero() {
                        scene
                            .nee_lights
                            .push(NeeLight::from_triangle(obj_idx, o.points, mat.ke));
                    }
                }
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
    {
        let res_str = format!("{}x{}", cfg.res_x, cfg.res_y).bold();
        let mut smp_str = "".cyan();
        if cfg.use_adaptive_sampling {
            smp_str = " w/ adaptive sampling".cyan();
        }
        println!("-- img resolution: {res_str}{smp_str}");
    }
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

    // Mark which objects are NEE-sampled lights so the path integrator can
    // avoid double-counting them (see `RenderJob::obj_is_nee_light`).
    let mut obj_is_nee_light = vec![false; scene.objects.len()];
    for light in &scene.nee_lights {
        obj_is_nee_light[light.obj_idx()] = true;
    }
    if !scene.nee_lights.is_empty() {
        println!("-- nee: {} light primitive(s)", scene.nee_lights.len());
    }

    let job = RenderJob {
        camera,
        image: Arc::new(Mutex::new(Image::new(false, false, 0, 0))),
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
        nee_lights: scene.nee_lights,
        obj_is_nee_light,
    };
    Ok(job)
}
