#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate mysql;

use rocket::fairing::AdHoc;
// use rocket::request::;
use rocket::response::content;
use rocket_contrib::serve::StaticFiles;
use std::path::{Path, PathBuf};

fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}

fn dbh() -> mysql::Pool {
    mysql::Pool::new("mysql://root:password@localhost:3307/isuda").unwrap()
}

#[get("/initialize")]
fn initialize() -> content::Json<&'static str> {
    init_query().unwrap();
    content::Json("{ 'result': 'ok' }")
}

fn init_query() -> Result<(), mysql::Error> {
    let pool = dbh();
    pool.prep_exec("DELETE FROM entry where id > 7101", ())?;
    pool.prep_exec("TRUNCATE star", ())?;
    Ok(())
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}
