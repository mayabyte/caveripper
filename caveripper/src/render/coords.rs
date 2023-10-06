use super::renderer::Render;
use crate::point::Point;

#[derive(Clone, Copy, Debug)]
pub struct Offset {
    pub from: Origin,
    pub amount: Point<2, f32>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Origin {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomCenter,
}

impl Origin {
    /// The delta between the top left of the given object and the chosen origin.
    /// Subtract from the desired position to get the top left coordinate.
    pub fn offset_from_top_left(&self, dims: Point<2, f32>) -> Point<2, f32> {
        match self {
            Origin::TopLeft => Point([0.0, 0.0]),
            Origin::TopCenter => Point([dims[0] / 2.0, 0.0]),
            Origin::TopRight => Point([dims[0], 0.0]),
            Origin::Center => dims / 2.0,
            Origin::CenterLeft => Point([0.0, dims[1] / 2.0]),
            Origin::CenterRight => Point([dims[0], dims[1] / 2.0]),
            Origin::BottomCenter => Point([dims[0] / 2.0, dims[1]]),
        }
    }

    /// Calculates the bounding box occupied by the given renderable placed at `pos`.
    /// `pos` is the non-normalized position provided by the user.
    pub fn to_bounds(&self, renderable: &impl Render, pos: Point<2, f32>) -> Bounds {
        let offset = self.offset_from_top_left(renderable.dimensions());
        let topleft = pos - offset;
        Bounds {
            topleft,
            bottomright: topleft + renderable.dimensions(),
        }
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Bounds {
    pub topleft: Point<2, f32>,
    pub bottomright: Point<2, f32>,
}

fn max_per_dim(a: Point<2, f32>, b: Point<2, f32>) -> Point<2, f32> {
    Point([f32::max(a[0], b[0]), f32::max(a[1], b[1])])
}

fn min_per_dim(a: Point<2, f32>, b: Point<2, f32>) -> Point<2, f32> {
    Point([f32::min(a[0], b[0]), f32::min(a[1], b[1])])
}

impl Bounds {
    pub fn combine(self, other: Bounds) -> Bounds {
        Bounds {
            topleft: min_per_dim(self.topleft, other.topleft),
            bottomright: max_per_dim(self.bottomright, other.bottomright),
        }
    }

    pub fn dims(&self) -> Point<2, f32> {
        self.bottomright - self.topleft
    }

    pub fn expand_by(mut self, amount: f32) -> Bounds {
        self.topleft = self.topleft - amount;
        self.bottomright = self.bottomright + amount;
        self
    }
}
