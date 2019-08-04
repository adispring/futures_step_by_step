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

#[derive(Copy, Clone, Debug)]
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
        Ok(self.inner)
    }
}

#[derive(Copy, Clone, Debug)]
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

pub struct AndThen<A, B, F>
where
    B: IntoFuture,
{
    future: _AndThen<A, B::Future, F>,
}

enum _AndThen<A, B, F> {
    First(A, F),
    Second(B),
}

impl<A, B, F> Future for AndThen<A, B, F>
where
    A: Future,
    B: IntoFuture<Error = A::Error>,
    F: FnOnce(A::Item) -> B,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        let second = match self.future {
            _AndThen::First(a, f) => match a.poll() {
                Ok(Ok(next)) => f(next).into_future(),
                Ok(Err(e)) => return Ok(Err(e)),
                Err(a) => {
                    return Err(AndThen {
                        future: _AndThen::First(a, f),
                    })
                }
            },
            _AndThen::Second(b) => b,
        };
        second.poll().map_err(|b| AndThen {
            future: _AndThen::Second(b),
        })
    }
}

pub struct OrElse<A, B, F>
where
    B: IntoFuture,
{
    future: _OrElse<A, B::Future, F>,
}

enum _OrElse<A, B, F> {
    First(A, F),
    Second(B),
}

impl<A, B, F> Future for OrElse<A, B, F>
where
    A: Future,
    B: IntoFuture<Item = A::Item>,
    F: FnOnce(A::Error) -> B,
{
    type Item = B::Item;
    type Error = B::Error;

    // Returns the option if it contains a value, otherwise calls f and returns the result.
    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        let second = match self.future {
            _OrElse::First(a, f) => match a.poll() {
                Ok(Ok(next)) => return Ok(Ok(next)),
                Ok(Err(e)) => f(e).into_future(),
                Err(a) => {
                    return Err(OrElse {
                        future: _OrElse::First(a, f),
                    })
                }
            },
            _OrElse::Second(b) => b,
        };
        second.poll().map_err(|b| OrElse {
            future: _OrElse::Second(b),
        })
    }
}

impl<T> Future for Receiver<T> {
    type Item = T;
    type Error = RecvError;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        match self.try_recv() {
            Ok(msg) => Ok(Ok(msg)),
            Err(TryRecvError::Empty) => Err(self),
            Err(TryRecvError::Disconnected) => Ok(Err(RecvError)),
        }
    }
}

pub struct Empty<T, E> {
    _marker: marker::PhantomData<(T, E)>,
}

impl<T, E> Empty<T, E> {
    pub fn new() -> Empty<T, E> {
        Empty {
            _marker: marker::PhantomData,
        }
    }
}

impl<T, E> Future for Empty<T, E> {
    type Item = T;
    type Error = E;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        Err(self)
    }
}

impl<T, E> Clone for Empty<T, E> {
    fn clone(&self) -> Empty<T, E> {
        Empty::new()
    }
}

impl<T, E> Copy for Empty<T, E> {}

pub struct Select<A, B> {
    a: A,
    b: B,
}

impl<A, B> Future for Select<A, B>
where
    A: Future,
    B: Future<Item = A::Item, Error = A::Error>,
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        let Select { a, b } = self;
        a.poll().or_else(|a| b.poll().map_err(|b| Select { a, b }))
    }
}

pub struct Join<A, B>
where
    A: Future,
    B: Future<Error = A::Error>,
{
    state: _Join<A, B>,
}

enum _Join<A, B>
where
    A: Future,
    B: Future<Error = A::Error>,
{
    Both(A, B),
    First(A, Result<B::Item, A::Error>),
    Second(Result<A::Item, A::Error>, B),
}

impl<A, B> Future for Join<A, B>
where
    A: Future,
    B: Future<Error = A::Error>,
{
    type Item = (A::Item, B::Item);
    type Error = A::Error;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        let (a, b) = match self.state {
            _Join::Both(a, b) => (a.poll(), b.poll()),
            _Join::First(a, b) => (a.poll(), Ok(b)),
            _Join::Second(a, b) => (Ok(a), b.poll()),
        };
        match (a, b) {
            (Ok(Err(e)), _) | (_, Ok(Err(e))) => Ok(Err(e)),
            (Ok(Ok(a)), Ok(Ok(b))) => Ok(Ok((a, b))),
            (Err(a), Ok(b)) => Err(Join {
                state: _Join::First(a, b),
            }),
            (Ok(a), Err(b)) => Err(Join {
                state: _Join::Second(a, b),
            }),
            (Err(a), Err(b)) => Err(Join {
                state: _Join::Both(a, b),
            }),
        }
    }
}
