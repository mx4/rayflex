use egui::Color32;
use egui::ColorImage;
use egui::TextureHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use crate::render::RenderConfig;
use crate::render::RenderJob;

const WIDTH: usize = 400;
const HEIGHT: usize = 400;

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
            path_level: 100,
            texture_handle: None,
        }
    }
}

impl RaymaxApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }
}

fn start_rendering(
    progress: Arc<Mutex<f32>>,
    img: Arc<Mutex<ColorImage>>,
    texture: TextureHandle,
    width: usize,
    height: usize,
    scene_file: String,
    output_file: String,
    use_gamma: bool,
    use_antialias: bool,
    path_level: u32,
    ctx: egui::Context,
) {
    let img_clone = img.clone();
    let update_func = move |pct: f32| {
        *progress.lock().unwrap() = pct.min(1.0);
        let mut texture_handle = texture.clone();

        texture_handle.set(img_clone.lock().unwrap().clone(), Default::default());
        ctx.request_repaint();
    };
    let cfg = RenderConfig {
        path_tracing: path_level,
        use_gamma,
        use_adaptive_sampling: use_antialias,
        res_x: width as u32,
        res_y: height as u32,
        reflection_max_depth: 4,
        adaptive_max_depth: 5,
        use_lines: false,
        use_hashmap: false,
    };

    let mut job = RenderJob::new(cfg);

    job.load_scene(PathBuf::from(scene_file)).expect("bar");
    job.set_progress_func(Box::new(update_func));
    job.render_scene(Some(img));
    job.save_image(PathBuf::from(output_file)).expect("foo");
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
            .max_width(250.0)
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

                let txt;
                let v = *self.progress.lock().unwrap();
                if v >= 1.0 {
                    txt = "done".to_owned();
                } else if v > 0.0 {
                    txt = format!("{:.0}%", 100.0 * v)
                } else {
                    txt = "".to_owned();
                }
                ui.add(egui::ProgressBar::new(v).text(txt));
                if ui.button("Start/Restart").clicked() {
                    let ctx_clone = ctx.clone();
                    self.img = Arc::new(Mutex::new(ColorImage::new(
                        [self.width, self.height],
                        Color32::BLACK,
                    )));

                    let img_clone = self.img.clone();
                    let value_clone = self.progress.clone();
                    let scene_file = self.scene_file.clone();
                    let output_file = self.output_file.clone();
                    let use_gamma = self.use_gamma;
                    let use_antialias = self.use_antialias;
                    let path_level = self.path_level;
                    let width = self.width;
                    let height = self.height;

                    let texture_handle;
                    {
                        texture_handle = ctx.load_texture(
                            "rendered_pixels",
                            self.img.lock().unwrap().clone(),
                            Default::default(),
                        );
                        self.texture_handle = Some(texture_handle.clone());
                    }
                    thread::spawn(move || {
                        start_rendering(
                            value_clone,
                            img_clone,
                            texture_handle,
                            width,
                            height,
                            scene_file,
                            output_file,
                            use_gamma,
                            use_antialias,
                            path_level,
                            ctx_clone,
                        )
                    });
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
