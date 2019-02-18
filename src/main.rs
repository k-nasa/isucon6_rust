#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;

use chrono::NaiveDateTime;
use rocket::response::content;
use rocket_contrib::templates::Template;
use std::cmp::{max, min};

fn main() {
    rocket::ignite()
        .mount("/", routes![index, initialize])
        .attach(Template::fairing())
        .launch();
}

fn dbh() -> mysql::Pool {
    mysql::Pool::new("mysql://isucon:isucon@localhost:3306/isuda").unwrap()
}

#[get("/initialize")]
fn initialize() -> content::Json<&'static str> {
    init_query().unwrap();
    content::Json("{ 'result': 'ok' }")
}

fn init_query() -> Result<(), mysql::Error> {
    let pool = dbh();
    pool.prep_exec("DELETE FROM entry where id > 7101", ())?;
    Ok(())
}

type EntryTuple = (u32, u32, String, String, NaiveDateTime, NaiveDateTime);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    id: u32,
    user_id: u32,
    keyword: String,
    description: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    html: Option<String>,
    stars: Vec<Star>,
}

impl Entry {
    fn from_tuple(t: EntryTuple) -> Self {
        Entry {
            id: t.0,
            user_id: t.1,
            keyword: t.2,
            description: t.3,
            created_at: t.4,
            updated_at: t.5,
            html: None,
            stars: vec![],
        }
    }
}
}
