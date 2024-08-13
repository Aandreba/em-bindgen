use core::task::{Context, Poll};
use em_bindgen::{console::init_with_level, utils::file_dialog::FileDialog, *};
use futures::{task::noop_waker_ref, FutureExt};
use http::Method;
use log::LevelFilter;
use std::time::Duration;

pub fn main() {
    init_with_level(LevelFilter::Debug).unwrap();

    let mut fut = std::pin::pin!(FileDialog::default().load_file());
    set_finite_main_loop(
        move || {
            let mut cx = Context::from_waker(noop_waker_ref());
            if let Poll::Ready(result) = fut.poll_unpin(&mut cx) {
                log::debug!("Finished");
                println!("{result:?}");
                cancel_main_loop();
            } else {
                log::debug!("Waiting");
            }
        },
        None,
    );

    // assert!(FileDialog::default()
    //     .set_file_name("hello.txt")
    //     .save_file(b"Hello world!"));
}

//* TESTS *//
pub async fn fetch_test() {
    let response =
        em_bindgen::fetch::Builder::new(Method::GET, c"https://pokeapi.co/api/v2/pokemon/ditto")
            .send()
            .await
            .unwrap();

    println!("{response:?}");
}

pub fn test_main_loop() {
    let mut count = 0;
    set_infinite_main_loop(
        move || {
            count += 1;
            println!("Count: {count}");
        },
        Some(em_bindgen::Timing::SetTimeout(Duration::from_millis(500))),
    )
}
