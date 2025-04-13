use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Build, Rocket, State};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::migrate::Migrator;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::{anyhow};
use crate::event::EventId;
use crate::{OpenEvent, SharedQxState};

// pub fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<Value> {
//     let mut map = Map::new();
//     for column in row.columns() {
//         let name = column.name();
//         let value: Value = match column.type_info().name() {
//             "INTEGER" => row.try_get::<i64, _>(name).map(Value::from)?,
//             "TEXT" => row.try_get::<String, _>(name).map(Value::from)?,
//             "REAL" => row.try_get::<f64, _>(name).map(Value::from)?,
//             "BLOB" => return Err(anyhow!("Sqlite BLOB to JSON NIY")),
//             t => return Err(anyhow!("Sqlite type {t} to JSON NIY")),
//         };
//         map.insert(name.to_string(), value);
//     }
//     Ok(Value::Object(map))
// }

// macro to decode some type from SQL text
#[macro_export]
macro_rules! impl_sqlx_text_type_encode_decode {
    ($type:ident) => {
        impl<DB: sqlx::Database> sqlx::Type<DB> for $type
        where str: sqlx::Type<DB>
        {
            fn type_info() -> <DB as sqlx::Database>::TypeInfo {
                // TEXT columns only
                <&str as sqlx::Type<DB>>::type_info()
            }
        }

        impl<'r, DB: sqlx::Database> sqlx::Decode<'r, DB> for $type
        where &'r str: sqlx::Decode<'r, DB>
        {
            fn decode(value: <DB as sqlx::Database>::ValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
                let value = <&str as sqlx::Decode<DB>>::decode(value)?;
                Ok(Self::from_string(value.to_string()))
            }
        }

        impl<'r> Encode<'r, Sqlite> for $type {
            fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'r>>) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                let s = format!("{}", self);
                <String as Encode<Sqlite>>::encode(s, buf)
            }
        }
    };
}

// macro to decode some type from SQL JSON text
#[macro_export]
macro_rules! impl_sqlx_json_text_type_encode_decode {
    ($type:ident) => {
        impl<DB: sqlx::Database> sqlx::Type<DB> for $type
        where str: sqlx::Type<DB>
        {
            fn type_info() -> <DB as sqlx::Database>::TypeInfo {
                // TEXT columns only
                <&str as sqlx::Type<DB>>::type_info()
            }
        }

        impl<'r, DB: sqlx::Database> sqlx::Decode<'r, DB> for $type
        where &'r str: sqlx::Decode<'r, DB>
        {
            fn decode(value: <DB as sqlx::Database>::ValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
                let value = <&str as sqlx::Decode<DB>>::decode(value)?;
                Ok(serde_json::from_str::<$type>(value)?)
            }
        }
        
        impl<'r> Encode<'r, Sqlite> for $type {
            fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'r>>) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                let s = serde_json::to_string(self).expect("Shall be serializable");
                <String as Encode<Sqlite>>::encode(s, buf)
            }
        }
    };
}

static MIGRATOR: Migrator = sqlx::migrate!("db/migrations"); // Auto-discovers migrations in `migrations/`
static EDB_MIGRATOR: Migrator = sqlx::migrate!("db/edb/migrations"); 

pub struct DbPool(pub SqlitePool);

pub struct DbPoolFairing();

const EVENTS_DB: &str = "qxdb";
#[rocket::async_trait]
impl Fairing for DbPoolFairing {
    fn info(&self) -> Info {
        Info {
            name: "SQLite Database Pool with Migrations",
            kind: Kind::Ignite | Kind::Liftoff,
        }
    }

    async fn on_ignite(&self, rocket: Rocket<Build>) -> rocket::fairing::Result {

        let figment = rocket.figment();
        let db_path = figment.extract_inner::<String>("db_path").expect("db_path");
        let pool= match open_db(&db_path, EVENTS_DB).await {
            Ok(p) => p,
            Err(err) => {
                error!("Migration error: {:?}", err);
                return Err(rocket);
            }
        };

        // Run migrations
        //#[cfg(any(not(debug_assertions), test))]
        match MIGRATOR.run(&pool).await {
            Ok(_) => info!("Migrations applied successfully!"),
            Err(err) => {
                error!("Migration error: {:?}", err);
                return Err(rocket);
            }
        };

        Ok(rocket.manage(DbPool(pool)))
    }
}

pub async fn get_event_db(event_id: EventId, state: &State<SharedQxState>) -> anyhow::Result<SqlitePool> {
    let schema_name = event_id_to_schema_name(event_id);
    let db_path = state.read().await.app_config.db_path.clone();
    if let Some(ev) = state.read().await.open_events.get(&event_id) {
        ev.hit_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return Ok(ev.db.clone());
    }
    let pool = open_db(&db_path, &schema_name).await?;

    match EDB_MIGRATOR.run(&pool).await {
        Ok(_) => info!("Event DB {schema_name} migrations applied successfully!"),
        Err(err) => {
            error!("Event DB {schema_name} migration error: {:?}", err);
        }
    };
    
    let oe = OpenEvent { hit_count: Arc::new(Default::default()), db: pool.clone() };
    state.write().await.open_events.insert(event_id, oe);
    Ok(pool)
}

async fn open_db(db_path: &str, schema_name: &str) -> anyhow::Result<SqlitePool> {
    let database_url = if cfg!(test) {
        "sqlite::memory:".to_string()
        // let db_path = format!("/tmp/{}.sqlite", schema_name);
        // let _ = std::fs::remove_file(&db_path);
        // std::fs::File::create(&db_path).map_err(|e | anyhow!("Failed to create SQLite database file {db_path} error: {e}"))?;
        // format!("sqlite://{db_path}")
    } else {
        let db_path = format!("{db_path}/{schema_name}.sqlite");
        if !Path::new(&db_path).exists() {
            // info!("creating database: {database_url}");
            std::fs::File::create(&db_path).map_err(|e | anyhow!("Failed to create SQLite database file {db_path} error: {e}"))?;
        }
        format!("sqlite://{db_path}")
    };
    let opts = SqliteConnectOptions::from_str(&database_url).map_err(|e| anyhow!("Open db error: {e}"))?
        // .journal_mode(SqliteJournalMode::Wal) // use WAL for better concurrency
        //.pragma("foreign_keys", "true") // enable foreign keys
        ;
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await.map_err(|e| anyhow!(e.to_string()))?;
    Ok(pool)
}

fn event_id_to_schema_name(event_id: EventId) -> String {
    format!("ev{event_id:0>4}")
}

#[test]
fn test_event_id_to_schema_name() {
    assert_eq!(&event_id_to_schema_name(1), "ev0001");
}
