use std::collections::HashMap;
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
struct PathSegment {
    pub edges: Vec<Edge>,
    pub start: (Twips, Twips),
    pub end: (Twips, Twips),
}

#[derive(Debug)]
pub struct Path<'a, T> {
    pub style: &'a T,
    pub style_id: NonZeroU32,
    segments: Vec<PathSegment>,
}


impl<'a, T> Path<'a, T> {
    fn new(style: &'a T, style_id: NonZeroU32) -> Self {
        Self {
            style,
            style_id,

            segments: vec![],
        }
    }

    fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
        self.end = match edge {
            Edge::LineTo { x, y } => (x, y),
            Edge::CurveTo { x2, y2, .. } => (x2, y2),
        };
    }

    fn flip(&mut self) {
        self.edges.reverse();
        // for edge in &mut self.edges {
        //     match edge {
        //         Edge::LineTo { x, y } => (),
        //         Edge::CurveTo { x1, y1, x2, y2 } => {

        //         }
        //     }
        // }
        std::mem::swap(&mut self.start, &mut self.end);
    }

    fn try_merge(&mut self, path: &Path<'a, T>) -> bool {
        if self.style_id != path.style_id {
            false
        } else if path.end == self.start {
            let mut edges = path.edges.clone();
            edges.extend_from_slice(self.edges.as_slice());
            self.edges = edges;
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

    // fn try_merge_reverse(&mut self, path: &Path<'a, T>) -> bool {
    //     if self.style_id != path.style_id {
    //         false
    //     } else if path.start == self.start {
    //         self.start = path.end;
    //         true
    //     } else if self.end == path.start {
    //         self.edges.extend_from_slice(path.edges.as_slice());
    //         self.end = path.end;
    //         true
    //     } else {
    //         false
    //     }
    // }

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
pub struct ActivePaths<'a, T>(HashMap<NonZeroU32, Vec<Path<'a, T>>>);

impl<'a, T> ActivePaths<'a, T> {
    fn new() -> Self {
        Self(HashMap::new())
    }

    fn merge_path(&mut self, mut path: Path<'a, T>) {
        let mut style_paths = self.0.entry(path.style_id).or_insert_with(|| Vec::with_capacity(1));
        for other in style_paths.iter_mut() {
            if other.try_merge(&mut path) {
                return;
            }
        }

        style_paths.push(path);
    }

    fn merge_path_undirected(&mut self, mut path: Path<'a, T>) {
        let mut style_paths = self.0.entry(path.style_id).or_insert_with(|| Vec::with_capacity(1));
        for other in style_paths.iter_mut() {
            if other.try_merge_undirected(&mut path) {
                return;
            }
        }

        style_paths.push(path);
    }
}

pub struct ShapeConverter<'a> {
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

    // Output.
    commands: Vec<DrawCommand<'a>>,
}

impl<'a> ShapeConverter<'a> {
    const DEFAULT_CAPACITY: usize = 512;

    pub fn from_shape(shape: &'a swf::Shape) -> Self {
        ShapeConverter {
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

            commands: Vec::with_capacity(Self::DEFAULT_CAPACITY),
        }
    }

    pub fn into_commands(mut self) -> Vec<DrawCommand<'a>> {
        while let Some(record) = self.iter.next() {
            match record {
                ShapeRecord::StyleChange(style_change) => {
                    if let Some((x, y)) = style_change.move_to {
                        self.x = x;
                        self.y = y;
                        // We've lifted the pen, so we're starting a new path.
                        // Flush the previous path.
                        self.flush_paths();
                    }

                    if let Some(ref styles) = style_change.new_styles {
                        // A new style list is also used to indicate a new drawing layer.
                        self.flush_layer();
                        self.fill_styles = &styles.fill_styles[..];
                        self.line_styles = &styles.line_styles[..];
                    }

                    if let Some(fs) = style_change.fill_style_1 {
                        if let Some(path) = self.fill_style1.take() {
                            self.fills.merge_path(path);
                        }

                        self.fill_style1 = if fs != 0 {
                            let fs_index = NonZeroU32::new(fs).unwrap();
                            let fill_style = &self.fill_styles[fs as usize - 1];
                            Some(Path::new(fill_style, fs_index, (self.x, self.y)))
                        } else {
                            None
                        }
                    }

                    if let Some(fs) = style_change.fill_style_0 {
                        if let Some(path) = self.fill_style0.take() {
                            self.fills.merge_path(path);
                        }

                        self.fill_style0 = if fs != 0 {
                            let fs_index = NonZeroU32::new(fs).unwrap();
                            let fill_style = &self.fill_styles[fs as usize - 1];
                            Some(Path::new(fill_style, fs_index, (self.x, self.y)))
                        } else {
                            None
                        }
                    }

                    if let Some(ls) = style_change.line_style {
                        if let Some(path) = self.line_style.take() {
                            self.strokes.merge_path_undirected(path);
                        }

                        self.line_style = if ls != 0 {
                            let ls_index = NonZeroU32::new(ls).unwrap();
                            let line_style = &self.line_styles[ls as usize - 1];
                            Some(Path::new(line_style, ls_index, (self.x, self.y)))
                        } else {
                            None
                        }
                    }
                }

                ShapeRecord::StraightEdge { delta_x, delta_y } => {
                    self.x += *delta_x;
                    self.y += *delta_y;
                    let edge = Edge::LineTo {
                        x: self.x,
                        y: self.y,
                    };

                    self.visit_edge(edge);
                }

                ShapeRecord::CurvedEdge {
                    control_delta_x,
                    control_delta_y,
                    anchor_delta_x,
                    anchor_delta_y,
                } => {
                    let x1 = self.x + *control_delta_x;
                    let y1 = self.y + *control_delta_y;
                    let x2 = x1 + *anchor_delta_x;
                    let y2 = y1 + *anchor_delta_y;
                    self.x = x2;
                    self.y = y2;
                    let edge = Edge::CurveTo { x1, y1, x2, y2 };

                    self.visit_edge(edge);
                }
            }
        }

        // Flush any open paths.
        self.flush_layer();

        self.commands
    }

    /// Adds an edge to the current path for the active fills/strokes.
    fn visit_edge(&mut self, edge: Edge) {
        if let Some(path) = &mut self.fill_style0 {
            path.add_edge(edge)
        }

        if let Some(path) = &mut self.fill_style1 {
            path.add_edge(edge)
        }

        if let Some(path) = &mut self.line_style {
            path.add_edge(edge)
        }
    }

    /// When the pen jumps to a new position, we reset the active path.
    fn flush_paths(&mut self) {
        // Move the current paths to the active list.
        if let Some(path) = self.fill_style1.take() {
            self.fills.merge_path(path);
        }

        if let Some(mut path) = self.fill_style0.take() {
            path.flip();
            self.fills.merge_path(path);
        }

        if let Some(path) = self.line_style.take() {
            self.strokes.merge_path_undirected(path);
        }
    }

    /// When a new layer starts, all paths are flushed and turned into drawing commands.
    fn flush_layer(self: &mut Self) {
        self.flush_paths();

        let fills = std::mem::replace(&mut self.fills.0, HashMap::new());
        let strokes = std::mem::replace(&mut self.strokes.0, HashMap::new());

        // for fill in &fills {
        //     println!("FILL: {:?}", fill.style);
        //     println!("{:?}", fill.start);
        //     for edge in &fill.edges {
        //         println!("{:?}", edge);
        //     }
        // }

        // Draw fills, and then strokes.
        // Strokes in the same layer always appear on top of fills.
        self.commands
            .extend(fills.values().map(DrawCommand::Fill));
        self.commands
            .extend(strokes.into_iter().map(DrawCommand::Stroke));
    }
}
