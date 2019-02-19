#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;

use chrono::NaiveDateTime;
use rand::Rng;
use regex::*;
use rocket::http::{Cookie, Cookies, Status};
use rocket::request::Form;
use rocket::response::{content, status::*, Redirect};
use rocket_contrib::json::{Json, JsonValue};
use rocket_contrib::templates::Template;
use sha1::{Digest, Sha1};
use std::cmp::{max, min};
use std::collections::HashMap;

fn main() {
    rocket::ignite()
        .mount(
            "/",
            routes![
                index,
                initialize,
                get_keyword,
                post_keyword,
                delete_keyword,
                get_register,
                post_register,
                get_login,
                post_login,
                post_star,
                get_logout,
            ],
        )
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

type StarTuple = (u32, String, String, NaiveDateTime);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Star {
    id: u32,
    keyword: String,
    user_name: String,
    created_at: NaiveDateTime,
}

impl Star {
    fn from_tuple(t: StarTuple) -> Self {
        Star {
            id: t.0,
            keyword: t.1,
            user_name: t.2,
            created_at: t.3,
        }
    }
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
fn index(page: Option<u32>, session: Cookies) -> Custom<Template> {
    const PER_PAGE: u32 = 10;
    let page = page.unwrap_or(1);

    let pool = dbh();

    let username = username_by_cookie(session);
    if username.is_err() {
        return Custom(Status::Forbidden, Template::render("", ()));
    }

    let username = username.unwrap();

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
    Custom(
        Status::Ok,
        Template::render(
            "index",
            &IndexTemplateContext {
                entries,
                page,
                last_page,
                pages,
                username,
                parent: "layout",
            },
        ),
    )
}

#[derive(Serialize)]
struct KeywordTemplateContext {
    entry: Entry,
    username: String,
    parent: &'static str,
}

#[get("/keyword/<keyword>")]
fn get_keyword(session: Cookies, keyword: String) -> Custom<Template> {
    let username = username_by_cookie(session);

    if username.is_err() {
        return Custom(Status::Forbidden, Template::render("", ()));
    }
    let username = username.unwrap();

    if keyword.is_empty() {
        return Custom(Status::BadRequest, Template::render("", ()));
    }

    let mut entry: Entry = dbh()
        .first_exec("SELECT * FROM entry where keyword = ?", (keyword,))
        .map(|f| Entry::from_tuple(mysql::from_row(f.unwrap())))
        .unwrap();

    entry.html = Some(htmlify(&entry));
    entry.stars = load_stars(&entry);

    Custom(
        Status::Ok,
        Template::render(
            "keyword",
            &KeywordTemplateContext {
                entry,
                username,
                parent: "layout",
            },
        ),
    )
}

#[post("/keyword/<keyword>")]
fn delete_keyword(keyword: String) -> Custom<Redirect> {
    if keyword.is_empty() {
        return Custom(Status::BadRequest, Redirect::to(""));
    }

    let pool = dbh();

    pool.prep_exec("DELETE from entry where keyword = ?", (keyword,))
        .unwrap();
    Custom(Status::Ok, Redirect::to("/"))
}

#[derive(FromForm)]
struct RequestKeyword {
    keyword: String,
    description: String,
}

#[post("/keyword", data = "<keyword>")]
fn post_keyword(keyword: Form<RequestKeyword>, mut session: Cookies) -> Custom<Redirect> {
    let pool = dbh();
    if keyword.keyword.is_empty()
        || keyword.description.is_empty()
        || is_spam_content(&keyword.keyword)
    {
        return Custom(Status::BadRequest, Redirect::to("/"));
    }

    let user_id: String = session
        .get_private("user_id")
        .and_then(|cookie| Some(cookie.value().to_string()))
        .unwrap_or("".into());

    if user_id.parse::<u32>().is_err() {
        return Custom(Status::Forbidden, Redirect::to("/"));
    }

    pool.prep_exec(
        "INSERT INTO entry (author_id, keyword, description, created_at, updated_at) VALUES (?, ?, ?, NOW(), NOW())",
        (user_id, &keyword.keyword, &keyword.description)
    ).unwrap();

    Custom(Status::Found, Redirect::to("/"))
}

#[get("/register")]
fn get_register(session: Cookies) -> Custom<Option<Template>> {
    let username = username_by_cookie(session);

    if username.is_err() {
        return Custom(Status::Forbidden, None);
    }

    let username = username.unwrap();

    let mut context: HashMap<&str, String> = HashMap::new();
    context.insert("username", username);
    context.insert("action", "register".into());

    Custom(
        Status::Ok,
        Some(Template::render("authentication", &context)),
    )
}

#[derive(FromForm)]
struct RequestRegister {
    name: String,
    password: String,
}

#[post("/register", data = "<register>")]
fn post_register(register: Form<RequestRegister>, mut session: Cookies) -> Custom<Redirect> {
    if register.name.is_empty() || register.password.is_empty() {
        return Custom(Status::NotFound, Redirect::to("/"));
    }

    let salt = rand_string(20);
    let pass_digest = format!(
        "{:x}",
        Sha1::digest_str(&(salt.clone() + &register.password))
    );
    let pool = dbh();

    pool.prep_exec(
        "INSERT INTO user (name, salt, password, created_at) VALUES (?, ?, ?, NOW())",
        (&register.name, salt, pass_digest),
    )
    .unwrap();

    let id: u32 = dbh()
        .first_exec("Select id from user where name = ?", (&register.name,))
        .map(|f| mysql::from_row(f.unwrap()))
        .unwrap();

    session.add_private(rocket::http::Cookie::new("user_id", id.to_string()));

    Custom(Status::Found, Redirect::to("/"))
}

#[get("/login")]
fn get_login(session: Cookies) -> Custom<Option<Template>> {
    let username = username_by_cookie(session);

    if username.is_err() {
        return Custom(Status::Forbidden, None);
    }

    let username = username.unwrap();

    let mut context: HashMap<&str, String> = HashMap::new();
    context.insert("username", username);
    context.insert("action", "login".into());

    Custom(
        Status::Ok,
        Some(Template::render("authentication", &context)),
    )
}

#[derive(FromForm)]
struct RequestLogin {
    name: String,
    password: String,
}

#[post("/login", data = "<login>")]
fn post_login(mut session: Cookies, login: Form<RequestLogin>) -> Custom<Redirect> {
    let user: (u32, String, String) = dbh()
        .first_exec(
            "Select id, password, salt from user where name = ?",
            (&login.name,),
        )
        .map(|f| mysql::from_row(f.unwrap()))
        .unwrap();

    let pass_digest = format!("{:x}", Sha1::digest_str(&(user.2 + &login.password)));
    if user.1 == pass_digest {
        session.add_private(Cookie::new("user_id", user.0.to_string()));
    } else {
        return Custom(Status::Forbidden, Redirect::to("/"));
    }

    Custom(Status::Found, Redirect::to("/"))
}

#[get("/logout")]
fn get_logout(mut session: Cookies) -> Redirect {
    session.remove_private(Cookie::named("user_id"));

    Redirect::to("/")
}

#[derive(FromForm, Serialize, Deserialize)]
struct RequestStar {
    keyword: String,
    user: String,
}

#[post("/stars", data = "<star>")]
fn post_star(star: Json<RequestStar>) -> JsonValue {
    let user: (u32, String, String) = dbh()
        .first_exec("Select id from entry where keyword = ?", (&star.keyword,))
        .map(|f| mysql::from_row(f.unwrap()))
        .unwrap();

    dbh()
        .prep_exec(
            "INSERT INTO star (keyword, user_name, created_at) VALUES (?, ?, NOW())",
            (&star.keyword, &star.user),
        )
        .unwrap();

    json!({"result": "ok"})
}

fn htmlify(entry: &Entry) -> String {
    if entry.description.is_empty() {
        return String::new();
    }

    let pool = dbh();
    let rows = pool
        .prep_exec(
            "SELECT keyword FROM entry ORDER BY CHARACTER_LENGTH(keyword) desc",
            (),
        )
        .unwrap();

    let keywords: Vec<String> = rows
        .into_iter()
        .map(|f| mysql::from_row(f.unwrap()))
        .collect();

    let keyword_re: String = keywords.join("|");
    let keyword_re = format!("/({})", keyword_re);

    let mut kw2sha = HashMap::new();
    let re = Regex::new(&keyword_re).unwrap();
    let result = re.replace_all(&entry.description, |caps: &Captures| {
        let kw = caps[0].to_string();
        let digest = format!("isuda_{:x}", Sha1::digest_str(&kw));
        kw2sha.insert(kw.clone(), digest);
        String::from(kw)
    });

    // FIXME resultを本当はurlencodingするほうが良い

    let mut content = result.replace("", "");
    for (kw, hash) in kw2sha {
        let url = format!("/keyword/{}", kw);
        let link = format!("<a href=\"{}\">{}</a>", url, kw);
        content = content.replace(&hash, &link);
    }

    content
}

fn load_stars(entry: &Entry) -> Vec<Star> {
    let keyword = &entry.keyword;

    let pool = dbh();
    let rows = pool
        .prep_exec("SELECT * FROM star where keyword = ?", (&keyword,))
        .unwrap();

    let stars: Vec<Star> = rows
        .into_iter()
        .map(|f| mysql::from_row(f.unwrap()))
        .map(|f| Star::from_tuple(f))
        .collect();

    stars
}

fn rand_string(l: u32) -> String {
    let mut rng = rand::thread_rng();

    (0..l)
        .map(|_| rng.gen_range(b'a', b'z' + 1) as char)
        .collect()
}

fn username_by_cookie(mut c: Cookies) -> Result<String, ()> {
    let user_id: String = c
        .get_private("user_id")
        .and_then(|cookie| Some(cookie.value().to_string()))
        .unwrap_or("".into());

    if user_id.is_empty() {
        return Ok(String::new());
    }

    let row = dbh()
        .first_exec("Select name from user where id = ?", (user_id,))
        .unwrap();

    if row.is_none() {
        return Err(());
    }

    let username: String = row.map_or(String::new(), |r| mysql::from_row(r));

    Ok(username)
}

fn is_spam_content(content: &str) -> bool {
    let mut map = HashMap::new();
    map.insert("content", content);

    let client = reqwest::Client::new();
    let mut res = client
        .post("http://localhost:5050")
        .json(&map)
        .send()
        .unwrap();

    let json: JsonValue = res.json().unwrap();
    json.get("valid").is_some()
}
