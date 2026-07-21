use egui::Color32;
use egui::ColorImage;
use egui::TextureHandle;
use egui::load::SizedTexture;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;

use crate::render::RenderConfig;
use crate::scene::load_scene;

use log::info;

const WIDTH: usize = 600;
const HEIGHT: usize = 600;
const SIDE_PANEL_WIDTH: usize = 250;

pub struct RayflexApp {
    scene_file: String,
    output_file: String,
    height: usize,
    width: usize,
    use_antialias: bool,
    use_gamma: bool,
    do_path_tracing: bool,
    path_level: u32,
    progress: Arc<Mutex<f32>>,
    texture_handle: Option<TextureHandle>,
    /// (width, height) of the texture currently held in `texture_handle`,
    /// captured at render start so the display can preserve the actual
    /// rendered aspect ratio even if the sliders are changed afterward.
    texture_size: Option<(usize, usize)>,
    rendering_active: Arc<AtomicBool>,
    rendering_needs_stop: Arc<AtomicBool>,
    scene_choice: usize,
}

impl Default for RayflexApp {
    fn default() -> Self {
        Self {
            scene_file: "scenes/cornell-box.json".to_owned(),
            output_file: "pic.png".to_owned(),
            progress: Arc::new(Mutex::new(0.0)),
            use_antialias: false,
            use_gamma: true,
            width: WIDTH,
            height: HEIGHT,
            do_path_tracing: true,
            path_level: 200,
            texture_handle: None,
            texture_size: None,
            rendering_active: Arc::new(AtomicBool::new(false)),
            rendering_needs_stop: Arc::new(AtomicBool::new(false)),
            scene_choice: 0,
        }
    }
}

fn start_rendering(
    rendering_active: Arc<AtomicBool>,
    rendering_needs_stop: Arc<AtomicBool>,
    cfg: RenderConfig,
    progress: Arc<Mutex<f32>>,
    texture: TextureHandle,
    ctx: egui::Context,
) {
    let res = load_scene(cfg);
    let mut job = res.unwrap();

    job.alloc_image();
    let img = job.image.lock().unwrap().get_img();

    let update_func = move |pct: f32| {
        *progress.lock().unwrap() = pct.min(1.0);
        let mut texture_handle = texture.clone();

        texture_handle.set(img.lock().unwrap().clone(), Default::default());
        ctx.request_repaint();
    };
    job.set_progress_func(Box::new(update_func.clone()));
    job.render_scene(rendering_needs_stop.clone());
    job.print_stats();
    // call it one last time to refresh texture
    update_func(1.0);
    job.save_image().expect("output file");

    rendering_active.store(false, Ordering::SeqCst);
    rendering_needs_stop.store(false, Ordering::SeqCst);
}

impl RayflexApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }

    fn stop_async(&mut self) {
        self.rendering_needs_stop.store(true, Ordering::SeqCst);
    }

    fn start_async(&mut self, ctx: &egui::Context) {
        self.rendering_active.store(true, Ordering::SeqCst);
        info!("start_async");
        let ctx_clone = ctx.clone();
        let value_clone = self.progress.clone();
        let rendering_active_clone = self.rendering_active.clone();
        let rendering_needs_stop_clone = self.rendering_needs_stop.clone();

        let texture_handle;
        {
            texture_handle = ctx.load_texture(
                "rendered_pixels",
                ColorImage::filled([self.width, self.height], Color32::BLACK),
                Default::default(),
            );
            self.texture_handle = Some(texture_handle.clone());
            self.texture_size = Some((self.width, self.height));
            info!("texture");
        }
        let cfg = RenderConfig {
            // path_level is only meaningful for path tracing; keep its value
            // across checkbox toggles and force 1 sample when not path tracing.
            path_tracing: if self.do_path_tracing {
                self.path_level.max(2)
            } else {
                1
            },
            use_gamma: self.use_gamma,
            use_adaptive_sampling: self.use_antialias,
            res_x: self.width as u32,
            res_y: self.height as u32,
            reflection_max_depth: 5,
            adaptive_max_depth: 2,
            use_hashmap: true,
            scene_file: PathBuf::from(self.scene_file.clone()),
            image_file: PathBuf::from(self.output_file.clone()),
        };

        info!("before-thread-spawn");
        thread::spawn(move || {
            info!("start-rendering");
            start_rendering(
                rendering_active_clone,
                rendering_needs_stop_clone,
                cfg,
                value_clone,
                texture_handle,
                ctx_clone,
            )
        });
        info!("after-thread-spawn");
    }
}

pub fn egui_main() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([(SIDE_PANEL_WIDTH + WIDTH + 50) as f32, (HEIGHT + 50) as f32]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "rayflex",
        native_options,
        Box::new(|cc| Ok(Box::new(RayflexApp::new(cc)))),
    )
    .unwrap();
}

impl eframe::App for RayflexApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let vec_str = [
            "cornell-box",
            "gold-gallery",
            "suzanne-bust",
            "torus-knot",
            "toybox",
            "sponza",
            "rayflex-pt",
            "trolley",
            "cow",
            "teapot",
            "buddha",
            "sphere-tunnel",
            "rayflex",
        ];

        egui::Panel::left("side_panel")
            .max_size(SIDE_PANEL_WIDTH as f32)
            .show(ui, |ui| {
                ui.heading("Settings");

                egui::ComboBox::from_label("Pick scene")
                    .selected_text(vec_str[self.scene_choice])
                    .show_ui(ui, |ui| {
                        for (i, s) in vec_str.iter().enumerate() {
                            let value =
                                ui.selectable_value(&mut self.scene_choice, i, s.to_owned());
                            if value.clicked() {
                                self.scene_choice = i;
                                self.scene_file = format!("scenes/{}.json", vec_str[i]);
                                self.do_path_tracing = matches!(
                                    vec_str[i],
                                    "cornell-box"
                                        | "gold-gallery"
                                        | "suzanne-bust"
                                        | "torus-knot"
                                        | "toybox"
                                        | "sponza"
                                        | "rayflex-pt"
                                );
                                self.use_gamma = true;
                                match vec_str[i] {
                                    "rayflex" | "rayflex-pt" => {
                                        self.width = 600;
                                        self.height = 400;
                                    }
                                    "gold-gallery" => {
                                        self.width = 800;
                                        self.height = 500;
                                    }
                                    "toybox" => {
                                        self.width = 800;
                                        self.height = 600;
                                    }
                                    // Sponza is by far the heaviest scene
                                    // (227k tris); keep the interactive
                                    // preview small so the UI stays usable.
                                    "sponza" => {
                                        self.width = 640;
                                        self.height = 400;
                                    }
                                    "suzanne-bust" | "torus-knot" => {
                                        self.width = 700;
                                        self.height = 700;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    });

                ui.horizontal(|ui| {
                    ui.label("scene file: ");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.scene_file)
                            .hint_text("scene-file.json"),
                    );
                });
                ui.add(egui::Separator::default());
                ui.horizontal(|ui| {
                    ui.label("output file: ");
                    ui.add(egui::TextEdit::singleline(&mut self.output_file).hint_text("pic.png"));
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.width, 32..=2048)
                            .text("width")
                            .suffix(" px")
                            .step_by(64.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.height, 32..=2048)
                            .text("height")
                            .suffix(" px")
                            .step_by(64.0),
                    );
                });
                ui.add(egui::Separator::default());
                ui.checkbox(&mut self.do_path_tracing, "use path-tracing");
                if self.do_path_tracing {
                    // Path tracing does its own multi-sample averaging, so
                    // adaptive antialiasing (a classic-ray-tracing-only
                    // feature) is mutually exclusive with it.
                    self.use_antialias = false;
                }
                ui.add_enabled(
                    self.do_path_tracing,
                    egui::Slider::new(&mut self.path_level, 2..=4096).text("Iterations"),
                );

                ui.vertical(|ui| {
                    ui.checkbox(&mut self.use_gamma, "gamma correction");
                    ui.add_enabled(
                        !self.do_path_tracing,
                        egui::Checkbox::new(&mut self.use_antialias, "adaptive antialiasing"),
                    );
                });
                ui.add(egui::Separator::default());

                let mut txt;
                let v = *self.progress.lock().unwrap();
                if v >= 1.0 {
                    txt = "done".to_owned();
                } else if v > 0.0 {
                    txt = format!("{:.0}%", 100.0 * v)
                } else {
                    txt = "".to_owned();
                }
                ui.add(egui::ProgressBar::new(v).text(txt));
                ui.add(egui::Separator::default());
                if self.rendering_active.load(Ordering::SeqCst) {
                    txt = "Stop".to_owned()
                } else {
                    txt = "Start".to_owned()
                };
                if ui
                    .add_sized(
                        [(SIDE_PANEL_WIDTH - 20) as f32, 30.],
                        egui::Button::new(txt.to_owned()),
                    )
                    .clicked()
                {
                    if self.rendering_active.load(Ordering::SeqCst) {
                        self.stop_async();
                    } else {
                        self.start_async(ui.ctx());
                    }
                }
                egui::warn_if_debug_build(ui);
            });

        egui::CentralPanel::default().show(ui, |ui| {
            if let (Some(texture), Some((tw, th))) = (&self.texture_handle, self.texture_size) {
                // Preserve the rendered image's aspect ratio instead of
                // stretching it to fill the panel (which deforms
                // non-square renders). Fit the largest rectangle with
                // the image's width/height ratio inside the available
                // space. Uses the size captured at render time so the
                // ratio matches the actual texture, not the live sliders.
                let avail = ui.available_size();
                let img_ratio = tw as f32 / th as f32;
                let avail_ratio = avail.x / avail.y;
                let display_size = if avail_ratio > img_ratio {
                    // Panel is wider than the image: height-constrained.
                    egui::vec2(avail.y * img_ratio, avail.y)
                } else {
                    // Panel is taller than the image: width-constrained.
                    egui::vec2(avail.x, avail.x / img_ratio)
                };
                let t = SizedTexture::new(texture, display_size);
                ui.image(t);
            }
        });
    }
}
