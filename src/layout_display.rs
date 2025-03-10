use crate::Message;
use core::fmt;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas::{self, Text};
use iced::widget::canvas::{Cache, Geometry};
use iced::{mouse, Color, Point, Rectangle, Renderer, Size, Theme};
use km::{self, KeyCoord, MetricContext};

#[derive(Debug, Clone)]
pub struct KeyData {
    letter: char,
    frequency: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum ColorStyle {
    Frequency,
    Metric,
    Fingers,
    None,
}

impl ColorStyle {
    pub const ALL: [ColorStyle; 4] = [
        ColorStyle::Frequency,
        ColorStyle::Metric,
        ColorStyle::Fingers,
        ColorStyle::None,
    ];
}

impl fmt::Display for ColorStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColorStyle::Frequency => write!(f, "Frequency"),
            ColorStyle::Metric => write!(f, "Metric"),
            ColorStyle::Fingers => write!(f, "Fingers"),
            ColorStyle::None => write!(f, "None"),
        }
    }
}

#[derive(Debug)]
pub struct LayoutDisplay {
    keys: Vec<(KeyCoord, Option<KeyData>)>,
    lowest_y: f32,
    highest_x: f32,
    lowest_x: f32,
    highest_y: f32,
    pub style: ColorStyle,
    cache: Cache,
}

fn color_from_finger(finger: km::Finger) -> Color {
    let kind = match finger.kind() {
        km::FingerKind::Pinky => 0.4,
        km::FingerKind::Ring => 0.3,
        km::FingerKind::Middle => 0.2,
        km::FingerKind::Index => 0.1,
        km::FingerKind::Thumb => 0.0,
    };

    let (r, b) = match finger.hand() {
        km::Hand::Left => (0.5 + kind, 0.1 + kind),
        km::Hand::Right => (0.1 + kind, 0.5 + kind),
    };

    Color::from_rgb(r, 0.1 + kind, b)
}

impl LayoutDisplay {
    fn keys(
        ctx: &MetricContext,
        style: ColorStyle,
        metric: usize,
    ) -> Vec<(KeyCoord, Option<KeyData>)> {
        let kb = &ctx.keyboard;
        let l = &ctx.layout;
        let corpus = &ctx.analyzer.corpus;
        let freqs: Vec<f32> = match style {
            ColorStyle::Frequency => {
                let mut max_freq = l.0.iter().map(|c| corpus.chars[*c]).max().unwrap() as f32;
                let mut heatmap_data: Vec<f32> = l.0.iter().map(|c| corpus.chars[*c] as f32).collect();

                let kb_size = kb.keys.map.iter().flatten().count();
                // kb.process_combo_indexes(); // XXX: what do???
                for (idx, combo) in kb.combo_indexes.iter().enumerate() {
                    let key = l.0[kb_size + idx];
                    let freq = corpus.chars[key] as f32;
                    if freq > max_freq {
                        max_freq = freq;
                    }
                    for i in combo {
                        heatmap_data[*i] += freq;
                    }
                }

                heatmap_data = heatmap_data
                    .iter()
                    .map(|c| 0.3 + (1.0 + c / (max_freq - 0.3)).log2())
                    .collect();

                println!("heatmap_data: {:?}", heatmap_data);
                heatmap_data
            }
            ColorStyle::Metric => {
                let counts: Vec<f32> = (0..ctx.layout.0.len())
                    .map(|p| {
                        let sum: f32 = ctx
                            .analyzer
                            .data
                            .strokes
                            .iter()
                            .filter(|data| data.nstroke.to_vec().contains(&p))
                            .filter_map(|s| {
                                s.amounts
                                    .iter()
                                    .find(|am| am.metric == metric)
                                    .map(|am| (&s.nstroke, am))
                            })
                            .map(|(ns, am)| {
                                am.amount
                                    * ctx.layout.frequency(
                                        &ctx.analyzer.corpus,
                                        ns,
                                        Some(ctx.metrics[metric].ngram_type),
                                    ) as f32
                            })
                            .sum();
                        sum
                    })
                    .collect();

                let max_freq = counts
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap();

                counts
                    .iter()
                    .map(|x| 0.2 + (1.0 + x / (*max_freq - 0.2)).log2())
                    .collect()
            }
            _ => vec![0.0; l.0.len()],
        };
        kb.keys
            .map
            .iter()
            .flatten()
            .zip(l.0.iter())
            .enumerate()
            .map(|(i, (kc, c))| {
                (
                    kc.clone(),
                    Some(KeyData {
                        letter: corpus.uncorpus_unigram(*c),
                        frequency: match style {
                            ColorStyle::Frequency => freqs[i],
                            ColorStyle::Metric => freqs[i],
                            _ => 0.0,
                        },
                    }),
                )
            })
            .collect()
    }
    pub fn update_keys(&mut self, ctx: &MetricContext, metric: usize) {
        self.keys = Self::keys(ctx, self.style, metric);
    }
    pub fn new(ctx: &MetricContext, style: ColorStyle, metric: usize) -> Self {
        let kb = &ctx.keyboard;
        let lowest_y = kb
            .keys
            .map
            .iter()
            .flatten()
            .map(|kc| (kc.y * 100.0).ceil() as i32)
            .min()
            .unwrap() as f32
            / 100.0;
        let highest_y = kb
            .keys
            .map
            .iter()
            .flatten()
            .map(|kc| (kc.y * 100.0).ceil() as i32)
            .max()
            .unwrap() as f32
            / 100.0;
        let highest_x = kb
            .keys
            .map
            .iter()
            .flatten()
            .map(|kc| (kc.x * 100.0).ceil() as i32)
            .max()
            .unwrap() as f32
            / 100.0;
        // TODO this sucks
        let lowest_x = kb
            .keys
            .map
            .iter()
            .flatten()
            .map(|kc| (kc.x * 100.0).ceil() as i32)
            .min()
            .unwrap() as f32
            / 100.0;
        Self {
            keys: Self::keys(ctx, style, metric),
            lowest_y,
            highest_y,
            highest_x,
            lowest_x,
            style: ColorStyle::Frequency,
            cache: Cache::default(),
        }
    }

    pub fn redraw(&mut self) {
        self.cache.clear();
    }
}

impl canvas::Program<Message> for LayoutDisplay {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let width = 1.0 + self.highest_x - self.lowest_x;
        let height = 1.0 + self.highest_y - self.lowest_y;
        let width_provided = (0.95 * bounds.width).min(500.0);
        let height_provided = (0.95 * bounds.height).min(500.0);
        let x_offset = (bounds.width - width_provided) / 2.0;
        let y_offset = (bounds.height - height_provided) / 2.0;
        let scale = (width_provided / width).min(height_provided / height);
        let key_size = scale * 0.9;

        let display = self.cache.draw(renderer, bounds.size(), |frame| {
            for (key, data) in &self.keys {
                let color = match self.style {
                    ColorStyle::None => Color::from_rgb(0.8, 0.8, 0.8),
                    ColorStyle::Frequency => {
                        if let Some(data) = &data {
                            let f = data.frequency;
                            Color::from_rgb(f / 1.5, f / 1.5, f)
                        } else {
                            Color::from_rgb(0.3, 0.3, 0.3)
                        }
                    }
                    ColorStyle::Metric => {
                        if let Some(data) = &data {
                            let f = data.frequency;
                            Color::from_rgb(f / 1.2, f / 1.5, f / 1.0)
                        } else {
                            Color::from_rgb(0.3, 0.3, 0.3)
                        }
                    }
                    ColorStyle::Fingers => color_from_finger(key.finger),
                };
                frame.fill_rectangle(
                    Point::new(x_offset + scale * key.x, y_offset + scale * key.y),
                    Size::new(key_size, key_size),
                    color,
                );
                if let Some(data) = data {
                    let mut text = Text::from(match data.letter {
                        ' ' => "␣".to_string(),
                        '\0' => "".to_string(),
                        _ => data.letter.to_string(),
                    });
                    let bx = x_offset + key.x * scale;
                    let by = y_offset + key.y * scale;
                    text.position =
                        Point::new((2.0 * bx + key_size) / 2.0, (2.0 * by + key_size) / 2.0);
                    text.horizontal_alignment = Horizontal::Center;
                    text.vertical_alignment = Vertical::Center;
                    text.size = iced::Pixels(0.5 * scale);
                    frame.fill_text(text)
                }
            }
        });
        vec![display]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        _event: canvas::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        (canvas::event::Status::Ignored, None)
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}
