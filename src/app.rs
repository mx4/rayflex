use egui::Color32;
use egui::ColorImage;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const WIDTH: usize = 400;
const HEIGHT: usize = 400;

pub struct RaymaxApp {
    filename: String,
    height: usize,
    width: usize,
    use_antialias: bool,
    use_gamma: bool,
    do_path_tracing: bool,
    progress: Arc<Mutex<f32>>,
    img: Arc<Mutex<ColorImage>>,
}

impl Default for RaymaxApp {
    fn default() -> Self {
        Self {
            filename: "scenes/cornell-box.json".to_owned(),
            progress: Arc::new(Mutex::new(0.0)),
            img: Arc::new(Mutex::new(ColorImage::new([WIDTH, HEIGHT], Color32::BLACK))),
            use_antialias: true,
            use_gamma: false,
            width: WIDTH,
            height: HEIGHT,
            do_path_tracing: false,
        }
    }
}

impl RaymaxApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }
}

fn bg_timer(progress: Arc<Mutex<f32>>, _img: Arc<Mutex<ColorImage>>, ctx: egui::Context) {
    let one_msec = Duration::from_millis(1);
    *progress.lock().unwrap() = 0.0;
    loop {
        thread::sleep(one_msec);
        *progress.lock().unwrap() += 0.0005;
        ctx.request_repaint();
        if *progress.lock().unwrap() >= 1.0 {
            break;
        }
    }
}

fn create_image(ctx: &egui::Context, image: Arc<Mutex<ColorImage>>) -> egui::TextureHandle {
    let mut img = image.lock().unwrap();
    let width = img.width();
    for x in 0..width {
        for y in 0..img.height() {
            img.pixels[y * width + x] = Color32::from_rgb(255, 155, 0);
        }
    }
    ctx.load_texture("rendered_pixels", img.clone(), Default::default())
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

        egui::SidePanel::left("side_panel").max_width(250.0).show(ctx, |ui| {
            ui.heading("Settings");

            ui.horizontal(|ui| {
                ui.label("filename: ");
                ui.add(egui::TextEdit::singleline(&mut self.filename).hint_text("scene-file.json"));
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut self.width, 0..=4096)
                                          .text("Width")
                                          .suffix(" pix")
                                          .step_by(32.0));
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut self.height, 0..=4096)
                                          .text("Height")
                                          .suffix(" pix")
                                          .step_by(32.0));
            });
            ui.vertical(|ui| {
                ui.checkbox(&mut self.do_path_tracing, "use path-tracing");
                ui.checkbox(&mut self.use_gamma, "gamma correction");
                ui.checkbox(&mut self.use_antialias, "adaptive antialiasing");
            });

            ui.add(egui::ProgressBar::new(*self.progress.lock().unwrap()).text("pct"));
            if ui.button("Start/Restart").clicked() {
                let ctx_clone = ctx.clone();
                self.img = Arc::new(Mutex::new(ColorImage::new([self.width, self.height], Color32::BLACK)));
                let img_clone = self.img.clone();
                let value_clone = self.progress.clone();
                thread::spawn(|| bg_timer(value_clone, img_clone, ctx_clone));
            }
            egui::warn_if_debug_build(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let img_clone = self.img.clone();
            let texture = create_image(ctx, img_clone);
            ui.add(egui::Image::new(texture.id(), texture.size_vec2()));
        });
    }
}