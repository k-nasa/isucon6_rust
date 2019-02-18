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

type StarTuple = (u32, String, String, String);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Star {
    id: u32,
    keyword: String,
    user_name: String,
    created_at: NaiveDateTime,
}

#[derive(Serialize)]
struct IndexTemplateContext {
    entries: Vec<Entry>,
    page: u32,
    last_page: u32,
    pages: Vec<u32>,
    username: String,
    parent: &'static str,
}

#[get("/?<page>")]
fn index(page: Option<u32>, session: rocket::http::Cookies) -> Template {
    const PER_PAGE: u32 = 10;
    let page = page.unwrap_or(1);

    let pool = dbh();

    let username = username_by_cookie(session);

    let rows = pool
        .prep_exec(
            "SELECT * FROM entry ORDER BY updated_at desc limit ? offset ?",
            (PER_PAGE, PER_PAGE * (page - 1)),
        )
        .unwrap();

    let mut rows: Vec<Entry> = rows
        .into_iter()
        .map(|f| mysql::from_row(f.unwrap()))
        .map(|f| Entry::from_tuple(f))
        .collect();

    for row in &mut rows {
        row.html = Some(htmlify(&row));
        row.stars = load_stars(&row);
    }
    let entries: Vec<Entry> = rows;

    let total_entries: u32 = pool
        .first_exec("SELECT COUNT(1) AS count FROM entry", ())
        .map(|f| mysql::from_row(f.unwrap()))
        .unwrap();

    let last_page: u32 = (total_entries as f64 / PER_PAGE as f64).ceil() as u32;
    let pages = ((max(1, page as i32 - 5_i32) as u32)..(min(last_page, page + 5) - 1)).collect();
    Template::render(
        "index",
        &IndexTemplateContext {
            entries,
            page,
            last_page,
            pages,
            parent: "layout",
        },
    )
}

fn htmlify(entry: &Entry) -> String {
    "heiojweiofjowefjiwofjoewjwiofoejoijefiowqjfiowrngov".into()
}

fn load_stars(entry: &Entry) -> Vec<Star> {
    vec![]
}

fn username_by_cookie(c: rocket::http::Cookies) -> String {
    let user_id: &str = match c.get("user_id") {
        Some(c) => c.value_raw().unwrap(),
        None => "",
    };

    let username: String = dbh()
        .first_exec("Select name from user where id = ?", (user_id,))
        .unwrap()
        .map_or(String::new(), |r| mysql::from_row(r));

    username
}
