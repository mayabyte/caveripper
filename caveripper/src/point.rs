use itertools::Itertools;
use num::{traits::real::Real, zero, Float, Zero};
use serde::{ser::SerializeSeq, Serialize};
use std::{
    fmt::Display,
    iter::Sum,
    ops::{Add, AddAssign, Div, Index, IndexMut, Mul, Neg, Sub},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point<const N: usize, T>(pub [T; N]);

impl<const N: usize, T> Point<N, T> {
    /// Euclidean distance between two Points.
    pub fn dist(&self, other: &Self) -> T
    where
        T: Real + AddAssign<T>,
    {
        let mut sum = zero();
        for i in 0..N {
            let delta = self.0[i] - other.0[i];
            sum += delta * delta;
        }
        <T as Real>::sqrt(sum)
    }

    pub fn length(&self) -> T
    where
        T: Real + AddAssign<T> + Zero,
    {
        self.dist(&Point([zero(); N]))
    }

    /// Scale all values to be in the range [0,1].
    pub fn normalized(mut self) -> Self
    where
        T: Real + Zero<Output = T> + AddAssign<T>,
    {
        let factor = Point([<T as Zero>::zero(); N]).dist(&self);
        for v in self.0.iter_mut() {
            *v = *v / factor;
        }
        self
    }

    pub fn swap(&mut self, d1: usize, d2: usize)
    where
        T: Copy,
    {
        (self[d1], self[d2]) = (self[d2], self[d1]);
    }

    pub fn dot(self, other: Self) -> T
    where
        T: Mul + Sum<<T as Mul>::Output>,
    {
        self.0
            .into_iter()
            .zip(other.0.into_iter())
            .map(|(s, o)| s * o)
            .sum()
    }
}

impl<T> Point<3, T> {
    /// Rotates this point about the given point in the XZ plane only.
    pub fn rotate_about_xz(&self, pivot: Point<2, T>, angle_rad: T) -> Self
    where
        T: Copy + Float,
    {
        let p = self.two_d().rotate_about(pivot, angle_rad);
        Point([p[0], self[1], p[1]])
    }

    pub fn two_d(&self) -> Point<2, T>
    where
        T: Copy,
    {
        Point([self[0], self[2]])
    }
}

impl<T> Point<2, T> {
    /// Produces a vector with perpendicular slope. Most useful for normals.
    pub fn perpendicular(&self) -> Self
    where
        T: Copy + Neg<Output = T>,
    {
        Self([-self[1], self[0]])
    }

    /// Rotates this point about the given point.
    pub fn rotate_about(&self, pivot: Point<2, T>, angle_rad: T) -> Self
    where
        T: Copy + Float,
    {
        // Translate such that the pivot point is at the origin
        let p = *self - pivot;

        // Rotate around the origin
        let sin = angle_rad.sin();
        let cos = angle_rad.cos();
        let p = Point([p[0] * cos - p[1] * sin, p[0] * sin + p[1] * cos]);

        // Undo the translation to origin and return
        p + pivot
    }
}

impl<const N: usize> Point<N, f32> {
    /// Like [dist], but uses Pikmin 2's fast inverse sqrt.
    pub fn p2_dist(&self, other: &Self) -> f32 {
        let mut sum = Zero::zero();
        for i in 0..N {
            let delta = self.0[i] - other.0[i];
            sum += delta * delta;
        }
        crate::pikmin_math::sqrt(sum)
    }

    pub fn p2_length(&self) -> f32 {
        self.p2_dist(&Point([0.0; N]))
    }
}

impl<const N: usize, T> Index<usize> for Point<N, T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<const N: usize, T> IndexMut<usize> for Point<N, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl<const N: usize, T: Display> Display for Point<N, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({})", self.0.iter().join(", "))
    }
}

impl<const N: usize, T: Default + Copy> Default for Point<N, T> {
    fn default() -> Self {
        Self([T::default(); N])
    }
}

/// Conversion from 3D to 2D coordinates by removing Y.
impl<T: Copy> From<Point<3, T>> for Point<2, T> {
    fn from(value: Point<3, T>) -> Self {
        Self([value[0], value[2]])
    }
}

impl<const N: usize, T: Add<Output = T> + Copy> Add for Point<N, T> {
    type Output = Self;
    fn add(mut self, rhs: Self) -> Self::Output {
        for i in 0..N {
            self[i] = self[i] + rhs[i];
        }
        self
    }
}

impl<const N: usize, T: Add<Output = T> + Copy> AddAssign for Point<N, T> {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<const N: usize, T: Sub<Output = T> + Copy> Sub for Point<N, T> {
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self::Output {
        for i in 0..N {
            self[i] = self[i] - rhs[i];
        }
        self
    }
}

impl<const N: usize, T: Sub<Output = T> + Copy> Sub<T> for Point<N, T> {
    type Output = Self;
    fn sub(mut self, rhs: T) -> Self::Output {
        for i in 0..N {
            self[i] = self[i] - rhs;
        }
        self
    }
}

impl<const N: usize, T: Mul<Output = T> + Copy> Mul<Point<N, T>> for Point<N, T> {
    type Output = Self;
    fn mul(mut self, rhs: Self) -> Self::Output {
        for i in 0..N {
            self[i] = self[i] * rhs[i];
        }
        self
    }
}

impl<const N: usize, T: Mul<Output = T> + Copy> Mul<T> for Point<N, T> {
    type Output = Self;
    fn mul(mut self, rhs: T) -> Self::Output {
        for i in 0..N {
            self[i] = self[i] * rhs;
        }
        self
    }
}

impl<const N: usize, T: Div<Output = T> + Copy> Div for Point<N, T> {
    type Output = Self;
    fn div(mut self, rhs: Self) -> Self::Output {
        for i in 0..N {
            self[i] = self[i] / rhs[i];
        }
        self
    }
}

impl<const N: usize, T: Div<Output = T> + Copy> Div<T> for Point<N, T> {
    type Output = Self;
    fn div(mut self, rhs: T) -> Self::Output {
        for i in 0..N {
            self[i] = self[i] / rhs;
        }
        self
    }
}

impl<const N: usize, T: Serialize> Serialize for Point<N, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_seq(Some(N))?;
        for i in 0..N {
            state.serialize_element(&self[i])?;
        }
        state.end()
    }
}
