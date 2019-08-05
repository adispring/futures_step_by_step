extern crate futures;

use futures::*;

fn is_future_v<A, B, C: Future<Item = A, Error = B>>(_: C) {}

fn get<F: Future>(f: F) -> Result<F::Item, F::Error> {
    f.poll().ok().unwrap()
}

#[test]
fn result_smoke() {
    let f = Ok(1).into_future();
    println!("{:#?}", f);

    is_future_v::<i32, u32, _>(f);
    is_future_v::<i32, u32, _>(f.map(|a| a + 1));
    is_future_v::<i32, u32, _>(f.and_then(|a| Ok(a)));
    is_future_v(f.or_else(|a| Err(a)));
    is_future_v(f.select(Err(3)));
    is_future_v::<(i32, i32), u32, _>(f.join(Err(3)));
}
