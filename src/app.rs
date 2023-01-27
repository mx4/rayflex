use egui::Color32;
use egui::ColorImage;
use egui::TextureHandle;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use crate::render::RenderConfig;
use crate::scene::load_scene;

use log::Level;
use log::info;

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
    texture_handle: Option<TextureHandle>,
    rendering_active: Arc<AtomicBool>,
    rendering_needs_stop: Arc<AtomicBool>,
    scene_choice: usize,
}

impl Default for RaymaxApp {
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
#[cfg(not(target_arch = "wasm32"))]
    job.print_stats();
    // call it one last time to refresh texture
    update_func(1.0);
    job.save_image().expect("output file");

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
        info!("start_async");
        let ctx_clone = ctx.clone();
        let value_clone = self.progress.clone();
        let rendering_active_clone = self.rendering_active.clone();
        let rendering_needs_stop_clone = self.rendering_needs_stop.clone();

        let texture_handle;
        {
            texture_handle = ctx.load_texture(
                "rendered_pixels",
                ColorImage::new([self.width, self.height], Color32::BLACK),
                Default::default(),
            );
            self.texture_handle = Some(texture_handle.clone());
            info!("texture");
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

#[cfg(not(target_arch = "wasm32"))]
pub fn egui_main() {
    let native_options = eframe::NativeOptions {
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

#[cfg(target_arch = "wasm32")]
pub fn egui_main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    console_log::init_with_level(Level::Debug);

    let web_options = eframe::WebOptions::default();
    info!("It works!");
    wasm_bindgen_futures::spawn_local(async {
        eframe::start_web(
            "the_canvas_id", // hardcode it
	    web_options,
	    Box::new(|cc| Box::new(RaymaxApp::new(cc))),
        )
	.await
        .expect("failed to start eframe");
    });
}

impl eframe::App for RaymaxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let vec_str = [
            "cornell-box".to_owned(),
            "trolley".to_owned(),
            "cow".to_owned(),
            "teapot".to_owned(),
            "buddha".to_owned(),
            "sphere-box".to_owned(),
            "sphere-nobox".to_owned(),
            "test".to_owned(),
        ];

        egui::SidePanel::left("side_panel")
            .max_width(SIDE_PANEL_WIDTH as f32)
            .show(ctx, |ui| {
                ui.heading("Settings");

                egui::ComboBox::from_label("Pick scene")
                    .selected_text(vec_str[self.scene_choice].clone())
                    .show_ui(ui, |ui| {
                        for (i, s) in vec_str.iter().enumerate() {
                            let value = ui.selectable_value(&mut self.scene_choice, i, s);
                            if value.clicked() {
                                self.scene_choice = i;
                                self.scene_file = format!("scenes/{}.json", vec_str[i]);
                                self.do_path_tracing = i == 0;
                                self.use_gamma = i == 0;
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
                if !self.do_path_tracing {
                    self.path_level = 1;
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
                    txt = "Stop".to_owned();
                } else {
                    txt = "Start".to_owned();
                }
                if ui
                    .add_sized([SIDE_PANEL_WIDTH as f32, 30.], egui::Button::new(txt))
                    .clicked()
                {
                    if self.rendering_active.load(Ordering::SeqCst) {
                        self.stop_async();
                    } else {
                        self.start_async(ctx);
                    }
                }
                egui::warn_if_debug_build(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.texture_handle {
                ui.add(egui::Image::new(texture.id(), texture.size_vec2()));
            }
        });
    }
}
