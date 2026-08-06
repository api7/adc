mod cli;
mod config;
mod error;
mod logging;
mod pipeline;
mod progress;

use std::collections::{HashMap, HashSet};

use adc_sdk::{BackendSyncOptions, Event, EventType, ResourceType};
use clap::Parser;

use cli::{BackendArgs, Cli, Command, DiffArgs, DumpArgs, LintArgs, SyncArgs, ValidateArgs};
use error::CliError;

#[tokio::main]
async fn main() {
    // `from_path`, not `dotenv()`: the latter walks up parent directories,
    // which Node's `dotenv` package (what the TS CLI uses) doesn't do.
    let _ = dotenvy::from_path(".env");

    let cli = Cli::parse();
    logging::init(cli.verbose);
    progress::set_verbose(cli.verbose);

    let result = match cli.command {
        Command::Ping(args) => cmd_ping(args).await,
        Command::Dump(args) => cmd_dump(args).await,
        Command::Diff(args) => cmd_diff(args).await,
        Command::Sync(args) => cmd_sync(args).await,
        Command::Lint(args) => cmd_lint(args).await,
        Command::Validate(args) => cmd_validate(args).await,
        Command::Convert => Err(CliError::msg("adc convert: not yet implemented")),
        Command::IngressSync => Err(CliError::msg("adc ingress-sync: not yet implemented")),
        Command::IngressServer => Err(CliError::msg("adc ingress-server: not yet implemented")),
    };

    match result {
        Ok(()) => progress::finish_ok(),
        Err(CliError::AlreadyReported) => std::process::exit(1),
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    }
}

async fn cmd_ping(args: BackendArgs) -> Result<(), CliError> {
    let backend = pipeline::init_backend(&args).await?;
    progress::stage("Connecting to backend...", backend.ping()).await?;
    println!(
        "Connected to the \"{}\" backend successfully!",
        args.backend.as_str()
    );
    Ok(())
}

async fn cmd_dump(args: DumpArgs) -> Result<(), CliError> {
    let backend = pipeline::init_backend(&args.backend).await?;
    let (include, exclude) = pipeline::resource_type_sets(&args.backend);
    let label_selector = pipeline::label_selector_map(&args.backend)?;
    let remote = progress::stage(
        "Fetching remote configuration...",
        pipeline::load_remote(backend.as_ref(), &include, &exclude, &label_selector),
    )
    .await?;

    let mut value = serde_json::to_value(&remote)?;
    if !args.with_id {
        config::strip_ids(&mut value);
    }
    tokio::fs::write(&args.output, serde_yaml_ng::to_string(&value)?).await?;
    println!(
        "Dump backend configuration to {} successfully!",
        args.output.display()
    );
    Ok(())
}

async fn cmd_diff(args: DiffArgs) -> Result<(), CliError> {
    let backend = pipeline::init_backend(&args.backend).await?;
    let (include, exclude) = pipeline::resource_type_sets(&args.backend);
    let label_selector = pipeline::label_selector_map(&args.backend)?;

    let local = progress::stage(
        "Loading local configuration...",
        pipeline::load_local(
            &args.files,
            &include,
            &exclude,
            &label_selector,
            args.backend.managed_by_label,
        ),
    )
    .await?;
    let remote = progress::stage(
        "Fetching remote configuration...",
        pipeline::load_remote(backend.as_ref(), &include, &exclude, &label_selector),
    )
    .await?;
    let events = progress::stage(
        "Computing diff...",
        pipeline::diff(backend.as_ref(), &local, &remote),
    )
    .await?;

    print_diff_summary(&events);
    tokio::fs::write("diff.yaml", serde_yaml_ng::to_string(&events)?).await?;
    println!("Detail diff result has been written to diff.yaml");
    Ok(())
}

async fn cmd_sync(args: SyncArgs) -> Result<(), CliError> {
    let backend = pipeline::init_backend(&args.backend).await?;
    let (include, exclude) = pipeline::resource_type_sets(&args.backend);
    let label_selector = pipeline::label_selector_map(&args.backend)?;

    let local = progress::stage(
        "Loading local configuration...",
        pipeline::load_local(
            &args.files,
            &include,
            &exclude,
            &label_selector,
            args.backend.managed_by_label,
        ),
    )
    .await?;
    let remote = progress::stage(
        "Fetching remote configuration...",
        pipeline::load_remote(backend.as_ref(), &include, &exclude, &label_selector),
    )
    .await?;
    let events = progress::stage(
        "Computing diff...",
        pipeline::diff(backend.as_ref(), &local, &remote),
    )
    .await?;

    let opts = BackendSyncOptions {
        concurrent: Some(args.request_concurrent),
        exit_on_failure: Some(true),
    };

    if progress::interactive() {
        logging::sync_slots::start(events.len() as u64);
    } else if progress::verbose() == 1 {
        logging::sync_report::start(events.len() as u64);
    }
    let sync_result = backend.sync(events, opts).await;
    logging::sync_slots::finish();
    logging::sync_report::finish();
    let results = sync_result.map_err(|err| {
        if progress::verbose() > 0 {
            CliError::AlreadyReported
        } else {
            err.into()
        }
    })?;

    // Successes already had their moment; only failures get a final
    // scroll-back-friendly list here.
    let mut failed = 0;
    for result in &results {
        if !result.success {
            failed += 1;
            match &result.event {
                Some(event) => println!(
                    "[FAILED] {} {}: \"{}\"",
                    event_verb(event),
                    event.resource_type.as_str(),
                    event.resource_name
                ),
                // A backend whose sync granularity is per-server rather
                // than per-event (apisix-standalone) has no single event to
                // blame for the failure — report which server instead.
                None => println!(
                    "[FAILED] sync{}",
                    result.server.as_deref().map(|server| format!(" to {server}")).unwrap_or_default()
                ),
            }
            if let Some(err) = &result.error {
                println!("  {err}");
            }
        }
    }
    // `progress::info`, not `stage`: a fact, not a start/finish task.
    let applied = results.len() - failed;
    let summary = format!("Sync completed: {applied} applied, {failed} failed");
    progress::info(&summary);
    if failed > 0 {
        return Err(CliError::msg("sync completed with failures"));
    }
    Ok(())
}

async fn cmd_lint(args: LintArgs) -> Result<(), CliError> {
    let empty_types: HashSet<ResourceType> = HashSet::new();
    let empty_labels = HashMap::new();
    progress::stage(
        "Linting configuration...",
        pipeline::load_local(
            &args.files,
            &empty_types,
            &empty_types,
            &empty_labels,
            false,
        ),
    )
    .await?;
    println!("Configuration is structurally valid.");
    Ok(())
}

async fn cmd_validate(args: ValidateArgs) -> Result<(), CliError> {
    let backend = pipeline::init_backend(&args.backend).await?;
    let (include, exclude) = pipeline::resource_type_sets(&args.backend);
    let label_selector = pipeline::label_selector_map(&args.backend)?;

    let local = progress::stage(
        "Loading local configuration...",
        pipeline::load_local(
            &args.files,
            &include,
            &exclude,
            &label_selector,
            args.backend.managed_by_label,
        ),
    )
    .await?;
    let remote = progress::stage(
        "Fetching remote configuration...",
        pipeline::load_remote(backend.as_ref(), &include, &exclude, &label_selector),
    )
    .await?;
    let events = progress::stage(
        "Computing diff...",
        pipeline::diff(backend.as_ref(), &local, &remote),
    )
    .await?;

    let result = progress::stage("Validating...", backend.validate(&events)).await?;
    if result.success {
        println!("Configuration is valid.");
        return Ok(());
    }

    for err in &result.errors {
        let name = err.resource_name.as_deref().unwrap_or("<unknown>");
        println!("{}: \"{name}\": {}", err.resource_type, err.error);
    }
    if let Some(message) = &result.error_message {
        println!("{message}");
    }
    Err(CliError::msg("configuration validation failed"))
}

fn print_diff_summary(events: &[Event]) {
    let mut created = 0;
    let mut updated = 0;
    let mut deleted = 0;
    for event in events {
        match event.event_type() {
            EventType::Create => created += 1,
            EventType::Delete => deleted += 1,
            EventType::Update => updated += 1,
            EventType::OnlySubEvents => continue,
        }
        println!(
            "{} {}: \"{}\"",
            event_verb(event),
            event.resource_type.as_str(),
            event.resource_name
        );
    }
    println!(
        "Summary: {created} will be created, {updated} will be updated, {deleted} will be deleted"
    );
}

fn event_verb(event: &Event) -> &'static str {
    match event.event_type() {
        EventType::Create => "create",
        EventType::Delete => "delete",
        EventType::Update => "update",
        _ => "",
    }
}
