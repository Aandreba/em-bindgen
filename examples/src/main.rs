use em_bindgen::{console::init_with_level, set_main_loop};
use http::Method;
use log::LevelFilter;
use std::time::Duration;

pub fn main() {
    init_with_level(LevelFilter::Debug).unwrap();
    log::info!("Hello world!");
    println!("Hi!!");

    // println!("{:?}", FileDialog::default().load_file());
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
    set_main_loop(
        move || {
            count += 1;
            println!("Count: {count}");
        },
        Some(em_bindgen::Timing::SetTimeout(Duration::from_millis(500))),
        true,
    )
}
