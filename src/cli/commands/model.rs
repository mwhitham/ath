//! The commands that use the optional runtime services: `key`, `key set`,
//! `key clear`, `models`, `predict`, `backtest`, and `share`.
//!
//! A password store or gateway client is built only here, and only when the
//! command needs a key. `predict --json` reads evidence offline and builds
//! neither.

use std::path::Path;
use std::rc::Rc;

use super::shared::{confirm_flags, proceed, required_argument, with_store, CommandEnvironment, Open};
use crate::cli::parse::Args;
use crate::cli::runtime::{refuse, CommandResult, Confirm, Context, Failure, DEFAULT_FILENAME};
use crate::error::Error;
use crate::json;
use crate::model::time;
use crate::prediction::backtest::{
    render_backtest, run_backtest, write_backtest_report, BacktestOptions, BacktestOutput,
};
use crate::prediction::predict::{request_prediction, write_prediction};
use crate::prediction::prediction_evidence;
use crate::runtime::gateway::GatewayClient;
use crate::runtime::keyring::{
    describe_cleared, describe_key_status, no_terminal_message, parse_gateway, Gateway, KeyAccount, Keyring,
    SecretStore,
};
use crate::runtime::model_client::ModelError;
use crate::runtime::models::saved_default_message;
use crate::runtime::models::{load_catalog, render_model_list, require_on_list, resolve_model, save_default_model};
use crate::runtime::share::{share_refusal, share_status};

/// A model or key refusal, printed as `ath: <message>`.
fn model_failure(error: ModelError) -> Failure {
    Failure::from(Error::from(error))
}

/// `--gateway <name>`, when given.
fn requested_gateway(args: &Args) -> Result<Option<Gateway>, Failure> {
    args.value("gateway").map(parse_gateway).transpose().map_err(Failure::from)
}

/// The password store and environment a command reads keys from.
struct Keys {
    store: Rc<dyn SecretStore>,
    env: std::collections::BTreeMap<String, String>,
}

impl Keys {
    fn of(context: &mut Context<'_>) -> Keys {
        Keys { store: context.secret_store(), env: context.io.env.clone() }
    }

    fn with<T>(&self, body: impl FnOnce(&Keyring<'_>) -> T) -> T {
        let env = CommandEnvironment(&self.env);
        body(&Keyring::new(&*self.store, &env))
    }
}

/// Connect to the requested gateway, or to the one gateway that has a key.
fn connect(context: &mut Context<'_>, requested: Option<Gateway>) -> Result<GatewayClient, Failure> {
    let endpoints = context.gateway_endpoints()?;
    let keys = Keys::of(context);
    keys.with(|keyring| GatewayClient::connect(keyring, requested, &endpoints)).map_err(model_failure)
}

/// `ath key`: which gateways have a saved or environment key.
pub fn key_status(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let status = Keys::of(context).with(|keyring| keyring.status());
    if args.flag("json") {
        return context.say_json(&json::to_value(&status));
    }
    context.say(&describe_key_status(&status.saved, &status.environment))
}

/// `ath key set <gateway>`: hidden input, saved in the password store.
pub fn key_set(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let account = KeyAccount::for_key_set(required_argument(args, "gateway"))?;
    if !context.io.terminal.stdin {
        return Err(refuse(no_terminal_message()));
    }
    let secret = (context.services.prompt_secret)(&account.prompt()).map_err(Error::from)?;
    Keys::of(context).with(|keyring| keyring.set(account, &secret)).map_err(Error::from)?;
    context.say(&account.saved_message())
}

/// `ath key clear [gateway]`.
pub fn key_clear(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let gateway = args.argument("gateway").map(parse_gateway).transpose()?;
    let cleared = Keys::of(context).with(|keyring| keyring.clear(gateway));
    context.say(&describe_cleared(&cleared))
}

/// `ath models`: the live list, or `--default <name>` saved beside the
/// database.
pub fn models(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let requested = requested_gateway(args)?;
    let client = connect(context, requested)?;
    let catalog = load_catalog(&client).map_err(model_failure)?;
    let json = args.flag("json");
    if let Some(name) = args.value("default").filter(|name| !name.is_empty()) {
        let path = context.find_database(args.value("file"))?;
        let saved = save_default_model(&path, name, &catalog)?;
        if json {
            return context.say_json(&json::to_value(&saved));
        }
        return context.say(&saved_default_message(&saved));
    }
    if json {
        return context.say_json(&json::to_value(&catalog));
    }
    context.say(&render_model_list(&catalog.models, catalog.gateway))
}

/// `ath predict <benchmark>`: evidence with `--json`; otherwise a model call,
/// a preview, and a confirmed write.
pub fn predict(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let benchmark = required_argument(args, "benchmark").to_string();
    let flags = confirm_flags(args);
    with_store(context, args, Open::Write, |context, store, path| {
        let evidence = prediction_evidence(store, &benchmark, args.value("as_of"), args.value("timezone"))?;
        if flags.json {
            return context.say_json(&json::to_value(&evidence));
        }
        let requested = requested_gateway(args)?;
        let client = connect(context, requested)?;
        let resolved = resolve_model(&client, path, args.value("model"))?;
        let preview = request_prediction(&evidence, &client, &resolved.model, &time::now_utc())?;
        context.say_json(&json::to_value(&preview))?;
        if !proceed(context, flags, "Save this prediction?")? {
            return Ok(());
        }
        let written = write_prediction(store, &preview)?;
        context.say(&format!(
            "saved prediction {} at database revision {}",
            written.record.record_id, written.database_revision
        ))
    })
}

/// `ath backtest`: replay history with one or more models. The report is a
/// separate file; athlete data is never written.
pub fn backtest(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let json = args.flag("json");
    with_store(context, args, Open::Write, |context, store, path| {
        let requested = requested_gateway(args)?;
        let client = connect(context, requested)?;
        let named = args.values("model");
        let all = args.flag("all");
        let models: Vec<String> = if all {
            load_catalog(&client).map_err(model_failure)?.models.into_iter().map(|model| model.id).collect()
        } else if !named.is_empty() {
            let catalog = load_catalog(&client).map_err(model_failure)?;
            named
                .iter()
                .map(|model| require_on_list(model, &catalog.models))
                .collect::<Result<_, _>>()
                .map_err(model_failure)?
        } else {
            vec![resolve_model(&client, path, None)?.model]
        };
        if all && !args.flag("yes") {
            let flags = Confirm { yes: false, dry_run: false, json };
            if !context.confirm(flags, &format!("Run a backtest with {} models?", models.len()))? {
                return context.say("nothing run");
            }
        }
        let before = store.database_revision()?;
        let report = run_backtest(store, &BacktestOptions { models, created_at: None }, &client)?;
        if store.database_revision()? != before {
            return Err(refuse("backtest changed the athlete database; report was not written"));
        }
        let written = write_backtest_report(path, &report)?;
        if json {
            let output = BacktestOutput { report: written.display().to_string(), body: report };
            return context.say_json(&json::to_value(&output));
        }
        context.say(&render_backtest(&report))?;
        context.say(&format!("wrote {}", written.display()))
    })
}

/// `ath share`: still a placeholder. It names the newest backtest report.
pub fn share(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let cwd = context.io.cwd.clone();
    let athlete_path =
        context.find_database(args.value("file")).unwrap_or_else(|_| Path::new(&cwd).join(DEFAULT_FILENAME));
    let status = share_status(&athlete_path, &cwd)?;
    if args.flag("json") {
        return context.say_json(&json::to_value(&status));
    }
    context.say(&share_refusal(status.latest_report.as_deref()))
}
