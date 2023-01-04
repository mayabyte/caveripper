use num::{Zero, traits::real::Real};
use std::ops::{Add, Mul, Div, Sub, AddAssign, Index, Neg};

#[derive(Debug)]
pub struct Point<const N: usize, T> {
    pub values: [T; N]
}

impl<const N: usize, T> Point<N, T> {
    /// Euclidean distance between two Points.
    pub fn dist(&self, other: &Self) -> T
    where T: Real + AddAssign<T>
    {
        let mut sum = Zero::zero();
        for i in 0..N {
            let delta = self.values[i] - other.values[i];
            sum += delta * delta;
        }
        <T as Real>::sqrt(sum)
    }

    /// Scale all values to be in the range [0,1].
    pub fn normal(mut self) -> Self
    where T: Real + Zero<Output=T> + AddAssign<T>,
    {
        let factor = Point{values:[<T as Zero>::zero();N]}.dist(&self);
        for v in self.values.iter_mut() {
            *v = *v / factor;
        }
        self
    }
}

impl<T> Point<2, T> {
    /// Produces a vector with perpendicular slope. Most useful for normals.
    pub fn perpendicular(&self) -> Self
    where T: Copy + Neg<Output=T>
    {
        Self{values:[-self[1], self[0]]}
    }
}

impl<const N: usize, T> Index<usize> for Point<N, T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

impl<const N: usize, T: Clone> Clone for Point<N, T> {
    fn clone(&self) -> Self {
        Self { values: self.values.clone() }
    }
}

impl<const N: usize, T: Copy + Clone> Copy for Point<N, T> {}

impl<const N: usize, T: Add<Output=T> + Copy> Add for Point<N, T> {
    type Output = Self;
    fn add(mut self, rhs: Self) -> Self::Output {
        for i in 0..N {
            self.values[i] = self.values[i] + rhs.values[i];
        }
        self
    }
}

impl<const N: usize, T: Sub<Output=T> + Copy> Sub for Point<N, T> {
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self::Output {
        for i in 0..N {
            self.values[i] = self.values[i] - rhs.values[i];
        }
        self
    }
}

impl<const N: usize, T: Mul<Output=T> + Copy> Mul<Point<N, T>> for Point<N, T> {
    type Output = Self;
    fn mul(mut self, rhs: Self) -> Self::Output {
        for i in 0..N {
            self.values[i] = self.values[i] * rhs.values[i];
        }
        self
    }
}

impl<const N: usize, T: Mul<Output=T> + Copy> Mul<T> for Point<N, T> {
    type Output = Self;
    fn mul(mut self, rhs: T) -> Self::Output {
        for i in 0..N {
            self.values[i] = self.values[i] * rhs;
        }
        self
    }
}

impl<const N: usize, T: Div<Output=T> + Copy> Div for Point<N, T> {
    type Output = Self;
    fn div(mut self, rhs: Self) -> Self::Output {
        for i in 0..N {
            self.values[i] = self.values[i] / rhs.values[i];
        }
        self
    }
}

impl<const N: usize, T: Div<Output=T> + Copy> Div<T> for Point<N, T> {
    type Output = Self;
    fn div(mut self, rhs: T) -> Self::Output {
        for i in 0..N {
            self.values[i] = self.values[i] / rhs;
        }
        self
    }
}
