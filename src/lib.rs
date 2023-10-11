use nih_plug_vizia::vizia::{
    cache::BoundingBox,
    prelude::*,
    vg::{Color, Paint, Path},
};

#[derive(Debug)]
pub enum ParamUpdateEvent {
    ParamUpdate,
}

pub enum ScopeLine<'a> {
    Constant(ConstantLine),
    Signal(SignalLine<'a>),
    Audio(AudioLine<'a>),
}

pub struct ConstantLine {
    constant: f32,
    color: Color,
}

impl ConstantLine {
    pub fn new(color: Color, constant: f32) -> Self {
        Self { color, constant }
    }
}

pub struct SignalLine<'a> {
    samples: &'a Vec<f32>,
    color: Color,
    width: f32,
}

impl<'a> SignalLine<'a> {
    pub fn new(samples: &'a Vec<f32>, color: Color, width: f32) -> Self {
        Self {
            samples,
            color,
            width,
        }
    }
}

pub struct AudioLine<'a> {
    samples: &'a Vec<f32>,
    color: Color,
}

impl<'a> AudioLine<'a> {
    pub fn new(samples: &'a Vec<f32>, color: Color) -> Self {
        Self { samples, color }
    }
}

pub trait ScopeData {
    fn recalculate(&mut self);
    fn scope_lines(&self) -> Vec<ScopeLine>;
}

pub struct ScopeView<T: ScopeData> {
    scope_data: T,
    config: ScopeConfig,
}

pub struct ScopeConfig {
    x_divs: u32,
    y_divs: u32,
}

impl<T: ScopeData + 'static> ScopeView<T> {
    pub fn new(cx: &mut Context, scope_data: T, config: Option<ScopeConfig>) -> Handle<Self> {
        let mut view = Self {
            scope_data,
            config: config.unwrap_or(ScopeConfig {
                x_divs: 10,
                y_divs: 10,
            }),
        };

        view.scope_data.recalculate();
        view.build(cx, |_| {})
    }

    fn draw_grid(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let ScopeConfig { x_divs, y_divs } = self.config;
        let bounds = cx.bounds();
        let grid_paint = Paint::color(Color::rgb(50, 50, 40));
        let mut grid_path = Path::new();

        for x in 0..x_divs + 1 {
            let x_pos = bounds.x + (x as f32 / x_divs as f32) * bounds.w;
            grid_path.move_to(x_pos, bounds.y);
            grid_path.line_to(x_pos, bounds.y + bounds.h);
        }
        for y in 0..y_divs + 1 {
            let y_pos = bounds.y + (y as f32 / y_divs as f32) * bounds.h;
            grid_path.move_to(bounds.x, y_pos);
            grid_path.line_to(bounds.x + bounds.w, y_pos);
        }

        canvas.stroke_path(&mut grid_path, &grid_paint);
    }

    fn draw_horizontal(&self, cx: &mut DrawContext, canvas: &mut Canvas, line: &ConstantLine) {
        let bounds = cx.bounds();
        let mut threshold_path = Path::new();
        let threshold_paint = Paint::color(line.color);

        let threshold_y = line.constant * bounds.h / 2.0;
        let base_y = bounds.y + bounds.h / 2.0;
        threshold_path.move_to(bounds.x, base_y + threshold_y);
        threshold_path.line_to(bounds.x + bounds.w, base_y + threshold_y);

        threshold_path.move_to(bounds.x, base_y - threshold_y);
        threshold_path.line_to(bounds.x + bounds.w, base_y - threshold_y);

        canvas.stroke_path(&mut threshold_path, &threshold_paint);
    }

    fn draw_signal(&self, cx: &mut DrawContext, canvas: &mut Canvas, line: &SignalLine) {
        let bounds = cx.bounds();
        let bucket_size = (line.samples.len() as f32 / bounds.w) as usize;
        let mut path = Path::new();
        path.move_to(bounds.x, bounds.y + bounds.h / 2.0);

        for (x, bucket) in line.samples.chunks(bucket_size).enumerate() {
            let bucket_sum: f32 = bucket.iter().sum();
            let average = bucket_sum / (bucket.len() as f32);

            let x = bounds.x + x as f32;
            let clipped_y = average.clamp(-1.0, 1.0);
            let y = bounds.y + clipped_y * bounds.h / 2.0 + bounds.h / 2.0;
            path.line_to(x, y);
        }

        let mut paint = Paint::color(line.color);
        paint.set_line_width(line.width);
        canvas.stroke_path(&mut path, &paint);
    }

    fn draw_audio(&self, cx: &mut DrawContext, canvas: &mut Canvas, line: &AudioLine) {
        let bounds = cx.bounds();
        let bucket_size = (line.samples.len() as f32 / bounds.w) as usize;
        let mut draw_wave = |vector: &Vec<f32>, scale: f32| {
            let mut path = Path::new();
            let mut x = bounds.x;
            let chunks = vector.chunks(bucket_size);
            let chunks_length = chunks.len();

            for bucket in chunks {
                let extrema = bucket
                    .iter()
                    .fold(None, |acc: Option<(f32, f32)>, &x| match acc {
                        Some((min, max)) => Some((min.min(x), max.max(x))),
                        None => Some((x, x)),
                    });

                let (min, max) = extrema.expect("Expect there not be NaN's etc in a plotted graph");

                let max = if max - min < 2.0 / bounds.h {
                    max + 4.0 / bounds.h
                } else {
                    max
                };

                let y_loc = |y: f32| {
                    bounds.y - scale * y.clamp(-1.0, 1.0) * bounds.h / 2.0 + bounds.h / 2.0
                };

                path.move_to(x, y_loc(min));
                path.line_to(x, y_loc(max));

                x += 1.0;

                if (x - bounds.x) as usize == chunks_length - 2 {
                    break;
                }
            }

            let scale = |c| (255.0 * c * scale.powf(1.0 / 5.0)) as u8;
            let mut paint = Paint::color(Color::rgb(
                scale(line.color.r),
                scale(line.color.g),
                scale(line.color.b),
            ));
            paint.set_line_width(2.0);

            canvas.stroke_path(&mut path, &paint);
        };

        draw_wave(&line.samples, 1.0);
        draw_wave(&line.samples, 0.5);
    }

    fn draw_border(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let BoundingBox { x, y, w, h } = cx.bounds();

        let width = 3.0;

        let mut path = Path::new();
        path.rect(x - width / 2.0, y - width / 2.0, w + width, h + width);
        let mut paint = Paint::color(Color::hex("#ccccdc"));
        paint.set_line_width(3.0);
        canvas.stroke_path(&mut path, &paint);
    }
}

impl<T: ScopeData + 'static> View for ScopeView<T> {
    fn element(&self) -> Option<&'static str> {
        Some("scope")
    }

    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|param_event, _| match param_event {
            ParamUpdateEvent::ParamUpdate => self.scope_data.recalculate(),
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let background_color = Color::rgb(0, 0, 0);

        let bounds = cx.bounds();

        canvas.clear_rect(
            bounds.x as u32,
            bounds.y as u32,
            bounds.w as u32,
            bounds.h as u32,
            background_color,
        );

        self.draw_grid(cx, canvas);

        self.scope_data
            .scope_lines()
            .iter()
            .for_each(|line| match line {
                ScopeLine::Constant(line) => self.draw_horizontal(cx, canvas, line),
                ScopeLine::Signal(line) => self.draw_signal(cx, canvas, line),
                ScopeLine::Audio(line) => self.draw_audio(cx, canvas, line),
            });

        self.draw_border(cx, canvas);
    }
}