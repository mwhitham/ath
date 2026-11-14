//! Keys live in the password store, never in the athlete file (D75).
//! Ported from `tests/keyring.test.ts`. The operating-system stores run
//! against fake `security`, `secret-tool`, and `powershell` programs under a
//! temporary directory and a test service name. No test reads or changes the
//! real password store.

use athleticstandard::runtime::keyring::{
    describe_cleared, describe_key_status, missing_store_message, parse_gateway, read_secret, Gateway, KeyAccount,
    KeyStatus, Keyring, MemorySecretStore, OsSecretStore, SecretStore, GARMIN_ACCOUNT, KEYRING_SERVICE,
};

const NO_ENV: [(&str, &str); 0] = [];

#[test]
fn the_service_and_accounts_are_unchanged() {
    assert_eq!(KEYRING_SERVICE, "athleticstandard");
    assert_eq!(KeyAccount::Gateway(Gateway::Vercel).account(), "vercel");
    assert_eq!(KeyAccount::Gateway(Gateway::OpenRouter).account(), "openrouter");
    assert_eq!(KeyAccount::Garmin.account(), GARMIN_ACCOUNT);
    assert_eq!(GARMIN_ACCOUNT, "garmin");
}

#[test]
fn saves_and_clears_a_key_in_a_fake_store() {
    let store = MemorySecretStore::new();
    let keyring = Keyring::new(&store, &NO_ENV);
    keyring.set(KeyAccount::Gateway(Gateway::Vercel), "sk-test-vercel").unwrap();
    assert_eq!(keyring.saved_gateways(), [Gateway::Vercel]);
    assert_eq!(keyring.resolve_key(Gateway::Vercel).as_deref(), Some("sk-test-vercel"));
    assert_eq!(keyring.clear(Some(Gateway::Vercel)), [Gateway::Vercel]);
    assert!(keyring.saved_gateways().is_empty());
    assert!(keyring.clear(None).is_empty());
}

#[test]
fn a_passed_key_is_trimmed_and_an_empty_key_is_refused() {
    let store = MemorySecretStore::new();
    let keyring = Keyring::new(&store, &NO_ENV);
    keyring.set(KeyAccount::Gateway(Gateway::OpenRouter), "  sk-or-test\n").unwrap();
    assert_eq!(keyring.resolve_key(Gateway::OpenRouter).as_deref(), Some("sk-or-test"));
    let refusal = keyring.set(KeyAccount::Gateway(Gateway::Vercel), " \t").unwrap_err();
    assert_eq!(refusal.message, "no key was entered, so nothing was saved.");
    assert!(store.get("vercel").is_none());
}

#[test]
fn reads_a_key_from_the_environment_for_scripts() {
    let store = MemorySecretStore::new();
    let env = [("AI_GATEWAY_API_KEY", "sk-env")];
    let keyring = Keyring::new(&store, &env);
    assert_eq!(keyring.resolve_gateway(None).unwrap(), Some(Gateway::Vercel));
    assert_eq!(keyring.resolve_key(Gateway::Vercel).as_deref(), Some("sk-env"));
    assert_eq!(keyring.env_gateways(), [Gateway::Vercel]);

    let fallback = [("AI_GATEWAY_API_KEY", "  "), ("VERCEL_AI_GATEWAY_API_KEY", "sk-second")];
    assert_eq!(Keyring::new(&store, &fallback).resolve_key(Gateway::Vercel).as_deref(), Some("sk-second"));
}

#[test]
fn the_environment_wins_over_a_saved_key() {
    let store = MemorySecretStore::new();
    store.set("openrouter", "saved").unwrap();
    let env = [("OPENROUTER_API_KEY", "from-env")];
    let keyring = Keyring::new(&store, &env);
    assert_eq!(keyring.resolve_key(Gateway::OpenRouter).as_deref(), Some("from-env"));
    assert_eq!(
        keyring.status(),
        KeyStatus { saved: vec![Gateway::OpenRouter], environment: vec![Gateway::OpenRouter] }
    );
}

#[test]
fn refuses_to_guess_when_both_gateways_are_available() {
    let store = MemorySecretStore::new();
    let keyring = Keyring::new(&store, &NO_ENV);
    keyring.set(KeyAccount::Gateway(Gateway::Vercel), "a").unwrap();
    keyring.set(KeyAccount::Gateway(Gateway::OpenRouter), "b").unwrap();
    let refusal = keyring.resolve_gateway(None).unwrap_err();
    assert_eq!(
        refusal.message,
        "both vercel and openrouter keys are available. Pass --gateway vercel or --gateway openrouter."
    );
    assert_eq!(keyring.resolve_gateway(Some(Gateway::OpenRouter)).unwrap(), Some(Gateway::OpenRouter));
    assert_eq!(keyring.clear(None), [Gateway::Vercel, Gateway::OpenRouter]);
    assert_eq!(keyring.resolve_gateway(None).unwrap(), None);
    assert_eq!(keyring.resolve_gateway(Some(Gateway::Vercel)).unwrap(), None);
}

#[test]
fn status_names_the_gateway_and_the_store_never_the_key() {
    let text = describe_key_status(&[Gateway::Vercel], &[]);
    assert!(text.contains("vercel"));
    assert!(!text.contains("sk-"));
    assert!(text.contains("password store"));
    assert!(text.ends_with("The key itself is not printed."));

    let env_text = describe_key_status(&[], &[Gateway::OpenRouter]);
    assert!(env_text.contains("Set in the environment: openrouter (OPENROUTER_API_KEY)."));

    let none = describe_key_status(&[], &[]);
    assert!(none.ends_with(
        "No gateway key is saved.\nRun `ath key set vercel` or `ath key set openrouter`, or set\n\
         AI_GATEWAY_API_KEY / OPENROUTER_API_KEY in the environment."
    ));

    let json = serde_json::to_string(&KeyStatus { saved: vec![Gateway::Vercel], environment: vec![] }).unwrap();
    assert_eq!(json, r#"{"saved":["vercel"],"environment":[]}"#);
    assert_eq!(describe_cleared(&[]), "no gateway key was saved.");
    assert_eq!(
        describe_cleared(&[Gateway::Vercel, Gateway::OpenRouter]),
        "cleared vercel and openrouter from the password store."
    );
}

#[test]
fn key_set_also_accepts_the_undocumented_garmin_account() {
    let garmin = KeyAccount::for_key_set("garmin").unwrap();
    assert_eq!(garmin, KeyAccount::Garmin);
    assert_eq!(garmin.prompt(), "Paste your Garmin personal API key: ");
    assert_eq!(garmin.saved_message(), "saved a garmin key in the password store. The key is not printed.");
    let vercel = KeyAccount::for_key_set("vercel").unwrap();
    assert_eq!(vercel.prompt(), "Paste your vercel key: ");
    assert_eq!(vercel.saved_message(), "saved a vercel key in the password store. The key is not printed.");
    assert_eq!(
        KeyAccount::for_key_set("other").unwrap_err().to_string(),
        "unknown gateway 'other'. Use vercel or openrouter."
    );
    // `ath key clear garmin` is refused as an unknown gateway.
    assert!(parse_gateway("garmin").is_err());

    let store = MemorySecretStore::new();
    let keyring = Keyring::new(&store, &NO_ENV);
    keyring.set(garmin, "garmin-saved").unwrap();
    assert_eq!(keyring.account_secret("garmin").as_deref(), Some("garmin-saved"));
    assert!(keyring.saved_gateways().is_empty(), "the Garmin key is not a gateway key");
    assert!(keyring.clear(None).is_empty(), "clearing gateways leaves the Garmin key");
    assert_eq!(store.get("garmin").as_deref(), Some("garmin-saved"));

    let env = [("GARMIN_PERSONAL_API_KEY", " garmin-env ")];
    assert_eq!(Keyring::new(&store, &env).account_secret("garmin").as_deref(), Some("garmin-env"));
    let other = [("GARMIN_PERSONAL_API_KEY", "x")];
    assert_eq!(Keyring::new(&store, &other).account_secret("other"), None);
}

#[test]
fn pasted_keys_follow_the_terminal_rules() {
    assert_eq!(read_secret(&mut &b"sk-abc\r"[..]).unwrap(), "sk-abc");
    assert_eq!(read_secret(&mut &b"x\x03"[..]).unwrap_err().message, "nothing was saved.");
}

#[test]
fn key_refusals_become_library_refusals() {
    let store = MemorySecretStore::new();
    let refusal = Keyring::new(&store, &NO_ENV).set(KeyAccount::Garmin, "").unwrap_err();
    let error: athleticstandard::Error = refusal.into();
    assert_eq!(error.code(), "refused");
}

#[cfg(unix)]
mod system_stores {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    const SERVICE: &str = "athleticstandard-test";

    fn fake_tool(directory: &Path, name: &str, body: &str) -> PathBuf {
        let path = directory.join(name);
        let script = format!("#!/bin/sh\nstore='{}'\n{body}", directory.display());
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    fn log(directory: &Path) -> String {
        fs::read_to_string(directory.join("log")).unwrap_or_default()
    }

    const SECURITY: &str = r#"printf '%s\n' "$*" >> "$store/log"
command="$1"; shift
service=""; account=""; secret=""
while [ $# -gt 0 ]; do
  case "$1" in
    -s) service="$2"; shift 2;;
    -a) account="$2"; shift 2;;
    -w) if [ $# -ge 2 ]; then secret="$2"; shift 2; else shift; fi;;
    *) shift;;
  esac
done
file="$store/item-$service-$account"
case "$command" in
  find-generic-password)
    [ -f "$file" ] || { echo "The specified item could not be found in the keychain." >&2; exit 44; }
    cat "$file"; echo;;
  add-generic-password)
    [ -f "$store/refuse" ] && { echo "User interaction is not allowed." >&2; exit 36; }
    printf '%s' "$secret" > "$file";;
  delete-generic-password) rm -f "$file";;
esac
"#;

    const SECRET_TOOL: &str = r#"printf '%s\n' "$*" >> "$store/log"
command="$1"; shift
if [ "$command" = store ]; then shift 2; fi
file="$store/item-$2-$4"
case "$command" in
  lookup) [ -f "$file" ] || exit 1; cat "$file";;
  store)
    [ -f "$store/refuse" ] && { echo "Cannot create an item in a locked collection" >&2; exit 1; }
    cat > "$file";;
  clear) rm -f "$file";;
esac
"#;

    const POWERSHELL: &str = r#"printf '%s %s %s args=%s\n' "$ATH_KEYRING_OP" "$ATH_KEYRING_TARGET" "$1 $2 $3" "$#" >> "$store/log"
case "$4" in *CredWrite*) ;; *) exit 9;; esac
file="$store/item-$(printf '%s' "$ATH_KEYRING_TARGET" | tr '/' '_')"
case "$ATH_KEYRING_OP" in
  get) [ -f "$file" ] || exit 1; cat "$file";;
  set)
    [ -f "$store/refuse" ] && { echo "CredWrite failed" >&2; exit 1; }
    printf '%s' "$ATH_KEYRING_SECRET" > "$file";;
  delete) rm -f "$file";;
esac
"#;

    fn round_trip(store: &OsSecretStore, directory: &Path) {
        let keyring = Keyring::new(store, &NO_ENV);
        assert!(keyring.saved_gateways().is_empty());
        keyring.set(KeyAccount::Gateway(Gateway::Vercel), "sk-fake-1").unwrap();
        keyring.set(KeyAccount::Garmin, "garmin-fake").unwrap();
        assert_eq!(keyring.saved_gateways(), [Gateway::Vercel]);
        assert_eq!(keyring.resolve_key(Gateway::Vercel).as_deref(), Some("sk-fake-1"));
        assert_eq!(keyring.account_secret("garmin").as_deref(), Some("garmin-fake"));
        assert_eq!(keyring.clear(None), [Gateway::Vercel]);
        assert!(keyring.saved_gateways().is_empty());
        assert_eq!(keyring.account_secret("garmin").as_deref(), Some("garmin-fake"));

        fs::write(directory.join("refuse"), "").unwrap();
        let refusal = keyring.set(KeyAccount::Gateway(Gateway::OpenRouter), "sk-fake-2").unwrap_err();
        assert!(refusal.message.ends_with('.'), "{}", refusal.message);
        assert!(store.get("openrouter").is_none());
    }

    #[test]
    fn keychain_uses_security_with_the_service_and_account() {
        let directory = tempfile::tempdir().unwrap();
        let tool = fake_tool(directory.path(), "security", SECURITY);
        let store = OsSecretStore::keychain(SERVICE, tool);
        assert_eq!(store.service(), SERVICE);
        round_trip(&store, directory.path());
        let calls = log(directory.path());
        assert!(calls.contains(&format!("add-generic-password -U -s {SERVICE} -a vercel -w sk-fake-1")));
        assert!(calls.contains(&format!("find-generic-password -s {SERVICE} -a vercel -w")));
        assert!(calls.contains(&format!("delete-generic-password -s {SERVICE} -a vercel")));
        let refusal = Keyring::new(&store, &NO_ENV).set(KeyAccount::Garmin, "x").unwrap_err();
        assert_eq!(refusal.message, "Keychain refused to save the key: User interaction is not allowed..");
    }

    #[test]
    fn the_secret_service_reads_the_key_from_standard_input() {
        let directory = tempfile::tempdir().unwrap();
        let tool = fake_tool(directory.path(), "secret-tool", SECRET_TOOL);
        let store = OsSecretStore::secret_service(SERVICE, Some(tool));
        round_trip(&store, directory.path());
        let calls = log(directory.path());
        assert!(calls.contains(&format!("store --label Athletic Standard (vercel) service {SERVICE} account vercel")));
        assert!(calls.contains(&format!("lookup service {SERVICE} account vercel")));
        assert!(calls.contains(&format!("clear service {SERVICE} account vercel")));
        assert!(!calls.contains("sk-fake"), "the key is never an argument");
        let refusal = Keyring::new(&store, &NO_ENV).set(KeyAccount::Garmin, "x").unwrap_err();
        assert_eq!(
            refusal.message,
            "the secret service refused to save the key: Cannot create an item in a locked collection."
        );
    }

    #[test]
    fn credential_manager_passes_the_key_in_the_environment() {
        let directory = tempfile::tempdir().unwrap();
        let tool = fake_tool(directory.path(), "powershell", POWERSHELL);
        let store = OsSecretStore::credential_manager(SERVICE, Some(tool));
        round_trip(&store, directory.path());
        let calls = log(directory.path());
        assert!(calls.contains(&format!("set {SERVICE}/vercel -NoProfile -NonInteractive -Command args=4")));
        assert!(calls.contains(&format!("get {SERVICE}/vercel")));
        assert!(calls.contains(&format!("delete {SERVICE}/vercel")));
        assert!(!calls.contains("sk-fake"), "the key is never an argument");
        let refusal = Keyring::new(&store, &NO_ENV).set(KeyAccount::Garmin, "x").unwrap_err();
        assert_eq!(refusal.message, "Credential Manager refused to save the key: CredWrite failed.");
    }

    #[test]
    fn a_missing_store_refuses_and_never_writes_a_file() {
        let directory = tempfile::tempdir().unwrap();
        for store in [OsSecretStore::secret_service(SERVICE, None), OsSecretStore::credential_manager(SERVICE, None)] {
            let keyring = Keyring::new(&store, &NO_ENV);
            assert!(keyring.saved_gateways().is_empty());
            let refusal = keyring.set(KeyAccount::Gateway(Gateway::Vercel), "sk-fake").unwrap_err();
            assert_eq!(refusal.message, missing_store_message());
            assert!(refusal.message.contains("We will not write the key to a file."));
            assert!(keyring.clear(None).is_empty());
        }
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_store_tool_that_cannot_start_reports_no_key_and_refuses_to_save() {
        let directory = tempfile::tempdir().unwrap();
        let store = OsSecretStore::keychain(SERVICE, directory.path().join("no-such-security"));
        let keyring = Keyring::new(&store, &NO_ENV);
        assert_eq!(keyring.resolve_key(Gateway::Vercel), None);
        let refusal = keyring.set(KeyAccount::Gateway(Gateway::Vercel), "sk-fake").unwrap_err();
        assert!(refusal.message.starts_with("Keychain refused to save the key: "));
    }
}
