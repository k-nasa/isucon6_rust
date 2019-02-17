#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
use rocket::fairing::AdHoc;
use rocket::request::*;
use rocket::response::*;
use rocket_contrib::serve::StaticFiles;
use std::path::{Path, PathBuf};

fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}
