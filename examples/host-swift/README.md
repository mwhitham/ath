# Swift example client for `ath-host`

This Swift package shows how an app talks to `ath-host`, the optional program that
serves athletes' records ([spec 2.1.3, section 7](../../build-history/v2.1.3/spec.md#7-clients-d266)).
It is example code to copy from. It is not published as a package.

The package needs Swift 6. It declares macOS 14 and iOS 17.

## What it has

- `AthleticStandardHost`: a library with `HostClient`. It covers the calls a coach app
  makes: capabilities, `me`, the roster, workout summaries with paging, adding a
  coaching note, and accepting a grant offer. It also covers the owner calls the tests
  need: creating a record, restoring a package, and creating, listing, and revoking
  grants.
- `host-example`: a command-line program that runs the coach flow and prints each step
  in plain words.
- Tests that start a real `ath-host` and run the flow against it.

## Run the coach flow

```sh
cd examples/host-swift
swift build
ATH_HOST_URL=https://records.example.org \
ATH_HOST_TOKEN='<a coach access token>' \
ATH_GRANT_CODE='<an offer code from the athlete>' \
swift run host-example
```

`ATH_GRANT_CODE` is optional. Leave it out when the coach has already accepted the
offer. `ATH_NOTE` sets the text of the note. The program prints lines such as:

```
You accepted an athlete's offer. You can now read workout summaries and add coaching notes.
You coach 1 athlete through this app.
- Demo athlete: you can read workout summaries and add coaching notes.
Demo athlete has these recent workouts:
- crossfit from 2026-08-10T17:25:00.000Z to 2026-08-10T17:48:00.000Z
You added a note about the crossfit on 2026-08-10T17:25:00.000Z. The host saved it as record ….
You sent the same note again with the same key. The host returned the first result and saved nothing new.
```

## How a real app signs in

An app signs the person in through the system browser, with the OAuth authorization
code flow and PKCE, as RFC 8252 describes for native apps
([D260](../../build-history/v2.1.3/decisions.md#d260-sign-in-is-provider-neutral-openid-connect)).
On iOS and macOS, `ASWebAuthenticationSession` opens the system browser. The app does
not show its own password form. The sign-in provider handles passwords, recovery, and
second factors, so the app never sees them.

1. Read the host's protected-resource metadata (RFC 9728) with
   `HostClient.protectedResource()`. It names the host's `resource` identifier and the
   sign-in provider in `authorization_servers`.
2. Read the provider's OpenID Connect discovery document. Run the authorization code
   flow with PKCE in the system browser.
3. Ask for an access token whose audience is the host's `resource` identifier. The host
   refuses a token meant for another service. Providers ask for the audience in
   different ways: some take a `resource` parameter (RFC 8707), and some take an API
   identifier or an `audience` parameter.
4. Create the client with `HostClient(baseURL:session:accessToken:)`. The function you
   pass returns a current token and refreshes it when it has expired. A refused token
   gives `HostError.Code.signInRequired`.

The host also checks which app the token was issued to, from the `azp` or `client_id`
claim. A grant names one app. A coach who accepted an offer in the coach app cannot use
that grant from another app.

## What the host decides

The host applies the athlete's grants on every request. The app never gets database
credentials
([D266](../../build-history/v2.1.3/decisions.md#d266-clients-reach-records-only-through-the-host-api)).
A database credential would let an app skip the grants, the retry receipts, and the
revocation check.

- A coach gets only what the athlete granted
  ([D262](../../build-history/v2.1.3/decisions.md#d262-grants-are-checked-on-every-request-revocation-raises-an-access-epoch)):
  workout summaries (record ID, revision, start, end, and activity type), adding
  notes, or adding training plans.
- After the athlete revokes a grant, the coach's next request about that athlete gets
  `notFound`. A note that was waiting to commit fails. The host gives the same answer
  for a record that does not exist, so a coach cannot learn whether a record exists.
- `HostClient` tells `URLSession` not to use cached replies. A cached list could show
  data after a revocation.

## Retrying a write

Each write sends an `Idempotency-Key` header
([D261](../../build-history/v2.1.3/decisions.md#d261-shared-edits-go-through-the-host)).
Make the key once, when the coach saves the note, for example with
`UUID().uuidString`. Keep the key with the unsent note. After a lost reply, send the
same note with the same key. The host returns the first result and saves nothing new.
The same key with a different note gets `requestConflict`.

The host compares the exact request bytes. The same note with its keys in another order
counts as a different request. `HostClient` encodes JSON with sorted keys, so the same
note gives the same bytes on every try, including after the app restarts.

## Errors

The host answers an error with a stable code. `HostClient` throws `HostError` with the
HTTP status, the `code`, and the message. A failure to reach the host stays a
`URLError`. A reply without the host's error body, such as the answer to an unreadable
query string, has a nil `code`.

## Tests

The tests start a real `ath-host` in native directory mode. They sign access tokens
with a fixed, test-only Ed25519 key, the same one `host/tests/support/mod.rs` uses, and
serve its public key from a small loopback server. They cover these steps:

- An owner creates a record, restores the demo package, and offers a grant.
- A coach accepts the offer with the coach app. The same coach in another app cannot.
- The coach reads the roster and the workout summaries across two pages.
- The coach sends a note twice with the same key and gets the same result.
- The owner revokes the grant, and the coach's requests get `notFound`.
- `host-example` runs the coach flow.

Build the Rust programs first. The tests run `target/debug/ath-host`, and they run
`target/debug/ath` to export the demo record as a package.

```sh
cargo build            # in the repository root
cd examples/host-swift
swift build
swift test
```

Set `ATH_HOST_EXECUTABLE` or `ATH_NATIVE_EXECUTABLE` to use other builds. The tests run
on macOS because they start the host as a separate program.

Some Command Line Tools releases leave the Swift Testing macro plugin out of the build
at random. `swift test` then reports `plugin for module 'TestingMacros' not found`.
Pass the plugin folder yourself:

```sh
swift test -Xswiftc -plugin-path -Xswiftc "$(xcode-select -p)/usr/lib/swift/host/plugins/testing"
```
