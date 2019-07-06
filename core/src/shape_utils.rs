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
pub struct Point {
    x: Twips,
    y: Twips,
    is_bezier_control: bool,
}

/// A path segment is a series of edges linked togerther.
/// Fill paths are directed, because the winding determines the fill-rule.
/// Stroke paths are undirected.
#[derive(Debug)]
struct PathSegment {
    pub points: Vec<Point>,
}

impl PathSegment {
    fn new(start: (Twips, Twips)) -> Self {
        Self {
            points: vec![Point { x: start.0, y: start.1, is_bezier_control: false }],
        }
    }

    /// Flips the direction of the path segment.
    /// Flash fill paths are dual-sided, with fill style 1 indicating the positive side
    /// and fill style 0 indicating the negative. We have to flip fill style 0 paths
    /// in order to link them to fill style 1 paths.
    fn flip(&mut self) {
        self.points.reverse();
    }

    /// Adds an edge to the end of the path segment.
    fn add_point(&mut self, point: Point) {
        self.points.push(point);
    }

    fn is_empty(&self) -> bool {
        self.points.len() <= 1
    }

    fn start(&self) -> (Twips, Twips) {
        let pt = &self.points.first().unwrap();
        (pt.x, pt.y)
    }

    fn end(&self) -> (Twips, Twips) {
        let pt = &self.points.last().unwrap();
        (pt.x, pt.y)
    }

    /// Attemps to merge another path segment.
    /// One path's start must meet the other path's end.
    /// Returns true if the merge is successful.
    fn try_merge(&mut self, other: &mut PathSegment, directed: bool) -> bool {
        if other.end() == self.start() {
            std::mem::swap(&mut self.points, &mut other.points);
            self.points.extend_from_slice(&other.points[..]);
            true
        } else if self.end() == other.start() {
            self.points.extend_from_slice(&other.points[..]);
            true
        } else if !directed && self.end() == other.end() {
            other.flip();
            self.points.extend_from_slice(&other.points[..]);
            true
        } else if !directed && self.start() == other.start() {
            other.flip();
            std::mem::swap(&mut self.points, &mut other.points);
            self.points.extend_from_slice(&other.points[..]);
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
struct PendingPath {
    // /// The fill or stroke style associated with this path.
    // style: &'a T,

    // /// The ID associated with the above style. Used for hashing/lookups.
    // /// The IDs are reset whenever a StyleChangeRecord is reached that contains new styles.
    // style_id: NonZeroU32,

    /// The list of path segments for this fill/stroke.
    /// For fills, this should turn into a list of closed paths when the shape is complete.
    /// Strokes may or may not be closed.
    segments: Vec<PathSegment>,
}


impl PendingPath {
    fn new() -> Self {
        Self { segments: vec![] }
    }

    fn merge_path(&mut self, mut new_segment: PathSegment, directed: bool) {
        if !new_segment.is_empty() {
            if let Some(i) = self.segments.iter_mut().position(|segment| segment.try_merge(&mut new_segment, directed)) {
                new_segment = self.segments.swap_remove(i);
                self.merge_path(new_segment, directed);
            } else {
                // Couldn't merge the segment any further to an existing segment. Add it to list.
                self.segments.push(new_segment);
            }
        }
    }
}

/// DrawCommands are the output of the ShapeCoverter.
#[derive(Debug)]
pub enum DrawPath<'a> {
    Stroke {
        style: &'a LineStyle,
        commands: Vec<DrawCommand>,
    },
    Fill {
        style: &'a FillStyle,
        commands: Vec<DrawCommand>,
    },
}

#[derive(Debug)]
pub enum DrawCommand {
    MoveTo {
        x: Twips,
        y: Twips,
    },
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

/// `PendingPathMap` maps from style IDs to the path associated with that style.
/// Each path is uniquely identified by its style ID (until the style list changes).
/// Style IDs tend to be sequential, so we just use a `Vec`.
#[derive(Debug)]
pub struct PendingPathMap(HashMap<NonZeroU32, PendingPath>);

impl PendingPathMap {
    fn new() -> Self {
        Self(HashMap::new())
    }

    fn merge_path(&mut self, path: ActivePath, directed: bool) {
        let pending_path = self.0.entry(path.style_id).or_insert_with(PendingPath::new);
        pending_path.merge_path(path.segment, directed);
    }
}

#[derive(Debug)]
pub struct ActivePath {
    style_id: NonZeroU32,
    segment: PathSegment,
}

impl ActivePath {
    fn new(style_id: NonZeroU32, start: (Twips, Twips)) -> Self {
        Self {
            style_id,
            segment: PathSegment::new(start),
        }
    }

    fn add_point(&mut self, point: Point) {
        self.segment.add_point(point)
    }

    fn flip(&mut self) {
        self.segment.flip()
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

    fill_style0: Option<ActivePath>,
    fill_style1: Option<ActivePath>,
    line_style: Option<ActivePath>,

    // Paths. These get flushed when the shape is complete
    // and for each new layer.
    fills: PendingPathMap,
    strokes: PendingPathMap,

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
                        if let Some(path) = self.fill_style1.take() {
                            self.fills.merge_path(path, true);
                        }

                        self.fill_style1 = if fs != 0 {
                            let id = NonZeroU32::new(fs).unwrap();
                            Some(ActivePath::new(id, (self.x, self.y)))
                        } else {
                            None
                        }
                    }

                    if let Some(fs) = style_change.fill_style_0 {
                        if let Some(mut path) = self.fill_style0.take() {
                            if !path.segment.is_empty() {
                                path.flip();
                                self.fills.merge_path(path, true);
                            }
                        }

                        self.fill_style0 = if fs != 0 {
                            let id = NonZeroU32::new(fs).unwrap();
                            Some(ActivePath::new(id, (self.x, self.y)))
                        } else {
                            None
                        }
                    }

                    if let Some(ls) = style_change.line_style {
                        if let Some(path) = self.line_style.take() {
                            self.strokes.merge_path(path, false);
                        }

                        self.line_style = if ls != 0 {
                            let id = NonZeroU32::new(ls).unwrap();
                            Some(ActivePath::new(id, (self.x, self.y)))
                        } else {
                            None
                        }
                    }
                }

                ShapeRecord::StraightEdge { delta_x, delta_y } => {
                    self.x += *delta_x;
                    self.y += *delta_y;

                    self.visit_point(Point {
                        x: self.x,
                        y: self.y,
                        is_bezier_control: false,
                    });
                }

                ShapeRecord::CurvedEdge {
                    control_delta_x,
                    control_delta_y,
                    anchor_delta_x,
                    anchor_delta_y,
                } => {
                    let x1 = self.x + *control_delta_x;
                    let y1 = self.y + *control_delta_y;

                    self.visit_point(Point {
                        x: x1,
                        y: y1,
                        is_bezier_control: true,
                    });

                    let x2 = x1 + *anchor_delta_x;
                    let y2 = y1 + *anchor_delta_y;

                    self.visit_point(Point {
                        x: x2,
                        y: y2,
                        is_bezier_control: false,
                    });

                    self.x = x2;
                    self.y = y2;
                }
            }
        }

        // Flush any open paths.
        self.flush_layer();
        println!("{:#?}", self.commands);
        self.commands
    }

    /// Adds a point to the current path for the active fills/strokes.
    fn visit_point(&mut self, point: Point) {
        if let Some(path) = &mut self.fill_style0 {
            path.add_point(point)
        }

        if let Some(path) = &mut self.fill_style1 {
            path.add_point(point)
        }

        if let Some(path) = &mut self.line_style {
            path.add_point(point)
        }
    }

    /// When the pen jumps to a new position, we reset the active path.
    fn flush_paths(&mut self) {
        // Move the current paths to the active list.
        if let Some(path) = self.fill_style1.take() {
            self.fill_style1 = Some(ActivePath::new(path.style_id, (self.x, self.y)));
            self.fills.merge_path(path, true);
        }

        if let Some(mut path) = self.fill_style0.take() {
            self.fill_style0 = Some(ActivePath::new(path.style_id, (self.x, self.y)));
            if !path.segment.is_empty() {
                path.flip();
                self.fills.merge_path(path, true);
            }
        }

        if let Some(path) = self.line_style.take() {
            self.line_style = Some(ActivePath::new(path.style_id, (self.x, self.y)));
            self.strokes.merge_path(path, false);
        }
    }

    /// When a new layer starts, all paths are flushed and turned into drawing commands.
    fn flush_layer(self: &mut Self) {
        self.flush_paths();
        self.fill_style0 = None;
        self.fill_style1 = None;
        self.line_style = None;

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
        for (style_id, path) in fills {
            assert!(style_id.get() > 0);
            let style = &self.fill_styles[style_id.get() as usize - 1];
            let mut commands = vec![];
            if !path.segments.is_empty() {
                for segment in path.segments {
                    if segment.is_empty() {
                        continue;
                    }

                    let mut points = segment.points.into_iter();
                    let start = points.next().unwrap();
                    commands.push(DrawCommand::MoveTo {
                        x: start.x,
                        y: start.y,
                    });
                    while let Some(point) = points.next() {
                        let command = if point.is_bezier_control {
                            let point2 = points.next().unwrap();
                            DrawCommand::CurveTo { x1: point.x, y1: point.y, x2: point2.x, y2: point2.y }
                        } else {
                            DrawCommand::LineTo { x: point.x, y: point.y }
                        };
                        commands.push(command);
                    }
                }
            }
            self.commands.push(DrawPath::Fill { style, commands });
        }
        for (style_id, path) in strokes {
            assert!(style_id.get() > 0);
            let style = &self.line_styles[style_id.get() as usize - 1];
            let mut commands = vec![];
            if !path.segments.is_empty() {
                for segment in path.segments {
                    if segment.is_empty() {
                        continue;
                    }

                    let mut points = segment.points.into_iter();
                    let start = points.next().unwrap();
                    commands.push(DrawCommand::MoveTo {
                        x: start.x,
                        y: start.y,
                    });
                    while let Some(point) = points.next() {
                        let command = if point.is_bezier_control {
                            let point2 = points.next().unwrap();
                            DrawCommand::CurveTo { x1: point.x, y1: point.y, x2: point2.x, y2: point2.y }
                        } else {
                            DrawCommand::LineTo { x: point.x, y: point.y }
                        };
                        commands.push(command);
                    }
                }
                self.commands.push(DrawPath::Stroke { style, commands });
            }
        }

        self.fills.0.clear();
        self.strokes.0.clear();
    }
}
