use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Build, Rocket, State};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::migrate::Migrator;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::anyhow;
use crate::event::EventId;
use crate::{OpenEvent, SharedQxState};

// macro to decode some type from SQL text
#[macro_export]
macro_rules! impl_sqlx_text_type_and_decode {
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
                Ok(Self(value.to_string()))
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

pub struct DbPool(pub SqlitePool);

pub struct DbPoolFairing();
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
        let db_path = figment.extract_inner::<String>("database_path").expect("database_path");
        let pool= match open_db(&db_path, "qxdb").await {
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
    let db_path = state.read().map_err(|e| anyhow!(e.to_string()))?.app_config.db_path.clone();
    if let Some(ev) = state.read().map_err(|e| anyhow!(e.to_string()))?.open_events.get(&event_id) {
        ev.hit_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return Ok(ev.db.clone());
    }
    let pool = open_db(&db_path, &schema_name).await?;
    let oe = OpenEvent { hit_count: Arc::new(Default::default()), db: pool.clone() };
    state.write().map_err(|e| anyhow!(e.to_string()))?.open_events.insert(event_id, oe);
    Ok(pool)
}

async fn open_db(db_path: &str, schema_name: &str) -> anyhow::Result<SqlitePool> {
    let database_url = if cfg!(test) {
        "sqlite::memory:".to_string()
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
