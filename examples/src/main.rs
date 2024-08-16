use em_bindgen::{console::init_with_level, future::block_on, *};
use log::LevelFilter;
use std::time::Duration;

pub fn main() {
    init_with_level(LevelFilter::Debug).unwrap();

    block_on(async move {
        let (parts, body) =
            fetch::get(c"http://httpbin.org/drip?duration=2&numbytes=10&code=200&delay=2")
                .await
                .unwrap()
                .into_parts();
        println!("{parts:#?}");

        let mut chunks = body.reader();
        while let chunk @ [_, ..] = chunks.fill_buf().await.unwrap() {
            println!("{chunk:?}");
            let len = chunk.len();
            chunks.consume(len);
        }

        println!("Done!");
    });

    // assert!(FileDialog::default()
    //     .set_file_name("hello.txt")
    //     .save_file(b"Hello world!"));
}

//* TESTS *//
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
