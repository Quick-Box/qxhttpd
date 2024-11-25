use rocket::fairing::AdHoc;
use rocket::response::Debug;
use rocket::{Build, Rocket};
use rocket::serde::json::Json;
use rocket_sync_db_pools::{database, rusqlite};
use rocket_sync_db_pools::rusqlite::params;

#[database("rusqlite")]
struct Db(rusqlite::Connection);

type SqlResult<T, E = Debug<rusqlite::Error>> = std::result::Result<T, E>;

#[get("/list")]
async fn list(db: Db) -> SqlResult<Json<Vec<i64>>> {
    let ids = db.run(|conn| {
        conn.prepare("SELECT id FROM posts")?
            .query_map(params![], |row| row.get(0))?
            .collect::<Result<Vec<i64>, _>>()
    }).await?;

    Ok(Json(ids))
}

async fn init_db(rocket: Rocket<Build>) -> Rocket<Build> {
    Db::get_one(&rocket).await
        .expect("database mounted")
        .run(|conn| {
            let check_table = |table: &str, create_sql: &str| {
                match conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |_| {Ok(())}) {
                    Ok(_) => info!("table {}:{table} exists", conn.path().unwrap_or_default()),
                    Err(_err) => {
                        // info!("SQL error: {}", err);
                        info!("Creating table: {}:{table}", conn.path().unwrap_or_default());
                        let sql = create_sql.to_owned().replace("{table}", table);
                        conn.execute(&sql, params![]).expect("create table");
                    },
                };
            };
            check_table("qechngin",
                        r#"
                        CREATE TABLE {table} (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            recordType VARCHAR NOT NULL,
                            record VARCHAR
                        )"#);
            check_table("occhngin",
                        r#"
                        CREATE TABLE {table} (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            runId INTEGER,
                            siId INTEGER,
                            checkTime VARCHAR
                        )"#);
        }).await;
        // .expect("can init rusqlite DB");

    rocket
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Rusqlite Stage", |rocket| async {
        rocket.attach(Db::fairing())
            .attach(AdHoc::on_ignite("Rusqlite Init", init_db))
            .mount("/sql", routes![list])
    })
}