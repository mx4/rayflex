use egui::Color32;
use egui::ColorImage;
use egui::TextureHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::thread;

use crate::render::RenderConfig;
use crate::render::RenderJob;

const WIDTH: usize = 600;
const HEIGHT: usize = 600;
const SIDE_PANEL_WIDTH: usize = 250;

pub struct RaymaxApp {
    scene_file: String,
    output_file: String,
    height: usize,
    width: usize,
    use_antialias: bool,
    use_gamma: bool,
    do_path_tracing: bool,
    path_level: u32,
    progress: Arc<Mutex<f32>>,
    img: Arc<Mutex<ColorImage>>,
    texture_handle: Option<TextureHandle>,
    rendering_active: Arc<AtomicBool>,
    rendering_needs_stop: Arc<AtomicBool>
}

impl Default for RaymaxApp {
    fn default() -> Self {
        Self {
            scene_file: "scenes/cornell-box.json".to_owned(),
            output_file: "pic.png".to_owned(),
            progress: Arc::new(Mutex::new(0.0)),
            img: Arc::new(Mutex::new(ColorImage::new([WIDTH, HEIGHT], Color32::BLACK))),
            use_antialias: false,
            use_gamma: true,
            width: WIDTH,
            height: HEIGHT,
            do_path_tracing: true,
            path_level: 200,
            texture_handle: None,
            rendering_active: Arc::new(AtomicBool::new(false)),
            rendering_needs_stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn start_rendering(
    rendering_active: Arc<AtomicBool>,
    rendering_needs_stop: Arc<AtomicBool>,
    cfg: RenderConfig,
    progress: Arc<Mutex<f32>>,
    img: Arc<Mutex<ColorImage>>,
    texture: TextureHandle,
    scene_file: String,
    output_file: String,
    ctx: egui::Context,
) {
    let img_clone = img.clone();
    let update_func = move |pct: f32| {
        *progress.lock().unwrap() = pct.min(1.0);
        let mut texture_handle = texture.clone();

        texture_handle.set(img_clone.lock().unwrap().clone(), Default::default());
        ctx.request_repaint();
    };

    let mut job = RenderJob::new(cfg);

    job.load_scene(PathBuf::from(scene_file))
        .expect("scene file");
    job.set_progress_func(Box::new(update_func));
    job.render_scene(Some(img), rendering_needs_stop.clone());
    job.save_image(PathBuf::from(output_file))
        .expect("output file");

    rendering_active.store(false, Ordering::SeqCst);
    rendering_needs_stop.store(false, Ordering::SeqCst);
}


impl RaymaxApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }

    fn stop_async(&mut self) {
        self.rendering_needs_stop.store(true, Ordering::SeqCst);
    }

    fn start_async(&mut self, ctx: &egui::Context) {
        self.rendering_active.store(true, Ordering::SeqCst);
        let ctx_clone = ctx.clone();
        self.img = Arc::new(Mutex::new(ColorImage::new(
            [self.width, self.height],
            Color32::BLACK,
        )));

        let img_clone = self.img.clone();
        let value_clone = self.progress.clone();
        let scene_file = self.scene_file.clone();
        let output_file = self.output_file.clone();
        let rendering_active_clone = self.rendering_active.clone();
        let rendering_needs_stop_clone = self.rendering_needs_stop.clone();

        let texture_handle;
        {
            texture_handle = ctx.load_texture(
                "rendered_pixels",
                self.img.lock().unwrap().clone(),
                Default::default(),
            );
            self.texture_handle = Some(texture_handle.clone());
        }
        let cfg = RenderConfig {
            path_tracing: self.path_level,
            use_gamma: self.use_gamma,
            use_adaptive_sampling: self.use_antialias,
            res_x: self.width as u32,
            res_y: self.height as u32,
            reflection_max_depth: 5,
            adaptive_max_depth: 2,
            use_lines: false,
            use_hashmap: true,
        };

        thread::spawn(move || {
            start_rendering(
                rendering_active_clone,
                rendering_needs_stop_clone,
                cfg,
                value_clone,
                img_clone,
                texture_handle,
                scene_file,
                output_file,
                ctx_clone,
            )
        });
    }
}

pub fn egui_main() {
    tracing_subscriber::fmt::init();

    let native_options: eframe::NativeOptions = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(
            (SIDE_PANEL_WIDTH + WIDTH + 50) as f32,
            (HEIGHT + 50) as f32,
        )),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "raymax",
        native_options,
        Box::new(|cc| Box::new(RaymaxApp::new(cc))),
    );
}


impl eframe::App for RaymaxApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        frame.close();
                    }
                });
            });
        });

        egui::SidePanel::left("side_panel")
            .max_width(SIDE_PANEL_WIDTH as f32)
            .show(ctx, |ui| {
                ui.heading("Settings");

                ui.horizontal(|ui| {
                    ui.label("scene file: ");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.scene_file)
                            .hint_text("scene-file.json"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("output file: ");
                    ui.add(egui::TextEdit::singleline(&mut self.output_file).hint_text("pic.png"));
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.width, 0..=2048)
                            .text("Width")
                            .suffix(" pix")
                            .step_by(32.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.height, 0..=2048)
                            .text("Height")
                            .suffix(" pix")
                            .step_by(32.0),
                    );
                });
                ui.checkbox(&mut self.do_path_tracing, "use path-tracing");
                if !self.do_path_tracing {
                    self.path_level = 1;
                }
                ui.add_enabled(
                    self.do_path_tracing,
                    egui::Slider::new(&mut self.path_level, 2..=2048).text("Iterations"),
                );

                ui.vertical(|ui| {
                    ui.checkbox(&mut self.use_gamma, "gamma correction");
                    ui.checkbox(&mut self.use_antialias, "adaptive antialiasing");
                });

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
                if self.rendering_active.load(Ordering::SeqCst) {
                    txt = "Stop".to_owned();
                } else {
                    txt = "Start".to_owned();
                }
                if ui.add_sized([SIDE_PANEL_WIDTH as f32, 30.],egui::Button::new(txt)).clicked() {
                    if self.rendering_active.load(Ordering::SeqCst) {
                        self.stop_async();
                    } else {
                        self.start_async(ctx);
                    }
                }
                egui::warn_if_debug_build(ui);
            });

        egui::CentralPanel::default()
            .show(ctx, |ui| {
                if let Some(texture) = &self.texture_handle {
                    ui.add(egui::Image::new(texture.id(), texture.size_vec2()));
                }
            });
    }
}
