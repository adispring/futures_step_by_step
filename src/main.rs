extern crate futures;

use futures::*;

fn main() {
    let f: FutureResult<i32, u32> = Ok(1).into_future();
    println!("{:#?}", f);
    // let g = f.map(|a| a + 1);
    // println!("{:#?}", g);
    let empty: Empty<i32, i32> = Empty::new();
    let empty_poll = empty.poll();
    println!("{:#?}", empty);
    println!("{:#?}", empty_poll);
}
