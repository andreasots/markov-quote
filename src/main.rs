#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate dotenv;
extern crate chrono;
extern crate markov;
extern crate rand;
extern crate serde;
extern crate diesel_full_text_search;

#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
#[macro_use] extern crate error_chain;
#[macro_use] extern crate serde_derive;

mod errors;
mod schema;
mod models;

type ConnectionPool = r2d2::Pool<r2d2_diesel::ConnectionManager<diesel::pg::PgConnection>>;

#[derive(Debug, Serialize)]
struct IndexTemplateContext {
    page: String,
    quotes: Vec<models::Quote>,
    form: FilterForm,
}

#[derive(Debug, FromForm, Serialize)]
pub struct FilterForm {
    mode: Option<String>,
    q: Option<String>,
}

use diesel::query_builder::QueryBuilder;
infix_predicate!(Ilike, " ILIKE ");

fn index_impl(pool: rocket::State<ConnectionPool>, mut form: FilterForm) -> Result<rocket_contrib::Template, errors::Error> {
    use diesel::prelude::*;
    use schema::quotes::dsl;
    use rand::Rng;
    use rand::distributions::{Weighted, WeightedChoice, IndependentSample};
    use diesel_full_text_search::{to_tsvector, plainto_tsquery, TsVectorExtensions};
    use diesel::expression::AsExpression;

    let conn = pool.get()?;

    if let Some(ref mut mode) = form.mode {
        *mode = mode.trim().into();
    }
    if let Some(ref mut query) = form.q {
        *query = query.trim().into();
    }

    let quotes = match (form.mode.as_ref().map(|s| &s[..]), form.q.as_ref()) {
        (Some("text"), Some(query)) if query.len() > 0 => {
            dsl::quotes
                .filter(dsl::deleted.eq(false))
                .filter(to_tsvector(dsl::quote).matches(plainto_tsquery(query)))
                .load::<models::Quote>(&*conn)?
        },
        (Some("name"), Some(query)) => {
            let mut query = query.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            query.insert(0, '%');
            query.push('%');

            dsl::quotes
                .filter(dsl::deleted.eq(false))
                .filter(Ilike::new(dsl::attrib_name, <String as AsExpression<diesel::types::Text>>::as_expression(query)))
                .load::<models::Quote>(&*conn)?
        },
        _ => {
            dsl::quotes
                .filter(dsl::deleted.eq(false))
                .load::<models::Quote>(&*conn)?
        }
    };

    let quotes = if quotes.len() > 0 {
        let mut quote_chain = markov::Chain::new();
        let mut attrib_chain = markov::Chain::new();
        let mut context_chain = markov::Chain::new();

        let mut attrib_count = 0;
        let mut context_count = 0;

        for quote in &quotes {
            quote_chain.feed_str(&quote.quote);
            if let Some(ref name) = quote.attrib_name {
                attrib_chain.feed_str(name);
                attrib_count += 1;
            }
            if let Some(ref context) = quote.context {
                context_chain.feed_str(context);
                context_count += 1;
            }
        }

        context_count = std::cmp::min(context_count * 7, quotes.len() as u32);

        let mut rng = rand::thread_rng();
        
        let mut name_distribution = vec![
            Weighted { weight: attrib_count, item: true },
            Weighted { weight: quotes.len() as u32 - attrib_count, item: false }
        ];
        let name_distribution = WeightedChoice::new(&mut name_distribution);

        let mut context_distribution = vec![
            Weighted { weight: context_count, item: true },
            Weighted { weight: quotes.len() as u32 - context_count, item: false },
        ];
        let context_distribution = WeightedChoice::new(&mut context_distribution);

        let mut quotes = vec![];
        for _ in 0..25 {
            quotes.push(models::Quote {
                id: rng.gen_range(5000, 10000),
                quote: quote_chain.generate_str(),
                attrib_name: if name_distribution.ind_sample(&mut rng) { Some(attrib_chain.generate_str()) } else { None },
                attrib_date: None, // TODO: generate random date
                deleted: false,
                context: if context_distribution.ind_sample(&mut rng) { Some(context_chain.generate_str()) } else { None },
                game_id: None,
                show_id: None,
            });
        }

        quotes
    } else {
        vec![]
    };

    Ok(rocket_contrib::Template::render("index", &IndexTemplateContext {
        page: String::from("quotes"),
        quotes: quotes,
        form: form,
    }))
}

#[get("/?<form>")]
fn index(pool: rocket::State<ConnectionPool>, form: FilterForm) -> Result<rocket_contrib::Template, errors::Error> {
    index_impl(pool, form)
}

#[get("/", rank = 2)]
fn index_bare(pool: rocket::State<ConnectionPool>) -> Result<rocket_contrib::Template, errors::Error> {
    index_impl(pool, FilterForm {
        mode: None,
        q: None,
    })
}

#[get("/static/<file..>")]
fn static_file(file: std::path::PathBuf) -> Result<rocket::response::NamedFile, errors::Error> {
    Ok(rocket::response::NamedFile::open(std::path::Path::new("static/").join(file))?)
}

fn main() {
    dotenv::dotenv().expect("failed to read '.env'");
    let config = r2d2::Config::default();
    let manager = r2d2_diesel::ConnectionManager::<diesel::pg::PgConnection>::new(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"));
    rocket::ignite()
        .mount("/", routes![index, index_bare, static_file])
        .manage(r2d2::Pool::new(config, manager).expect("failed to initialise connection pool"))
        .launch();
}
