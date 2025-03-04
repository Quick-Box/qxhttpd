use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Build, Rocket};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::migrate::Migrator;
use std::path::Path;
use std::str::FromStr;

#[macro_export]
macro_rules! impl_sqlx_text_type_and_decode {
    ($type:ident) => {
        impl<DB: sqlx::Database> sqlx::Type<DB> for $type
        where str: sqlx::Type<DB>
        {
            fn type_info() -> <DB as sqlx::Database>::TypeInfo {
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
#[macro_export]
macro_rules! impl_sqlx_json_text_type_and_decode {
    ($type:ident) => {
        impl<DB: sqlx::Database> sqlx::Type<DB> for $type
        where str: sqlx::Type<DB>
        {
            fn type_info() -> <DB as sqlx::Database>::TypeInfo {
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
        // let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let figment = rocket.figment();
        let database_url = figment.extract_inner::<String>("database_url").expect("database_url");

        // Ensure database file exists
        if database_url.starts_with("sqlite://") {
           let db_path = database_url.trim_start_matches("sqlite://");
           if !Path::new(db_path).exists() {
               // info!("creating database: {database_url}");
               std::fs::File::create(db_path).expect("Failed to create SQLite database file");
           }
        }
        info!("Opening database: {database_url}");
        // Initialize connection pool
        let opts = SqliteConnectOptions::from_str(&database_url).expect("valid sqlite url")
            //.journal_mode(SqliteJournalMode::Wal) // use WAL for better concurrency
            //.pragma("foreign_keys", "true") // enable foreign keys
            ;
        let pool = match SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
        {
            Ok(pool) => pool,
            Err(err) => {
                error!("Database connection error: {:?}", err);
                return Err(rocket);
            }
        };

        // Run migrations
        // TODO: bring migration back
        // match MIGRATOR.run(&pool).await {
        //     Ok(_) => info!("Migrations applied successfully!"),
        //     Err(err) => {
        //         error!("Migration error: {:?}", err);
        //         return Err(rocket);
        //     }
        // };

        Ok(rocket.manage(DbPool(pool)))
    }
}
