use std::marker;
use std::sync::mpsc::{Receiver, RecvError, TryRecvError};

pub trait IntoFuture {
    type Future: Future<Item = Self::Item, Error = Self::Error>;
    type Item;
    type Error;

    fn into_future(self) -> Self::Future;
}

impl<F: Future> IntoFuture for F {
    type Future = F;
    type Item = F::Item;
    type Error = F::Error;
    fn into_future(self) -> F {
        self
    }
}

pub trait Future {
    type Item;
    type Error;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self>
    where
        Self: Sized;

    // dyn https://doc.rust-lang.org/book/ch17-02-trait-objects.html
    fn boxed<'a>(self) -> Box<dyn Future<Item = Self::Item, Error = Self::Error> + 'a>
    where
        Self: Sized + 'a,
    {
        Box::new(self)
    }

    fn map<F, U>(self, f: F) -> Map<Self, F>
    where
        F: FnOnce(Self::Item) -> U,
        Self: Sized,
    {
        Map { future: self, f: f }
    }

    fn map_err<F, E>(self, f: F) -> MapErr<Self, F>
    where
        F: FnOnce(Self::Error) -> E,
        Self: Sized,
    {
        MapErr { future: self, f: f }
    }

    fn and_then<F, B>(self, f: F) -> AndThen<Self, B, F>
    // where https://doc.rust-lang.org/book/ch10-02-traits.html#clearer-trait-bounds-with-where-clauses
    where
        F: FnOnce(Self::Item) -> B,
        B: IntoFuture<Error = Self::Error>,
        Self: Sized,
    {
        AndThen {
            future: _AndThen::First(self, f),
        }
    }

    fn or_else<F, B>(self, f: F) -> OrElse<Self, B, F>
    where
        F: FnOnce(Self::Error) -> B,
        B: IntoFuture<Item = Self::Item>,
        Self: Sized,
    {
        OrElse {
            future: _OrElse::First(self, f),
        }
    }

    fn select<B>(self, other: B) -> Select<Self, B::Future>
    where
        B: IntoFuture<Item = Self::Item, Error = Self::Error>,
        Self: Sized,
    {
        Select {
            a: self,
            b: other.into_future(),
        }
    }

    fn join<B>(self, other: B) -> Join<Self, B::Future>
    where
        B: IntoFuture<Error = Self::Error>,
        Self: Sized,
    {
        Join {
            state: _Join::Both(self, other.into_future()),
        }
    }
}

#[derive(Copy, Clone)]
pub struct FutureResult<T, E> {
    inner: Result<T, E>,
}

impl<T, E> IntoFuture for Result<T, E> {
    type Future = FutureResult<T, E>;
    type Item = T;
    type Error = E;

    fn into_future(self) -> FutureResult<T, E> {
        FutureResult { inner: self }
    }
}

impl<T, E> Future for FutureResult<T, E> {
    type Item = T;
    type Error = E;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        Ok(self.inner.ok_or(()))
    }
}

pub struct Map<A, F> {
    future: A,
    f: F,
}

impl<U, A, F> Future for Map<A, F>
where
    A: Future,
    F: FnOnce(A::Item) -> U,
{
    type Item = U;
    type Error = A::Error;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        match self.future.poll() {
            Ok(result) => Ok(result.map(self.f)),
            Err(f) => Err(Map {
                future: f,
                f: self.f,
            }),
        }
    }
}

pub struct MapErr<A, F> {
    future: A,
    f: F,
}

impl<A, E, F> Future for MapErr<A, F>
where
    A: Future,
    F: FnOnce(A::Error) -> E,
{
    type Item = A::Item;
    type Error = E;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        match self.future.poll() {
            Ok(result) => Ok(result.map_err(self.f)),
            Err(f) => Err(MapErr {
                future: f,
                f: self.f,
            }),
        }
    }
}
