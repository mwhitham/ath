//! Gateway and provider keys live in the computer's password store, never in
//! the athlete file (D75). Agents read that file.
//!
//! The store is Keychain on a Mac, Credential Manager on Windows, and the
//! secret service on Linux and other systems. The operating system encrypts
//! the key and unlocks it when the person is logged in. This module does not
//! invent its own lock, and it never writes a plaintext file when the store
//! is missing.
//!
//! Each store is reached through the system's own tool, as version 2.1.0 did:
//! `security` on a Mac, `secret-tool` elsewhere, and PowerShell calling
//! `CredRead`/`CredWrite` on Windows. The service name and account names are
//! unchanged, so keys saved by 2.1.0 remain usable.
//!
//! A key in the environment wins over a saved key, for scripts.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Serialize;

use super::Environment;
use crate::error::Error;

/// The password-store service name every key is saved under.
pub const KEYRING_SERVICE: &str = "athleticstandard";

/// The account the Garmin personal API key is saved under.
pub const GARMIN_ACCOUNT: &str = "garmin";

/// The environment variable that holds a Garmin personal API key.
pub const GARMIN_KEY_VARIABLE: &str = "GARMIN_PERSONAL_API_KEY";

/// A model gateway the terminal can call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Gateway {
    Vercel,
    #[serde(rename = "openrouter")]
    OpenRouter,
}

impl Gateway {
    /// Both gateways, in the order status and selection use.
    pub const ALL: [Gateway; 2] = [Gateway::Vercel, Gateway::OpenRouter];

    /// The gateway with exactly this name.
    pub fn from_name(name: &str) -> Option<Gateway> {
        match name {
            "vercel" => Some(Gateway::Vercel),
            "openrouter" => Some(Gateway::OpenRouter),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Gateway::Vercel => "vercel",
            Gateway::OpenRouter => "openrouter",
        }
    }

    /// The environment variable `ath key` names for this gateway.
    pub fn env_var(self) -> &'static str {
        match self {
            Gateway::Vercel => "AI_GATEWAY_API_KEY",
            Gateway::OpenRouter => "OPENROUTER_API_KEY",
        }
    }

    /// Every environment variable read for this gateway, in precedence order.
    pub fn env_vars(self) -> &'static [&'static str] {
        match self {
            Gateway::Vercel => &["AI_GATEWAY_API_KEY", "VERCEL_AI_GATEWAY_API_KEY"],
            Gateway::OpenRouter => &["OPENROUTER_API_KEY"],
        }
    }
}

impl std::fmt::Display for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `--gateway <name>` and `ath key clear <gateway>`.
pub fn parse_gateway(value: &str) -> Result<Gateway, Error> {
    Gateway::from_name(value)
        .ok_or_else(|| Error::refused(format!("unknown gateway '{value}'. Use vercel or openrouter.")))
}

/// The store cannot do what was asked. The message names the remedy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRefusal {
    pub message: String,
}

impl KeyRefusal {
    pub fn new(message: impl Into<String>) -> Self {
        KeyRefusal { message: message.into() }
    }
}

impl std::fmt::Display for KeyRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for KeyRefusal {}

impl From<KeyRefusal> for Error {
    fn from(refusal: KeyRefusal) -> Self {
        Error::Refused(refusal.message)
    }
}

/// A password store holding one secret per account under one service name.
///
/// Reading never fails: a store that cannot be read reports no key, as 2.1.0
/// did. Saving fails with a refusal. Deleting ignores a missing key.
pub trait SecretStore {
    fn get(&self, account: &str) -> Option<String>;
    fn set(&self, account: &str, secret: &str) -> Result<(), KeyRefusal>;
    fn delete(&self, account: &str);
}

/// An in-memory store for tests. It is never used as a fallback.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    secrets: RefCell<HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        MemorySecretStore::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, account: &str) -> Option<String> {
        self.secrets.borrow().get(account).cloned()
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), KeyRefusal> {
        self.secrets.borrow_mut().insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) {
        self.secrets.borrow_mut().remove(account);
    }
}

/// The operating system's password store, reached through its own tool.
#[derive(Debug, Clone)]
pub struct OsSecretStore {
    service: String,
    backend: Backend,
}

#[derive(Debug, Clone)]
enum Backend {
    Keychain { security: PathBuf },
    SecretService { secret_tool: Option<PathBuf> },
    CredentialManager { powershell: Option<PathBuf> },
}

impl OsSecretStore {
    /// The store this system uses, with the real service name. Nothing is
    /// spawned until a key is read, saved, or deleted.
    pub fn for_this_system() -> Self {
        #[cfg(target_os = "macos")]
        let store = OsSecretStore::keychain(KEYRING_SERVICE, PathBuf::from("security"));
        #[cfg(windows)]
        let store = OsSecretStore::credential_manager(KEYRING_SERVICE, super::find_on_path("powershell"));
        #[cfg(not(any(target_os = "macos", windows)))]
        let store = OsSecretStore::secret_service(KEYRING_SERVICE, super::find_on_path("secret-tool"));
        store
    }

    /// Keychain through `security`.
    pub fn keychain(service: impl Into<String>, security: PathBuf) -> Self {
        OsSecretStore { service: service.into(), backend: Backend::Keychain { security } }
    }

    /// The secret service through `secret-tool`. None means the tool is not
    /// installed: no key can be read, and saving refuses.
    pub fn secret_service(service: impl Into<String>, secret_tool: Option<PathBuf>) -> Self {
        OsSecretStore { service: service.into(), backend: Backend::SecretService { secret_tool } }
    }

    /// Credential Manager through PowerShell. None means PowerShell is not
    /// available: no key can be read, and saving refuses.
    pub fn credential_manager(service: impl Into<String>, powershell: Option<PathBuf>) -> Self {
        OsSecretStore { service: service.into(), backend: Backend::CredentialManager { powershell } }
    }

    pub fn service(&self) -> &str {
        &self.service
    }
}

/// The refusal when no password store can be used.
pub fn missing_store_message() -> String {
    format!(
        "this computer has no password store we can use, so the key was not saved. \
         Install the secret service (secret-tool) or set {} / {} in the environment. \
         We will not write the key to a file.",
        Gateway::Vercel.env_var(),
        Gateway::OpenRouter.env_var()
    )
}

struct ToolOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run_tool(mut command: Command, input: Option<&str>) -> ToolOutput {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return ToolOutput { success: false, stdout: String::new(), stderr: error.to_string() },
    };
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    match child.wait_with_output() {
        Ok(output) => ToolOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
        Err(error) => ToolOutput { success: false, stdout: String::new(), stderr: error.to_string() },
    }
}

fn tool_detail(output: &ToolOutput) -> String {
    let text = if output.stderr.is_empty() { output.stdout.trim() } else { output.stderr.trim() };
    // The callers end the sentence with their own period.
    let text = text.trim_end_matches('.');
    if text.is_empty() {
        "unknown error".to_string()
    } else {
        text.to_string()
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

impl SecretStore for OsSecretStore {
    fn get(&self, account: &str) -> Option<String> {
        match &self.backend {
            Backend::Keychain { security } => {
                let mut command = Command::new(security);
                command.args(["find-generic-password", "-s", &self.service, "-a", account, "-w"]);
                let output = run_tool(command, None);
                output.success.then(|| output.stdout.trim().to_string()).and_then(non_empty)
            }
            Backend::SecretService { secret_tool: Some(tool) } => {
                let mut command = Command::new(tool);
                command.args(["lookup", "service", &self.service, "account", account]);
                let output = run_tool(command, None);
                let value = output.stdout.strip_suffix('\n').unwrap_or(&output.stdout).to_string();
                output.success.then_some(value).and_then(non_empty)
            }
            Backend::CredentialManager { powershell: Some(powershell) } => {
                let output = run_tool(credential_command(powershell, "get", &self.service, account, ""), None);
                output.success.then(|| output.stdout.trim().to_string()).and_then(non_empty)
            }
            Backend::SecretService { secret_tool: None } | Backend::CredentialManager { powershell: None } => None,
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), KeyRefusal> {
        match &self.backend {
            Backend::Keychain { security } => {
                // As in 2.1.0, `security` takes the key as an argument.
                let mut command = Command::new(security);
                command.args(["add-generic-password", "-U", "-s", &self.service, "-a", account, "-w", secret]);
                let output = run_tool(command, None);
                if !output.success {
                    return Err(KeyRefusal::new(format!(
                        "Keychain refused to save the key: {}.",
                        tool_detail(&output)
                    )));
                }
                Ok(())
            }
            Backend::SecretService { secret_tool: Some(tool) } => {
                let mut command = Command::new(tool);
                command.args([
                    "store",
                    "--label",
                    &format!("Athletic Standard ({account})"),
                    "service",
                    &self.service,
                    "account",
                    account,
                ]);
                let output = run_tool(command, Some(&format!("{secret}\n")));
                if !output.success {
                    return Err(KeyRefusal::new(format!(
                        "the secret service refused to save the key: {}.",
                        tool_detail(&output)
                    )));
                }
                Ok(())
            }
            Backend::CredentialManager { powershell: Some(powershell) } => {
                let output = run_tool(credential_command(powershell, "set", &self.service, account, secret), None);
                if !output.success {
                    return Err(KeyRefusal::new(format!(
                        "Credential Manager refused to save the key: {}.",
                        tool_detail(&output)
                    )));
                }
                Ok(())
            }
            Backend::SecretService { secret_tool: None } | Backend::CredentialManager { powershell: None } => {
                Err(KeyRefusal::new(missing_store_message()))
            }
        }
    }

    fn delete(&self, account: &str) {
        match &self.backend {
            Backend::Keychain { security } => {
                let mut command = Command::new(security);
                command.args(["delete-generic-password", "-s", &self.service, "-a", account]);
                run_tool(command, None);
            }
            Backend::SecretService { secret_tool: Some(tool) } => {
                let mut command = Command::new(tool);
                command.args(["clear", "service", &self.service, "account", account]);
                run_tool(command, None);
            }
            Backend::CredentialManager { powershell: Some(powershell) } => {
                run_tool(credential_command(powershell, "delete", &self.service, account, ""), None);
            }
            Backend::SecretService { secret_tool: None } | Backend::CredentialManager { powershell: None } => {}
        }
    }
}

/// Windows Credential Manager through a small `CredWrite`/`CredRead` helper.
/// The secret travels in an environment variable, never on the command line.
fn credential_command(powershell: &PathBuf, operation: &str, service: &str, account: &str, secret: &str) -> Command {
    let mut command = Command::new(powershell);
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", CREDENTIAL_SCRIPT])
        .env("ATH_KEYRING_OP", operation)
        .env("ATH_KEYRING_TARGET", format!("{service}/{account}"))
        .env("ATH_KEYRING_SECRET", secret);
    command
}

const CREDENTIAL_SCRIPT: &str = r#"
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class AthCred {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  public struct CREDENTIAL {
    public uint Flags;
    public uint Type;
    public string TargetName;
    public string Comment;
    public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
    public uint CredentialBlobSize;
    public IntPtr CredentialBlob;
    public uint Persist;
    public uint AttributeCount;
    public IntPtr Attributes;
    public string TargetAlias;
    public string UserName;
  }
  [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  public static extern bool CredWrite([In] ref CREDENTIAL userCredential, uint flags);
  [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  public static extern bool CredRead(string target, uint type, uint reserved, out IntPtr credentialPtr);
  [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  public static extern bool CredDelete(string target, uint type, uint reserved);
  [DllImport("advapi32.dll")]
  public static extern void CredFree(IntPtr cred);
}
"@
$op = $env:ATH_KEYRING_OP
$target = $env:ATH_KEYRING_TARGET
if ($op -eq 'get') {
  $ptr = [IntPtr]::Zero
  if (-not [AthCred]::CredRead($target, 1, 0, [ref]$ptr)) { exit 1 }
  $cred = [System.Runtime.InteropServices.Marshal]::PtrToStructure($ptr, [type][AthCred+CREDENTIAL])
  $bytes = New-Object byte[] $cred.CredentialBlobSize
  [System.Runtime.InteropServices.Marshal]::Copy($cred.CredentialBlob, $bytes, 0, $cred.CredentialBlobSize)
  [AthCred]::CredFree($ptr)
  [Console]::Write([System.Text.Encoding]::Unicode.GetString($bytes))
} elseif ($op -eq 'set') {
  $secret = $env:ATH_KEYRING_SECRET
  $bytes = [System.Text.Encoding]::Unicode.GetBytes($secret)
  $ptr = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($bytes.Length)
  [System.Runtime.InteropServices.Marshal]::Copy($bytes, 0, $ptr, $bytes.Length)
  $cred = New-Object AthCred+CREDENTIAL
  $cred.Type = 1
  $cred.TargetName = $target
  $cred.UserName = 'athleticstandard'
  $cred.CredentialBlobSize = $bytes.Length
  $cred.CredentialBlob = $ptr
  $cred.Persist = 2
  $ok = [AthCred]::CredWrite([ref]$cred, 0)
  [System.Runtime.InteropServices.Marshal]::FreeHGlobal($ptr)
  if (-not $ok) { exit 1 }
} elseif ($op -eq 'delete') {
  [AthCred]::CredDelete($target, 1, 0) | Out-Null
}
"#;

/// The account `ath key set` saves under. `garmin` is accepted although the
/// argument help names only the two gateways, as in 2.1.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAccount {
    Gateway(Gateway),
    Garmin,
}

impl KeyAccount {
    /// The account for `ath key set <argument>`.
    pub fn for_key_set(argument: &str) -> Result<KeyAccount, Error> {
        if argument == GARMIN_ACCOUNT {
            return Ok(KeyAccount::Garmin);
        }
        parse_gateway(argument).map(KeyAccount::Gateway)
    }

    /// The password-store account name.
    pub fn account(self) -> &'static str {
        match self {
            KeyAccount::Gateway(gateway) => gateway.as_str(),
            KeyAccount::Garmin => GARMIN_ACCOUNT,
        }
    }

    /// The hidden-input prompt for this account.
    pub fn prompt(self) -> String {
        match self {
            KeyAccount::Gateway(gateway) => format!("Paste your {gateway} key: "),
            KeyAccount::Garmin => "Paste your Garmin personal API key: ".to_string(),
        }
    }

    /// The line `ath key set` prints after saving.
    pub fn saved_message(self) -> String {
        format!("saved a {} key in the password store. The key is not printed.", self.account())
    }
}

/// `ath key --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyStatus {
    /// Gateways with a key in the password store.
    pub saved: Vec<Gateway>,
    /// Gateways with a key in the environment.
    pub environment: Vec<Gateway>,
}

/// Key lookup and storage over one password store and one environment.
pub struct Keyring<'a> {
    store: &'a dyn SecretStore,
    env: &'a dyn Environment,
}

impl<'a> Keyring<'a> {
    pub fn new(store: &'a dyn SecretStore, env: &'a dyn Environment) -> Self {
        Keyring { store, env }
    }

    fn first_env(&self, names: &[&str]) -> Option<String> {
        names.iter().find_map(|name| {
            let value = self.env.var(name)?;
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        })
    }

    /// A key already in the environment, for scripts.
    pub fn env_key(&self, gateway: Gateway) -> Option<String> {
        self.first_env(gateway.env_vars())
    }

    /// Gateways with a key in the environment.
    pub fn env_gateways(&self) -> Vec<Gateway> {
        Gateway::ALL.into_iter().filter(|gateway| self.env_key(*gateway).is_some()).collect()
    }

    /// Gateways with a key in the password store.
    pub fn saved_gateways(&self) -> Vec<Gateway> {
        Gateway::ALL.into_iter().filter(|gateway| self.store.get(gateway.as_str()).is_some()).collect()
    }

    /// `ath key --json`.
    pub fn status(&self) -> KeyStatus {
        KeyStatus { saved: self.saved_gateways(), environment: self.env_gateways() }
    }

    /// The key for a gateway: the environment first, then the password store.
    pub fn resolve_key(&self, gateway: Gateway) -> Option<String> {
        self.env_key(gateway).or_else(|| self.store.get(gateway.as_str()))
    }

    /// Which gateway to use, or a refusal when both have keys and none was
    /// named. None when the named gateway, or every gateway, has no key.
    pub fn resolve_gateway(&self, requested: Option<Gateway>) -> Result<Option<Gateway>, KeyRefusal> {
        if let Some(gateway) = requested {
            return Ok(self.resolve_key(gateway).map(|_| gateway));
        }
        let available: Vec<Gateway> =
            Gateway::ALL.into_iter().filter(|gateway| self.resolve_key(*gateway).is_some()).collect();
        match available.as_slice() {
            [] => Ok(None),
            [only] => Ok(Some(*only)),
            _ => Err(KeyRefusal::new(
                "both vercel and openrouter keys are available. Pass --gateway vercel or --gateway openrouter.",
            )),
        }
    }

    /// A provider key such as `garmin`. For Garmin, `GARMIN_PERSONAL_API_KEY`
    /// wins over the saved key.
    pub fn account_secret(&self, account: &str) -> Option<String> {
        if account == GARMIN_ACCOUNT {
            if let Some(value) = self.first_env(&[GARMIN_KEY_VARIABLE]) {
                return Some(value);
            }
        }
        self.store.get(account)
    }

    /// Save a key. Surrounding whitespace is removed; an empty key is refused.
    pub fn set(&self, account: KeyAccount, secret: &str) -> Result<(), KeyRefusal> {
        let value = secret.trim();
        if value.is_empty() {
            return Err(KeyRefusal::new("no key was entered, so nothing was saved."));
        }
        self.store.set(account.account(), value)
    }

    /// Delete saved gateway keys: the named one, or both. Returns the
    /// gateways that had a key.
    pub fn clear(&self, gateway: Option<Gateway>) -> Vec<Gateway> {
        let targets: Vec<Gateway> = match gateway {
            Some(gateway) => vec![gateway],
            None => Gateway::ALL.to_vec(),
        };
        let mut cleared = Vec::new();
        for target in targets {
            if self.store.get(target.as_str()).is_none() {
                continue;
            }
            self.store.delete(target.as_str());
            cleared.push(target);
        }
        cleared
    }
}

/// The human `ath key` status text. It names gateways, never keys.
pub fn describe_key_status(saved: &[Gateway], from_env: &[Gateway]) -> String {
    let mut lines = vec![
        "A gateway key is only for calling a model from a bare terminal.".to_string(),
        "Inside Claude Code, Cursor, or Codex you do not need one.".to_string(),
        "The key never goes in the athlete file.".to_string(),
        String::new(),
    ];
    if saved.is_empty() && from_env.is_empty() {
        lines.push("No gateway key is saved.".to_string());
        lines.push("Run `ath key set vercel` or `ath key set openrouter`, or set".to_string());
        lines.push(format!("{} / {} in the environment.", Gateway::Vercel.env_var(), Gateway::OpenRouter.env_var()));
        return lines.join("\n");
    }
    if !saved.is_empty() {
        let names: Vec<&str> = saved.iter().map(|gateway| gateway.as_str()).collect();
        lines.push(format!("Saved in the password store: {}.", names.join(", ")));
    }
    if !from_env.is_empty() {
        let names: Vec<String> = from_env.iter().map(|gateway| format!("{gateway} ({})", gateway.env_var())).collect();
        lines.push(format!("Set in the environment: {}.", names.join(", ")));
    }
    lines.push("The key itself is not printed.".to_string());
    lines.join("\n")
}

/// The line `ath key clear` prints.
pub fn describe_cleared(cleared: &[Gateway]) -> String {
    if cleared.is_empty() {
        return "no gateway key was saved.".to_string();
    }
    let names: Vec<&str> = cleared.iter().map(|gateway| gateway.as_str()).collect();
    format!("cleared {} from the password store.", names.join(" and "))
}

/// The refusal when there is no terminal to paste a key into.
pub fn no_terminal_message() -> String {
    "there is no terminal to paste a key into. Run this somewhere you can type, \
     or set AI_GATEWAY_API_KEY / OPENROUTER_API_KEY in the environment."
        .to_string()
}

/// Read a pasted key from raw terminal bytes.
///
/// Enter, Ctrl-D, or the end of input finishes. Ctrl-C refuses. Backspace
/// removes one character. Other control characters are ignored.
pub fn read_secret(input: &mut dyn Read) -> Result<String, KeyRefusal> {
    let mut value: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match input.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(KeyRefusal::new(format!("could not read the key: {error}"))),
        }
        match byte[0] {
            b'\n' | b'\r' | 0x04 => break,
            0x03 => return Err(KeyRefusal::new("nothing was saved.")),
            0x7f | 0x08 => {
                // Remove one whole character, not one byte of it.
                while let Some(last) = value.pop() {
                    if last & 0xC0 != 0x80 {
                        break;
                    }
                }
            }
            other if other >= b' ' => value.push(other),
            _ => {}
        }
    }
    Ok(String::from_utf8_lossy(&value).into_owned())
}

/// Hidden input for `ath key set`: write the prompt on standard output and
/// read the key from the terminal with echo off.
///
/// This is a terminal helper for the command layer. Library operations never
/// call it.
pub fn prompt_secret(question: &str) -> Result<String, KeyRefusal> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return Err(KeyRefusal::new(no_terminal_message()));
    }
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(question.as_bytes());
    let _ = stdout.flush();
    let result = read_hidden();
    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();
    result
}

#[cfg(unix)]
fn read_hidden() -> Result<String, KeyRefusal> {
    let stty = |args: &[&str]| -> Option<String> {
        let output = Command::new("stty").args(args).stdin(Stdio::inherit()).stderr(Stdio::null()).output().ok()?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    let saved = stty(&["-g"]).ok_or_else(|| KeyRefusal::new(no_terminal_message()))?;
    if stty(&["raw", "-echo"]).is_none() {
        return Err(KeyRefusal::new(no_terminal_message()));
    }
    let result = read_secret(&mut std::io::stdin().lock());
    stty(&[saved.as_str()]);
    result
}

#[cfg(windows)]
fn read_hidden() -> Result<String, KeyRefusal> {
    // PowerShell reads from the console with masked echo.
    const SCRIPT: &str = "$s = Read-Host -AsSecureString; \
        $p = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($s); \
        try { [Console]::Out.Write([Runtime.InteropServices.Marshal]::PtrToStringBSTR($p)) } \
        finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($p) }";
    let powershell = super::find_on_path("powershell").ok_or_else(|| KeyRefusal::new(no_terminal_message()))?;
    let output = Command::new(powershell)
        .args(["-NoProfile", "-Command", SCRIPT])
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|_| KeyRefusal::new(no_terminal_message()))?;
    if !output.status.success() {
        return Err(KeyRefusal::new("nothing was saved."));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(not(any(unix, windows)))]
fn read_hidden() -> Result<String, KeyRefusal> {
    Err(KeyRefusal::new(no_terminal_message()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_input_handles_control_characters() {
        let read = |bytes: &[u8]| read_secret(&mut &bytes[..]);
        assert_eq!(read(b"sk-abc\r").unwrap(), "sk-abc");
        assert_eq!(read(b"sk-ab\x7fc\n").unwrap(), "sk-ac");
        assert_eq!(read(b"x\x1b[A\x04ignored").unwrap(), "x[A");
        assert_eq!(read("k\u{e9}\x08y\n".as_bytes()).unwrap(), "ky");
        assert_eq!(read(b"abc").unwrap(), "abc");
        assert_eq!(read(b"ab\x03cd").unwrap_err().message, "nothing was saved.");
    }
}
