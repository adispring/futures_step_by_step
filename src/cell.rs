use std::cell::UnsafeCell;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering::{self, Acquire, Release, SeqCst};
use std::sync::atomic::{AtomicBool, AtomicUsize};

pub struct AtomicCell<T> {
    in_use: AtomicBool,
    data: UnsafeCell<T>,
}
