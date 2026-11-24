//! `ath-host serve`, `setup`, `invite`, and `version`.

use std::path::PathBuf;
use std::time::Duration;

fn usage() -> ! {
    eprintln!(
        "usage:
  ath-host serve --config <file>
  ath-host setup --config <file> --issuer <issuer> --subject <subject> [--name <display name>]
  ath-host invite --config <file> [--days <n>]
  ath-host backup --config <file> --output <folder>
  ath-host restore --config <file> --athlete <athlete id> --package <zip>
  ath-host version"
    );
    std::process::exit(2)
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|arg| arg == name).and_then(|index| args.get(index + 1)).cloned()
}

fn config(args: &[String]) -> ath_host::config::Config {
    let path = PathBuf::from(option(args, "--config").unwrap_or_else(|| usage()));
    ath_host::config::Config::read(&path).unwrap_or_else(|error| fail(&error))
}

fn fail(message: &str) -> ! {
    eprintln!("ath-host: {message}");
    std::process::exit(1)
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("version") | Some("--version") => println!("ath-host {}", ath_host::version()),
        Some("serve") => serve(config(&args)).await,
        Some("setup") => setup(config(&args), &args).await,
        Some("invite") => invite(config(&args), &args).await,
        Some("backup") => backup(config(&args), &args).await,
        Some("restore") => restore(config(&args), &args).await,
        _ => usage(),
    }
}

async fn serve(config: ath_host::config::Config) {
    std::fs::create_dir_all(&config.data_directory).unwrap_or_else(|error| fail(&error.to_string()));
    // One host process owns a data folder: in directory mode it holds the records.
    let lock = std::fs::File::create(config.data_directory.join(".ath-host.lock"))
        .unwrap_or_else(|error| fail(&error.to_string()));
    if lock.try_lock().is_err() {
        fail("another ath-host is using this data folder");
    }
    let listen = config.listen.clone();
    let state = ath_host::state(config).await.unwrap_or_else(|error| fail(&error));
    let finished = ath_host::backup::finish_erasures(&state).await.unwrap_or_else(|error| fail(&error.to_string()));
    if finished > 0 {
        eprintln!("ath-host: finished {finished} erasure(s) that an earlier run did not complete");
    }
    let sweeper = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            sweeper.storage.sweep_idle();
        }
    });
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .unwrap_or_else(|error| fail(&format!("cannot listen on {listen}: {error}")));
    eprintln!("ath-host {} listening on {listen}", ath_host::version());
    axum::serve(listener, ath_host::api::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .unwrap_or_else(|error| fail(&error.to_string()));
    drop(lock);
}

/// Claim the first owner, once, on the machine (D260): create their record.
async fn setup(config: ath_host::config::Config, args: &[String]) {
    let issuer = option(args, "--issuer").unwrap_or_else(|| usage());
    let subject = option(args, "--subject").unwrap_or_else(|| usage());
    let state = ath_host::state(config).await.unwrap_or_else(|error| fail(&error));
    let claimed = state
        .directory
        .set_setting_once("first_owner", &format!("{issuer}\n{subject}"))
        .await
        .unwrap_or_else(|error| fail(&error.to_string()));
    if !claimed {
        fail("this host already has its first owner; use ath-host invite for more athletes");
    }
    let principal = state
        .directory
        .principal(&issuer, &subject, option(args, "--name").as_deref())
        .await
        .unwrap_or_else(|error| fail(&error.to_string()));
    let athlete = state
        .directory
        .create_athlete(&principal.principal_id, option(args, "--name").as_deref())
        .await
        .unwrap_or_else(|error| fail(&error.to_string()));
    state.storage.create(&athlete.resource_id).await.unwrap_or_else(|error| fail(&error.to_string()));
    println!("{}", athlete.resource_id);
}

/// An invitation code that lets one person create their own record.
async fn invite(config: ath_host::config::Config, args: &[String]) {
    let days: i64 = option(args, "--days").map(|days| days.parse().unwrap_or_else(|_| usage())).unwrap_or(7);
    let state = ath_host::state(config).await.unwrap_or_else(|error| fail(&error));
    let offer = state
        .directory
        .create_athlete_invitation("operator", days.clamp(1, 90) * 24 * 3600)
        .await
        .unwrap_or_else(|error| fail(&error.to_string()));
    println!("{}\nexpires {}", offer.code, offer.expires_at);
}

/// Back up every athlete's complete package, each checked by a trial restore.
async fn backup(config: ath_host::config::Config, args: &[String]) {
    let output = PathBuf::from(option(args, "--output").unwrap_or_else(|| usage()));
    let state = ath_host::state(config).await.unwrap_or_else(|error| fail(&error));
    let done = ath_host::backup::backup_all(&state, &output).await.unwrap_or_else(|error| fail(&error.to_string()));
    for entry in done {
        println!("{}\t{}\t{}", entry.athlete_id, entry.manifest_digest, entry.file.display());
    }
}

/// Restore one athlete from a backup, then reapply later erasures.
async fn restore(config: ath_host::config::Config, args: &[String]) {
    let athlete = option(args, "--athlete").unwrap_or_else(|| usage());
    let package = PathBuf::from(option(args, "--package").unwrap_or_else(|| usage()));
    let bytes =
        std::fs::read(&package).unwrap_or_else(|error| fail(&format!("cannot read {}: {error}", package.display())));
    let state = ath_host::state(config).await.unwrap_or_else(|error| fail(&error));
    let revision = ath_host::backup::restore_athlete(&state, &athlete, &bytes)
        .await
        .unwrap_or_else(|error| fail(&error.to_string()));
    println!("restored {athlete} at database revision {revision}");
}
