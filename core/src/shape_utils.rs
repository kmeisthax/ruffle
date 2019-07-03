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

/// A path segment is a series of edges linked togerther.
/// Fill paths are directed, because the winding determines the fill-rule.
/// Stroke paths are undirected.
#[derive(Debug)]
struct PathSegment {
    pub edges: Vec<Edge>,
    pub start: (Twips, Twips),
    pub end: (Twips, Twips),
}

impl PathSegment {
    fn new(start: (Twips, Twips)) -> Self {
        Self {
            edges: vec![],
            start,
            end: start,
        }
    }

    fn is_closed(&self) -> bool {
        // Flash doesn't contain any explicit "close" instructions (compared to SVG).
        // A stroke is automatically closed if the start meets the end point.
        self.start == self.end
    }

    /// Flips the direction of the path segment.
    /// Flash fill paths are dual-sided, with fill style 1 indicating the positive side
    /// and fill style 0 indicating the negative. We have to flip fill style 0 paths
    /// in order to link them to fill style 1 paths.
    fn flip(&mut self) {

    }

    /// Adds an edge to the end of the path segment.
    fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
        self.end = match edge {
            Edge::LineTo { x, y } => (x, y),
            Edge::CurveTo { x2, y2, .. } => (x2, y2),
        };
    }
    
    /// Attemps to merge another path segment.
    /// One path's start must meet the other path's end.
    /// Returns true if the merge is successful.
    fn try_merge(&mut self, other: PathSegment, directed: bool) -> bool {
        if other.end == self.start {
            let mut edges = other.edges.clone();
            edges.extend_from_slice(self.edges.as_slice());
            self.edges = edges;
            self.start = other.start;
            true
        } else if self.end == other.start {
            self.edges.extend_from_slice(path.edges.as_slice());
            self.end = other.end;
            true
        } else {
            false
        }
    }
}

/// The internal path structure used by ShapeConverter.
/// 
/// Each path is uniquely identified by its fill/stroke style. But Flash gives
/// the path edges as an "edge soup" -- they can arrive in an arbitrary order.
/// We have to link the edges together for each path. This structure contains
/// a list of path segment, and each time a path segment is added, it will try
/// to merge it with an existing segment.
#[derive(Debug)]
pub struct PendingPath<'a, T> {
    /// The fill or stroke style associated with this path.
    style: &'a T,

    /// The ID associated with the above style. Used for hashing/lookups.
    /// The IDs are reset whenever a StyleChangeRecord is reached that contains new styles.
    style_id: NonZeroU32,

    /// The list of path segments for this fill/stroke.
    /// For fills, this should turn into a list of closed paths when the shape is complete.
    /// Strokes may or may not be closed.
    segments: Vec<PathSegment>,
}


impl<'a, T> PendingPath<'a, T> {
    fn new(style: &'a T, style_id: NonZeroU32) -> Self {
        Self {
            segments: vec![],
        }
    }
}

/// DrawCommands are the output of the ShapeCoverter.
#[derive(Debug)]
pub enum DrawPath<'a> {
    Stroke { style: &'a LineStyle, commands: &'a [DrawCommand], is_closed: bool },
    Fill { style: &'a FillStyle, commands: &'a [DrawCommand] },
}

#[derive(Debug)]
pub enum DrawCommand {
    MoveTo { x: Twips, y: Twips },
    LineTo { x: Twips, y: Twips },
    CurveTo { x1: Twips, y1 : Twips, x2: Twips, y2: Twips },
}

/// `PendingPathMap` maps from style IDs to the path associated with that style.
/// Each path is uniquely identified by its style ID (until the style list changes).
/// Style IDs tend to be sequential, so we just use a `Vec`.
#[derive(Debug)]
pub struct PendingPathMap<'a, T>(Vec<PendingPath<'a, T>>);

impl<'a, T> PendingPathMap<'a, T> {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn merge_path(&mut self, style_id: NonZeroU32, mut path_segment: PathSegment) {
        let pending_path = if let Some(pending_path) = self.0.get_mut(style_id.get()) {
            pending_path
        } else {
            let id = style_id.get() as usize;
            if self.0.len() <= id {
                self.0.resize_with(id, || PendingPath::new(&));
            }
            self.0.last_mut()
        };

        pending_path.merge_path(path_segment);
    }

    fn flush() {

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
    fill_style0: Option<(NonZeroU32, PathSegment)>, // Negative side of pen -- path gets flipped
    fill_style1: Option<(NonZeroU32, PathSegment)>, // Positive side of pen
    line_style: Option<(NonZeroU32, PathSegment)>,  // Undirected path

    // Paths. These get flushed when the shape is complete
    // and for each new layer.
    fills: PendingPathMap<'a, FillStyle>,
    strokes: PendingPathMap<'a, LineStyle>,

    // Output.
    commands: Vec<DrawPath<'a>>,
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

            fills: PendingPathMap::new(),
            strokes: PendingPathMap::new(),

            commands: Vec::with_capacity(Self::DEFAULT_CAPACITY),
        }
    }

    pub fn into_commands(mut self) -> Vec<DrawPath<'a>> {
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
                        if let Some((id, segment)) = self.fill_style1.take() {
                            self.fills.merge_path(id, segment);
                        }

                        self.fill_style1 = if fs != 0 {
                            let id = NonZeroU32::new(fs).unwrap();
                            let fill_style = &self.fill_styles[fs as usize - 1];
                            Some((id, PathSegment::new((self.x, self.y))))
                        } else {
                            None
                        }
                    }

                    if let Some(fs) = style_change.fill_style_0 {
                        if let Some((id, segment)) = self.fill_style0.take() {
                            self.fills.merge_path(id, segment);
                        }

                        self.fill_style0 = if fs != 0 {
                            let id = NonZeroU32::new(fs).unwrap();
                            let fill_style = &self.fill_styles[fs as usize - 1];
                            Some((id, PathSegment::new((self.x, self.y))))
                        } else {
                            None
                        }
                    }

                    if let Some(ls) = style_change.line_style {
                        if let Some((id, segment)) = self.line_style.take() {
                            self.strokes.merge_path(id, segment);
                        }

                        self.line_style = if ls != 0 {
                            let id = NonZeroU32::new(ls).unwrap();
                            let line_style = &self.line_styles[ls as usize - 1];
                            Some((id, PathSegment::new((self.x, self.y))))
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
