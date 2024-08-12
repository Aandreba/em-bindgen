use em_bindgen::{set_main_loop, utils::file_dialog::FileDialog};
use http::Method;
use std::time::Duration;

pub fn main() {
    println!("{:?}", FileDialog::default().load_file());
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
