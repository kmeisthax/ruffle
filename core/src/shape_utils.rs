use std::num::NonZeroU32;
use swf::{FillStyle, LineStyle, ShapeRecord, Twips};

pub fn calculate_shape_bounds(shape_records: &[swf::ShapeRecord]) -> swf::Rectangle {
    let mut bounds = swf::Rectangle {
        x_min: Twips::new(std::i32::MAX),
        y_min: Twips::new(std::i32::MAX),
        x_max: Twips::new(std::i32::MIN),
        y_max: Twips::new(std::i32::MIN),
    };
    let mut x = Twips::new(0);
    let mut y = Twips::new(0);
    for record in shape_records {
        match record {
            swf::ShapeRecord::StyleChange(style_change) => {
                if let Some((move_x, move_y)) = style_change.move_to {
                    x = move_x;
                    y = move_y;
                    bounds.x_min = Twips::min(bounds.x_min, x);
                    bounds.x_max = Twips::max(bounds.x_max, x);
                    bounds.y_min = Twips::min(bounds.y_min, y);
                    bounds.y_max = Twips::max(bounds.y_max, y);
                }
            }
            swf::ShapeRecord::StraightEdge { delta_x, delta_y } => {
                x += *delta_x;
                y += *delta_y;
                bounds.x_min = Twips::min(bounds.x_min, x);
                bounds.x_max = Twips::max(bounds.x_max, x);
                bounds.y_min = Twips::min(bounds.y_min, y);
                bounds.y_max = Twips::max(bounds.y_max, y);
            }
            swf::ShapeRecord::CurvedEdge {
                control_delta_x,
                control_delta_y,
                anchor_delta_x,
                anchor_delta_y,
            } => {
                x += *control_delta_x;
                y += *control_delta_y;
                bounds.x_min = Twips::min(bounds.x_min, x);
                bounds.x_max = Twips::max(bounds.x_max, x);
                bounds.y_min = Twips::min(bounds.y_min, y);
                bounds.y_max = Twips::max(bounds.y_max, y);
                x += *anchor_delta_x;
                y += *anchor_delta_y;
                bounds.x_min = Twips::min(bounds.x_min, x);
                bounds.x_max = Twips::max(bounds.x_max, x);
                bounds.y_min = Twips::min(bounds.y_min, y);
                bounds.y_max = Twips::max(bounds.y_max, y);
            }
        }
    }
    if bounds.x_max < bounds.x_min || bounds.y_max < bounds.y_min {
        bounds = Default::default();
    }
    bounds
}

#[derive(Debug, Copy, Clone)]
pub enum Edge {
    LineTo {
        x: Twips,
        y: Twips,
    },
    CurveTo {
        x1: Twips,
        y1: Twips,
        x2: Twips,
        y2: Twips,
    },
}

#[derive(Debug)]
pub struct Path<'a, T> {
    style: &'a T,
    style_id: NonZeroU32,

    start: (Twips, Twips),
    end: (Twips, Twips),
    edges: Vec<Edge>,
}

impl<'a, T> Path<'a, T> {
    fn new(style: &'a T, style_id: NonZeroU32) -> Self {
        Self {
            style,
            style_id,

            start: Default::default(),
            end: Default::default(),
            edges: vec![],
        }
    }

    fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge)
    }

    fn flip(&mut self) {
        self.edges.reverse();
        std::mem::swap(&mut self.start, &mut self.end);
    }

    fn try_merge(&mut self, path: &Path<'a, T>) -> bool {
        if self.style_id != path.style_id {
            false
        } else if path.end == self.start {
            self.start = path.start;
            true
        } else if self.end == path.start {
            self.edges.extend_from_slice(path.edges.as_slice());
            self.end = path.end;
            true
        } else {
            false
        }
    }

    fn try_merge_undirected(&mut self, path: &Path<'a, T>) -> bool {
        if self.style_id != path.style_id {
            false
        } else if path.end == self.start {
            self.start = path.start;
            true
        } else if self.end == path.start {
            self.edges.extend_from_slice(path.edges.as_slice());
            self.end = path.end;
            true
        } else if path.start == self.start {
            self.start = path.start;
            true
        } else if self.end == path.end {
            self.edges.extend_from_slice(path.edges.as_slice());
            self.end = path.end;
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub enum DrawCommand<'a> {
    Stroke(Path<'a, LineStyle>),
    Fill(Path<'a, FillStyle>),
}

#[derive(Debug)]
pub struct ActivePaths<'a, T>(Vec<Path<'a, T>>);

impl<'a, T> ActivePaths<'a, T> {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn merge_path(&mut self, mut path: Path<'a, T>) {
        for other in self.0.iter_mut() {
            if other.try_merge(&mut path) {
                return;
            }
        }

        self.0.push(path);
    }
}

pub struct DrawShapeIter<'a> {
    // SWF shape commands.
    iter: std::slice::Iter<'a, swf::ShapeRecord>,

    // Pen position.
    x: Twips,
    y: Twips,

    // Fill styles and line styles.
    // These change from StyleChangeRecords, and a flush occurs when these change.
    fill_styles: &'a [swf::FillStyle],
    line_styles: &'a [swf::LineStyle],

    // The current path that the pen is drawing.
    // Each edge command gets added to the appropriate path, and when the pen lifts,
    // the path will be merged into the active paths.
    fill_style0: Option<Path<'a, FillStyle>>, // Negative side of pen -- path gets flipped
    fill_style1: Option<Path<'a, FillStyle>>, // Positive side of pen
    line_style: Option<Path<'a, LineStyle>>,  // Undirected path

    // Paths. These get flushed when the shape is complete
    // and for each new layer.
    fills: ActivePaths<'a, FillStyle>,
    strokes: ActivePaths<'a, LineStyle>,
}

fn swf_shape_to_draws(shape: &swf::Shape) -> impl Iterator<Item = DrawCommand> {
    let mut state = DrawShapeIter {
        iter: shape.shape.iter(),

        x: Twips::new(0),
        y: Twips::new(0),

        fill_styles: &shape.styles.fill_styles,
        line_styles: &shape.styles.line_styles,

        fill_style0: None,
        fill_style1: None,
        line_style: None,

        fills: ActivePaths::new(),
        strokes: ActivePaths::new(),
    };

    fn visit_edge(state: &mut DrawShapeIter, edge: Edge) {
        state.fill_style0.as_mut().map(|path| path.add_edge(edge));
        state.fill_style1.as_mut().map(|path| path.add_edge(edge));
        state.line_style.as_mut().map(|path| path.add_edge(edge));
    }

    fn flush_paths(state: &mut DrawShapeIter) {}

    fn flush_layer(state: &mut DrawShapeIter) {}

    std::iter::from_fn(move || {
        if let Some(record) = state.iter.next() {
            match record {
                ShapeRecord::StyleChange(style_change) => {
                    if let Some(ref styles) = style_change.new_styles {
                        flush_layer(&mut state);
                        state.fill_styles = &styles.fill_styles[..];
                        state.line_styles = &styles.line_styles[..];
                    }

                    if let Some((dx, dy)) = style_change.move_to {
                        state.x += dx;
                        state.y += dy;
                        // We've lifted the pen, so we're starting a new path.
                        // Flush the previous path.
                        flush_paths(&mut state);
                    }

                    if let Some(fs) = style_change.fill_style_1 {
                        let fill_style = &state.fill_styles[fs as usize - 1];
                        let new_path = if fs != 0 { Some(Path::new(fill_style, NonZeroU32::new(fs).unwrap()) } else { None };
                        if let Some(path) = state.fill_style1.replace(new_path) {
                            state.fills.merge_path(path);
                        }
                    }

                    if let Some(fs) = style_change.fill_style_0 {

                    }

                    if let Some(ls) = style_change.line_style {

                    }
                }

                ShapeRecord::StraightEdge { delta_x, delta_y } => {
                    state.x += *delta_x;
                    state.y += *delta_y;
                    let edge = Edge::LineTo {
                        x: state.x,
                        y: state.y,
                    };

                    visit_edge(&mut state, edge);
                }

                ShapeRecord::CurvedEdge {
                    control_delta_x,
                    control_delta_y,
                    anchor_delta_x,
                    anchor_delta_y,
                } => {
                    let x1 = state.x + *control_delta_x;
                    let y1 = state.y + *control_delta_y;
                    let x2 = x1 + *anchor_delta_x;
                    let y2 = y1 + *anchor_delta_y;
                    state.x = x2;
                    state.y = y2;
                    let edge = Edge::CurveTo { x1, y1, x2, y2 };

                    visit_edge(&mut state, edge);
                }
            }
        }

        None
    })
}
