extern crate futures;

use futures::*;

fn main() {
    let f: FutureResult<i32, u32> = Ok(1).into_future();
    println!("{:#?}", f);
    // let g = f.map(|a| a + 1);
    // println!("{:#?}", g);
}
