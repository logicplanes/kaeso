use kaeso_testkit::services::{self, Kind, TestServices};
use secrecy::ExposeSecret;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mock-provider") => {
            let addr = args.get(1).map_or("127.0.0.1:0", String::as_str);
            let mock = kaeso_testkit::mock_provider::MockProvider::start_on(addr).await;
            println!("{}", mock.base_url);
            std::future::pending::<()>().await;
            std::process::ExitCode::SUCCESS
        }
        Some("mock-idp") => {
            let (Some(addr), Some(id), Some(secret)) = (args.get(1), args.get(2), args.get(3))
            else {
                eprintln!("usage: kaeso-testkit mock-idp ADDR CLIENT_ID CLIENT_SECRET");
                return std::process::ExitCode::from(2);
            };
            let idp = kaeso_testkit::oidc::MockIdp::start_on(addr, id, secret).await;
            println!("{}", idp.issuer());
            std::future::pending::<()>().await;
            std::process::ExitCode::SUCCESS
        }
        Some("fresh-services") => {
            let kind = match args.get(1).map(String::as_str) {
                None => Kind::Development,
                Some("--temporary") => Kind::Test,
                Some(_) => {
                    eprintln!("usage: kaeso-testkit fresh-services [--temporary]");
                    return std::process::ExitCode::from(2);
                }
            };
            let s = TestServices::start_detached(kind).await;
            println!("KAESO_POSTGRES_URL={}", s.pg_url.expose_secret());
            println!("KAESO_CLICKHOUSE_URL={}", s.clickhouse_url.expose_secret());
            println!("KAESO_CLICKHOUSE_DATABASE={}", s.clickhouse_database);
            println!("KAESO_VALKEY_URL={}", s.valkey_url.expose_secret());
            println!("KAESO_VALKEY_KEY_PREFIX={}", s.valkey_prefix);
            std::process::ExitCode::SUCCESS
        }
        Some("drop-services") => {
            let Some(name) = args.get(1) else {
                eprintln!("usage: kaeso-testkit drop-services NAME");
                return std::process::ExitCode::from(2);
            };
            match services::drop_databases(
                &services::postgres_url().into(),
                &services::clickhouse_url().into(),
                name,
            )
            .await
            {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("drop-services failed: {e}");
                    std::process::ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!(
                "usage: kaeso-testkit mock-provider [ADDR] | mock-idp ADDR CLIENT_ID CLIENT_SECRET | fresh-services [--temporary] | drop-services NAME"
            );
            std::process::ExitCode::from(2)
        }
    }
}
