# Hosting records with ath-host

`ath-host` is an optional program that serves many athletes' records over HTTPS. An
athlete can let a coach read workout summaries and add notes and programming. Apps
reach the records through a versioned API. This guide is for people who run a host
and for people who write apps that call one.

The guide describes the code in [`host/`](../host/) as it is now. Version 2.1.3 is
not finished. Where the [2.1.3 plan](../build-history/v2.1.3/spec.md) asks for
something the code does not do yet, the guide says so. The decisions behind the host
are D254–D268 in [decisions.md](../build-history/v2.1.3/decisions.md).

## 1. What ath-host is and is not

- **It is a separate program.** The `ath` command does not contain it and does not
  need it. Local use needs no account, no network, and no host (D75, D254).
- **It keeps one database per athlete** (D256). Each one is an ordinary Athletic
  Standard database, the same as a local `athlete.ath.sqlite`. A separate directory
  database holds the people, records, invitations, and grants the host knows about.
- **It calls the Athletic Standard library for every record operation.** It has no
  record rules of its own. The two-tier wall, revisions, deletion, and erasure work
  as they do in `ath`.
- **It trusts one sign-in provider that the operator chooses** (D260). It has no
  passwords or accounts of its own.
- **The project does not run a host.** It ships the program and this guide. Whoever
  runs a host is responsible for it and for the records on it.
- **It is not in the release archives or the npm package.** Build it from source
  (section 4.1), or build the container image (section 4.7).

What is built today:

| Part | State |
|---|---|
| Directory mode: one machine, records in a folder | Built and covered by tests. |
| Remote mode: databases on a libSQL server, attachments in an S3-compatible bucket | Built. Tested only against a local stand-in for the server and the bucket. |
| Sign-in checks | Built. Tested with a fixed test key. The recipes in section 6 have not been run against live provider accounts. |
| Grants, revocation, coach notes, and programming | Built and covered by tests. |
| Moves between hosts, the Litestream recipe, scheduled exports, restore drills | Not built (build step 8). |
| A generated description of the API in `schema/host-api/1/` (D266) | Not built. Section 7 is written from `host/src/api.rs`. |

## 2. Choosing a backend

The host keeps records in one of two ways, called backends:

- **Directory mode** keeps each athlete's database and attachments in a folder on the
  host's own disk. One host process owns the folder.
- **Remote mode** keeps each athlete's database on a server that speaks the libSQL
  protocol, and attachments in an S3-compatible bucket. The host keeps only temporary
  working copies, so several copies of the host can serve the same athletes.

Answer these questions in order. Section 6 of the
[storage study](../build-history/v2.1.3/appendix/hosted-storage-options.md) gives
the facts behind each one.

1. **Does anyone but the athlete need to reach the record?** If not, use `ath` with a
   local file. You do not need a host.
2. **Will one disk hold every record you expect?** Estimate from the size of real
   databases: the `.ath.sqlite` file and its `attachments/` folder. If one disk holds
   them all, directory mode is the simplest choice.
3. **Do you need more than one copy of the host running at once, or no disk to
   manage?** Use remote mode. Put the host and the database server in the same
   region. Every request makes at least one network round trip to the server, and
   every write makes several.
4. **Must every part run on machines you control?** Use directory mode, or remote
   mode with a libSQL server you run yourself (section 5.3). The self-run server is
   no longer actively released.
5. **Do you need each athlete's database locked with its own key?** Neither mode
   offers this in the host yet. The plan allows it in remote mode where the provider
   supports it (D264).
6. **Who can read the records?** Whoever runs the host and its storage. Write that in
   `operator_disclosure` (section 3).

No storage provider is listed as supported yet. A provider is listed only after a
named run of the qualification suite passes (D258, spec section 8).

## 3. Configuration

The host reads one JSON file, named with `--config`. The file never holds a secret.
Every field whose name ends in `_env` names an environment variable, and the host
reads the secret from that variable when it starts. A missing variable stops the host
with `the environment variable NAME is not set`.

Unknown fields are refused. A misspelled field stops the host instead of being
ignored.

### 3.1 Top-level fields

| Field | Required | Meaning |
|---|---|---|
| `listen` | Yes | Address and port to listen on, such as `127.0.0.1:8080`. The host speaks plain HTTP. Put TLS in front of it (section 4.4). |
| `public_url` | Yes | The address clients use, such as `https://records.example.org`. The host publishes it as `resource` in its protected-resource metadata (section 6.4). |
| `data_directory` | Yes | The host's own folder. A relative path is read from the folder that holds the configuration file. In directory mode it holds every record. In remote mode it holds working copies and staged files. |
| `storage` | Yes | Where records live. An object whose `kind` is `directory`, `libsql`, or `local_server` (section 3.2). |
| `sign_in` | Yes | The sign-in provider (section 3.4). |
| `operator_disclosure` | Yes | Text shown to an athlete when they connect: who runs this host and who runs its storage (D264). The API returns it from `/v1/capabilities`, from `/v1/athletes/{id}`, and when a record is created. |
| `open_enrollment` | No, default `false` | When `true`, any signed-in person can create a record. When `false`, a person needs a code from `ath-host invite`. |
| `limits` | No | Size and time limits (section 3.5). Every field has a default. |

### 3.2 `storage`

**`"kind": "directory"`** has no other fields. Records live in
`data_directory/athletes/`.

**`"kind": "libsql"`** keeps records on a libSQL server:

| Field | Meaning |
|---|---|
| `url_template` | The address of one athlete's database, with `{database}` where the database name goes. The host fills in `ath-` and 32 hexadecimal characters, taken from the record's resource ID. |
| `directory_url` | The address of the host's directory database. It must exist before the host starts. |
| `token_env` | The variable that holds the access token for every database, the directory included. |
| `provision` | How a new athlete's database is created (section 3.3). |
| `attachments` | The S3-compatible bucket for attachment files (section 3.3). |

**`"kind": "local_server"`** exists for the tests. Each athlete's "server" database
is a file in the folder named by `directory`, and `attachments` names a bucket. It
runs the same commit code as remote mode without a network. Do not use it to serve
athletes.

### 3.3 `provision` and `attachments`

`provision` is an object whose `kind` is one of:

| `kind` | Other fields | What the host does for a new record |
|---|---|---|
| `existing` | None | Nothing. The database must already answer at its address (section 5.4). |
| `turso` | `organization`, `group`, `platform_token_env` | Asks Turso's Platform API to create the database in that organization and group. `platform_token_env` names the variable that holds a Platform API token (section 5.2). |
| `sqld_admin` | `admin_url` | Asks a self-run libSQL server to create the database as a namespace, through its admin address (section 5.3). |

`attachments` describes an S3-compatible bucket:

| Field | Meaning |
|---|---|
| `endpoint` | The bucket service's address, such as `https://ACCOUNT_ID.r2.cloudflarestorage.com`. A path in the address is kept as a prefix of every object address. |
| `bucket` | The bucket name. |
| `region` | The region name the service expects, such as `auto` or `eu-west-1`. |
| `access_key_env` | The variable that holds the access key ID. |
| `secret_key_env` | The variable that holds the secret key. |
| `path_style` | Default `true`: addresses of the form `endpoint/bucket/key`. Set `false` for addresses with the bucket in the host name. Section 5.5 lists the value for common services. |

### 3.4 `sign_in`

| Field | Required | Meaning |
|---|---|---|
| `issuer` | Yes | The provider's issuer, exactly as it appears in the `iss` claim of its tokens. |
| `audience` | Yes | The value the `aud` claim must contain. Use the same value as `public_url`, so clients that ask for a token for the host get one the host accepts. |
| `jwks_uri` | No | Where to read the provider's public signing keys. When absent, the host reads `jwks_uri` from `<issuer>/.well-known/openid-configuration`. |
| `allowed_clients` | No, default empty | The client IDs of the apps the host accepts. When the list is empty, any app the provider issued a token to is accepted. Grants still name the app. |

### 3.5 `limits`

| Field | Default | Meaning |
|---|---|---|
| `body_bytes` | `67108864` (64 MiB) | The largest request body. It bounds package uploads and attachment uploads. |
| `page_size` | `200` | The largest number of items in one list response. Keep it at 10,000 or less: the library refuses longer record lists. |
| `working_copy_idle_seconds` | `900` | Remote mode: how long an unused working copy is kept before it is deleted. |

### 3.6 Example: directory mode

```json
{
  "listen": "127.0.0.1:8080",
  "public_url": "https://records.example.org",
  "data_directory": "/var/lib/ath-host",
  "operator_disclosure": "This host is run by Example Rowing Club on a server the club manages. Records and their backups are stored on that server's disk.",
  "open_enrollment": false,
  "storage": { "kind": "directory" },
  "sign_in": {
    "issuer": "https://sign-in.example.org/realms/athletes",
    "audience": "https://records.example.org",
    "allowed_clients": ["athlete-app", "coach-app"]
  },
  "limits": {
    "body_bytes": 67108864,
    "page_size": 200,
    "working_copy_idle_seconds": 900
  }
}
```

This file names no secret, so it needs no environment variables.

### 3.7 Example: remote mode with an S3-compatible bucket

```json
{
  "listen": "0.0.0.0:8080",
  "public_url": "https://records.example.org",
  "data_directory": "/var/lib/ath-host",
  "operator_disclosure": "This host is run by Example Rowing Club. Records are stored in Turso Cloud databases, and attachment files in a Cloudflare R2 bucket.",
  "open_enrollment": false,
  "storage": {
    "kind": "libsql",
    "url_template": "libsql://{database}-example-org.turso.io",
    "directory_url": "libsql://ath-host-directory-example-org.turso.io",
    "token_env": "ATH_HOST_DATABASE_TOKEN",
    "provision": {
      "kind": "turso",
      "organization": "example-org",
      "group": "athletes",
      "platform_token_env": "ATH_HOST_TURSO_PLATFORM_TOKEN"
    },
    "attachments": {
      "endpoint": "https://ACCOUNT_ID.r2.cloudflarestorage.com",
      "bucket": "athlete-attachments",
      "region": "auto",
      "access_key_env": "ATH_HOST_BUCKET_ACCESS_KEY",
      "secret_key_env": "ATH_HOST_BUCKET_SECRET_KEY",
      "path_style": true
    }
  },
  "sign_in": {
    "issuer": "https://example.authkit.app",
    "audience": "https://records.example.org",
    "jwks_uri": "https://example.authkit.app/oauth2/jwks",
    "allowed_clients": []
  }
}
```

This file needs four environment variables: `ATH_HOST_DATABASE_TOKEN`,
`ATH_HOST_TURSO_PLATFORM_TOKEN`, `ATH_HOST_BUCKET_ACCESS_KEY`, and
`ATH_HOST_BUCKET_SECRET_KEY`. The names are your choice. The file only has to name
them.

## 4. Running on one machine

### 4.1 Build

Install the Rust toolchain named in `rust-toolchain.toml`, then build from the
repository root:

```sh
cargo build --release -p athleticstandard-host
```

The executable is `target/release/ath-host`.

### 4.2 Commands

```text
ath-host serve  --config <file>
ath-host setup  --config <file> --issuer <issuer> --subject <subject> [--name <display name>]
ath-host invite --config <file> [--days <n>]
ath-host version
```

- `serve` runs the host until it receives an interrupt signal (SIGINT, or Ctrl-C).
  It then stops taking connections and finishes open requests. SIGTERM ends it at
  once, without finishing them, so configure your service manager to stop it with
  SIGINT.
- `setup` claims the first owner (section 4.5).
- `invite` makes an invitation code (section 4.6).
- `version` prints `ath-host 2.1.3`.

### 4.3 The data folder

In directory mode the data folder looks like this:

```text
<data>/.ath-host.lock                              held by the running host
<data>/host.sqlite                                 the directory: people, records, invitations, grants, settings
<data>/athletes/<resource-id>/athlete.ath.sqlite   one athlete's database
<data>/athletes/<resource-id>/attachments/         that athlete's attachment files
<data>/staging/uploads/<resource-id>/<upload-id>   uploaded files waiting for a record
<data>/staging/packages/  <data>/staging/exports/  temporary folders for package uploads and exports
```

A resource ID looks like `ath_` followed by 32 hexadecimal characters. The host makes
it when the record is created. It is not the database ID inside the record.

Rules for the data folder:

- **One host process per data folder.** `serve` locks `.ath-host.lock` and refuses
  to start when another host holds it ("another ath-host is using this data
  folder"). SQLite allows one writer per file, and the host orders each athlete's
  writes inside its own process.
- **Run the host as its own system user, and make the data folder readable by that
  user only** (`chmod 700`). Athlete databases are created owner-only. The host
  creates `host.sqlite`, the folders, and staged uploads with the process's default
  permissions. `host.sqlite` holds the key the host uses for receipt digests.
- **Give a real path.** The library refuses a path that passes through a symbolic
  link, and a database in a folder that other local users can write.
- **Directory mode uses one machine.** The plan's routing of athletes between several
  machines (spec section 6.1) is not built.

### 4.4 TLS

`ath-host` speaks plain HTTP. Put it behind a proxy that ends TLS, such as Caddy or
nginx, or behind a platform's HTTPS front end. Set `listen` to `127.0.0.1` or to a
private network address, so nothing reaches the host without TLS. Inside a container,
`0.0.0.0` is right when only the proxy can reach the container's port. Access tokens
and health records cross this connection.

### 4.5 The first owner

The operator claims the first owner once, on the machine (D260). The first owner is
an ordinary athlete with one record. The API has no administrator role.

1. Sign in through the provider as the first owner. Find the subject: the `sub`
   claim of an access token, or the user ID the provider shows for that person.
2. Run `setup` with the issuer exactly as it is in `sign_in.issuer`. The command does
   not compare them, and a different issuer creates an owner who can never sign in.

   ```sh
   ath-host setup --config /etc/ath-host/ath-host.json \
     --issuer https://sign-in.example.org/realms/athletes \
     --subject 8a3c5e2f-0d41-4b7e-9c1a-5f2e7d6b4a90 \
     --name "First athlete"
   ```

3. The command creates the person and an empty record, and prints the record's
   resource ID. `--name` becomes the display name of both. A second `setup` fails with
   "this host already has its first owner".

### 4.6 Invitations

```sh
ath-host invite --config /etc/ath-host/ath-host.json --days 14
```

The command prints a code and the time it expires. A code lasts 7 days unless
`--days` says otherwise, from 1 to 90. It works once. The person signs in through an
app, and the app sends `POST /v1/athletes` with `{"invitation": "<code>"}`. The host
stores only a hash of the code.

In directory mode, `setup` and `invite` use the same data folder as `serve`, and can
run while it serves. In remote mode, both commands delete the working copies in the
data folder they are given. Give them their own data folder there: copy the
configuration file and change `data_directory`.

### 4.7 Running in a container

[`host/Dockerfile`](../host/Dockerfile) builds `ath-host` with the Rust toolchain
named in `rust-toolchain.toml`. The image runs the host as user ID 10001, without
administrator rights. This container definition has not been built or run yet.

Build the image from the repository root:

```sh
docker build -f host/Dockerfile -t ath-host .
```

The image expects:

- the configuration file at `/config/ath-host.json`. To use another path, pass
  `serve --config <path>` after the image name;
- `data_directory` set to `/data`, which is a volume;
- `listen` set to `0.0.0.0:8080`, because connections come from outside the
  container;
- the secrets the file names, as environment variables (`-e NAME` or `--env-file`).

Run it on one machine with a named volume, and publish the port to the machine
itself only. Put a TLS proxy in front (section 4.4).

```sh
docker volume create ath-host-data
docker run -d --name ath-host \
  -v ath-host-data:/data \
  -v /etc/ath-host:/config:ro \
  -p 127.0.0.1:8080:8080 \
  ath-host
```

In directory mode, run `setup` and `invite` inside the running container, so they use
its data folder:

```sh
docker exec ath-host ath-host setup --config /config/ath-host.json \
  --issuer https://sign-in.example.org/realms/athletes --subject <subject>
docker exec ath-host ath-host invite --config /config/ath-host.json
```

In remote mode, run them in a separate container with its own `/data`, because they
delete the working copies in the data folder they use (section 4.6):

```sh
docker run --rm -v /etc/ath-host:/config:ro --env-file /etc/ath-host.env \
  ath-host invite --config /config/ath-host.json
```

Other rules:

- The image stops the host with SIGINT, the signal on which it finishes open requests.
- A folder from the machine mounted at `/data` must belong to user ID 10001, and
  other users must not be able to write it.
- In directory mode, run one container per volume. A second container on the same
  volume refuses to start.
- In remote mode, give each container its own `/data`. Any number of containers can
  share the databases and the bucket.

## 5. Remote mode

Remote mode has run only against a local stand-in in the tests. No libSQL server and
no bucket service has passed the qualification suite yet (D258). Treat the setups
below as untested until a named run passes.

### 5.1 How remote mode works

- **Working copies.** The host keeps a working copy of each active athlete's database
  in `<data>/working/<resource-id>/`. It builds a copy by reading every table from the
  server, and keeps it current from the server's replication log. It deletes every
  copy when it starts, and deletes a copy after `working_copy_idle_seconds` without
  use. It checks for idle copies once a minute.
- **A write** runs through the library on the working copy, which records the rows
  it changed. The host then commits those rows to the server in one transaction.
  The transaction first checks that the server's database revision and access epoch
  still match the copy (D268). If another host copy wrote first, the host refreshes
  the copy and runs the request again, up to three times. The receipt makes a second
  run safe (D261).
- **Foreign keys.** The host refuses to commit when the server does not report
  foreign keys as on for the connection.
- **Several host copies** can serve the same athletes. They share the athletes'
  databases, the directory database, and the bucket. Each needs its own data folder,
  because of the lock in section 4.3, and enough local disk for the working copies of
  the athletes it serves.
- **Uploads wait on local disk.** An uploaded file stays in the `staging/uploads/`
  folder of the host copy that received it until a record names it. Send the upload
  and the request that uses it to the same host copy.
- **Database names.** The database for resource ID `ath_<hex>` is named `ath-<hex>`.

### 5.2 Turso Cloud

[Turso Cloud](https://docs.turso.tech/turso-cloud) hosts libSQL databases. Use
databases on the libSQL engine. Turso-engine databases on Turso Cloud are in early
preview and are not part of the first qualification run (D258).

1. Create a group for the athletes' databases, and create the directory database in
   it by hand, for example `turso db create ath-host-directory --group athletes`.
2. Make a group token with `turso group tokens create athletes`. One group token
   opens every database in the group, including the directory. Put it in the
   variable named by `token_env`.
3. Find the address pattern. `turso db show ath-host-directory --url` prints one
   database's address. Turso documents host names of the form
   `<database>-<organization>.turso.io`. Replace the database name with `{database}`
   to get `url_template`.
4. Make a Platform API token and put it in the variable named by
   `platform_token_env`. For each new record the host sends
   `POST https://api.turso.tech/v1/organizations/{organization}/databases` with the
   database name and group
   ([create a database](https://docs.turso.tech/api-reference/databases/create)). A
   `409` answer, which means the name exists, counts as success.

The Platform API token can create and delete databases in the organization. The
group token can read and write every athlete's database. Keep both only where the
host runs.

Turso's database limits, restore windows, and encryption options are summarized in
section 3.2 of the
[storage study](../build-history/v2.1.3/appendix/hosted-storage-options.md). Turso
Cloud makes `application_id` and `user_version` read-only. The host reads them and
never writes them; the metadata table in the record is authoritative (D93, D258).

### 5.3 A libSQL server you run yourself

The libSQL server, `sqld`, serves many databases as namespaces
([libSQL](https://github.com/tursodatabase/libsql)). Its last release was v0.24.32
on 2025-02-14, and its maintainers point new projects to the Turso engine. It suits
tests and operators who want remote mode without a vendor.

1. Start `sqld` with namespaces (`--enable-namespaces`) and an admin address
   (`--admin-listen-addr`). Require client tokens with `--auth-jwt-key-file`, and put
   a token signed with the matching key in the variable named by `token_env`.
2. Keep the admin address on a private network that only the host can reach. The
   server's documentation describes no authentication for the admin API.
3. The server picks the namespace from the first part of the host name in each
   request. Point a wildcard DNS name such as `*.db.internal` at the server, and set
   `url_template` to `http://{database}.db.internal:8080`. Use `https://` when the
   connection leaves a private network.
4. Create the directory namespace by hand, then set `directory_url` to match:

   ```sh
   curl -X POST http://db-admin.internal:9090/v1/namespaces/ath-host-directory/create
   ```

5. Set `provision` to `{"kind": "sqld_admin", "admin_url": "http://db-admin.internal:9090"}`.
   For each new record the host sends `POST <admin_url>/v1/namespaces/<name>/create`.
   A `409` answer counts as success.

The admin API and namespace addressing are described in the server's
[admin API](https://github.com/tursodatabase/libsql/blob/main/docs/ADMIN_API.md) and
[user guide](https://github.com/tursodatabase/libsql/blob/main/docs/USER_GUIDE.md).

### 5.4 The `existing` provisioner

With `"provision": {"kind": "existing"}` the host creates no database. It writes the
schema into whatever database answers at the address `url_template` gives. The host
makes the resource ID at the moment a record is created, so you cannot create the
database ahead of time. Use `existing` only with a server that creates a database the
first time it is addressed. When the database is missing, creating the record fails
with `internal`, and the directory keeps an entry for a record with no database.

### 5.5 S3-compatible buckets

Remote mode keeps attachment files in one bucket under
`<resource-id>/attachments/<stored name>`. The stored name contains the attachment's
record ID, so two files never share a key. The host reads and writes objects with
signed requests that are valid for 5 minutes. Clients never see the bucket. They
download through the host (section 7.4).

Settings for common services, taken from each service's documentation and not yet
tested with the host:

| Service | `endpoint` | `region` | `path_style` |
|---|---|---|---|
| [Amazon S3](https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html) | `https://s3.<region>.amazonaws.com` | the bucket's region | `false` |
| [Cloudflare R2](https://developers.cloudflare.com/r2/api/s3/api/) | `https://<account-id>.r2.cloudflarestorage.com` | `auto` | `true` |
| [Railway buckets](https://docs.railway.com/storage-buckets) | `https://t3.storageapi.dev` | `auto` | `false`. Older buckets may need `true`; the bucket's Credentials tab says which. |
| [Supabase Storage](https://supabase.com/docs/guides/storage/s3/authentication) | `https://<project-ref>.storage.supabase.co/storage/v1/s3` | the project's region | `true` |
| [MinIO](https://github.com/minio/minio) | your server, such as `http://minio.internal:9000` | `us-east-1` unless you set another | `true` |

Give the host a key that reaches this one bucket only, where the service allows it.
Supabase's S3 keys reach every bucket in the project. Railway buckets and Supabase
Storage keep no earlier versions of an object, so a deleted file is gone unless it
was copied elsewhere (study section 4).

In remote mode the host stores a new file at its final key, then commits the record
that names it. An erasure removes the file after the commit. The plan's staging key
in the bucket, signed download links for clients, and a cleanup job for files that no
record names (D259) are not built. A commit that fails after the upload leaves an
unused object in the bucket.

## 6. Sign-in

### 6.1 What the host checks

Every request except the two discovery routes needs an access token in the
`Authorization: Bearer <token>` header (D260). The host checks, in order:

1. **The signature algorithm.** Only public-key signatures are accepted: RS256,
   RS384, RS512, PS256, PS384, PS512, ES256, ES384, and EdDSA. A token signed with a
   shared secret, such as HS256, is refused.
2. **The signing key.** The host reads the provider's keys from `jwks_uri` and picks
   the one named by the token's `kid`. A token with no `kid` is accepted only when
   the provider publishes one key. Keys are trusted for 10 minutes. An unknown `kid`
   makes the host read the keys again, at most once every 30 seconds.
3. **The claims.** `iss` must equal `issuer`. `aud` must contain `audience`. `exp`
   must be in the future, with 30 seconds of leeway. `sub` must be present.
4. **The app.** The token must name the app it was issued to, in `azp` or, when
   `azp` is absent, `client_id`. When `allowed_clients` is not empty, the app must be
   in it.

A failed check gets `401 sign_in_required`, with a message that says which check
failed.

The host identifies a person by issuer and subject, never by email address. A person
signing in for the first time becomes a new principal. The optional `name` claim
becomes their display name. If you change provider or issuer, every person arrives as
a new principal, and the host has no command to give them their old records.

### 6.2 Apps

Every token names an app. A grant is for one coach through one app (section 8). An
owner has full owner rights through any app the host accepts. The host keeps no list
of the apps an owner has connected, so it cannot disconnect one app. Limit apps with
`allowed_clients`, and end an app's sessions at the provider.

### 6.3 Native apps

Native and command-line apps sign in through the system browser, with the
authorization code flow and PKCE ([RFC 8252](https://www.rfc-editor.org/rfc/rfc8252)).
With PKCE, the app makes a one-time secret, sends a hash of it when sign-in starts,
and sends the secret itself when it collects the token. A stolen authorization code
is then useless to anyone else.

- Register the app with the provider as a public client. It holds no client secret.
- Use a loopback redirect, such as `http://127.0.0.1:<port>/callback`, or a
  private-use URI scheme that belongs to the app.
- Ask for a token for the host's audience: send `resource=<public_url>` where the
  provider supports it, or rely on the provider's audience setting (sections 6.5 and
  6.6).

### 6.4 MCP clients and protected-resource metadata

The host publishes protected-resource metadata
([RFC 9728](https://www.rfc-editor.org/rfc/rfc9728)) at
`/.well-known/oauth-protected-resource`, without sign-in:

```json
{
  "resource": "https://records.example.org",
  "authorization_servers": ["https://sign-in.example.org/realms/athletes"],
  "bearer_methods_supported": ["header"],
  "resource_name": "Athletic Standard host"
}
```

A client that follows RFC 9728 or the
[MCP authorization rules](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization)
reads this document, finds the provider, signs the person in, and asks for a token
with `resource` set to `public_url`. Configure the provider so that such a token
carries `public_url` in `aud`, and set `sign_in.audience` to the same value.

The host serves the document only at the root well-known address. It does not add a
`WWW-Authenticate` header to its 401 answers yet, so clients find the document by its
well-known address. The MCP rules allow either way.

### 6.5 Recipe: WorkOS AuthKit

This recipe comes from WorkOS's documentation. It has not been run against a live
WorkOS account.

| Host field | Value |
|---|---|
| `issuer` | Your AuthKit domain, such as `https://example.authkit.app`. |
| `jwks_uri` | `https://<authkit-domain>/oauth2/jwks`. Set it, because WorkOS documents this address for its keys. |
| `audience` | Your `public_url`. In the WorkOS dashboard, add it as a Resource Indicator under Connect, Configuration, and choose *Set as default*, so a token requested without `resource` still names the host. Without a Resource Indicator, `aud` is the environment's client ID. |
| The app claim | WorkOS access tokens name the app in `client_id`. The host reads it because WorkOS sends no `azp`. |
| `allowed_clients` | The client IDs of the OAuth applications you register, or empty (section 6.2). |

Apps:

- Register each app as an OAuth application under Connect. Mark native apps and
  command-line tools as *Public*. A public application must use PKCE.
- The authorization address is `https://<authkit-domain>/oauth2/authorize`.
- For MCP clients, turn on Client ID Metadata Documents, or Dynamic Client
  Registration for older clients, under Connect, Configuration. These clients arrive
  with client IDs you did not register. Add them to `allowed_clients`, or leave the
  list empty.
- WorkOS also issues tokens for AI agents from the same issuer, marked with
  `sub_profile` set to `ai_agent`. The host does not read `sub_profile`.

WorkOS documentation used:
[access token claims](https://workos.com/docs/authkit/connect/token-claims),
[OAuth applications](https://workos.com/docs/authkit/connect/oauth),
[authorize endpoint](https://workos.com/docs/reference/workos-connect/authorize), and
[MCP](https://workos.com/docs/authkit/mcp).

### 6.6 Recipe: Keycloak

This recipe comes from Keycloak's documentation. It has not been run against a live
Keycloak server.

| Host field | Value |
|---|---|
| `issuer` | `https://<keycloak-host>/realms/<realm>` |
| `jwks_uri` | Leave it out. The host reads it from `<issuer>/.well-known/openid-configuration`. |
| `audience` | Your `public_url`. Keycloak does not add it to `aud` by itself. Create a client scope with an *Audience* mapper, set *Included Custom Audience* to `public_url`, turn on *Add to access token*, and add the scope as a default scope to each app's client. |
| The app claim | Keycloak names the app in `azp`. |
| `allowed_clients` | The client IDs of the apps' clients. |

Other settings:

- Keep the `basic` client scope as a default scope of each client. Since Keycloak 25
  it adds `sub` to access tokens, and the host refuses a token without `sub`. Do not
  turn on lightweight access tokens, which leave `sub` out.
- Keep a public-key signature for access tokens. The realm default, RS256, works.
- For a native app, create a client with *Client authentication* off and *Standard
  flow* on. On its Advanced tab, set *Proof Key for Code Exchange Code Challenge
  Method* to `S256`. Add the loopback redirect, using `127.0.0.1` rather than
  `localhost`, or the app's private-use scheme.
- For MCP clients, Keycloak's guide marks its support for the current MCP
  authorization versions as experimental. Resource Indicators need
  `--features=resource-indicators`. Without that feature Keycloak ignores `resource`,
  and the audience mapper above supplies `aud`. Client ID Metadata Documents need
  `--features=cimd`. Dynamic Client Registration is supported.

Keycloak documentation used:
[OpenID Connect endpoints and native apps](https://www.keycloak.org/securing-apps/oidc-layers),
[MCP authorization server](https://www.keycloak.org/securing-apps/mcp-authz-server),
[Server administration](https://www.keycloak.org/docs/latest/server_admin/index.html),
and the [Keycloak 25 release notes](https://www.keycloak.org/2024/06/keycloak-2500-released)
for the `basic` scope.

## 7. API version 1

### 7.1 Conventions

- Send JSON request bodies unless a route says otherwise. Unknown fields in a JSON
  body are refused with `400 invalid`.
- Every record write except erasure needs an `Idempotency-Key` header of 1 to 200
  characters. A request repeated with the same key and the same body returns the
  original result and writes nothing. The same key with a different body gets
  `409 request_conflict`. Keys are kept per person, per app, and per record (D261).
- Who may call a route:
  - **Anyone**: no token needed.
  - **Signed in**: any accepted token.
  - **Owner**: the person who owns the record, through any accepted app.
  - **Coach with a permission**: a person with an active grant for the record,
    through the app that made the request, that includes the permission.
- A caller who is neither the owner nor a coach with an active grant gets
  `404 not_found`, the same answer as for a record that does not exist. A coach on an
  owner route, and an owner on a coach route, get `403 permission_denied`.

### 7.2 Errors

Every error is JSON with a stable code:

```json
{ "error": { "code": "stale", "message": "…" } }
```

| Status | Code | When |
|---|---|---|
| 400 | `invalid` | The request is malformed: bad JSON, an unknown field, an unknown record type, a missing `Idempotency-Key`, or record data that fails validation. |
| 401 | `sign_in_required` | No token, or a token that fails a check in section 6.1. An expired token gets this code too. |
| 403 | `permission_denied` | The caller is related to the record, but the route or permission is not theirs. Also returned when creating a record without an invitation on a host without open enrollment. |
| 404 | `not_found` | No such record, resource, or code, or nothing the caller may see. |
| 409 | `stale` | An expected revision is out of date, or access to the record changed while a coach write waited. |
| 409 | `request_conflict` | The `Idempotency-Key` was used before with a different body. |
| 409 | `host_moving` | The record is frozen for a move. No route freezes a record yet. |
| 413 | `limit_reached` | Defined, not returned yet. A body larger than `body_bytes` gets status 413 from the web layer, with a plain-text body. |
| 422 | `refused` | The library refused the change, such as a package restored into a record that is not empty, an attachment of the wrong type, or evidence the coach cannot read. |
| 500 | `internal` | The host could not finish. Details go to the host's log, never to the response. |

### 7.3 Discovery and account routes

| Method and path | Who | Request | Response |
|---|---|---|---|
| `GET /.well-known/oauth-protected-resource` | Anyone | None | Protected-resource metadata (section 6.4). |
| `GET /v1/capabilities` | Anyone | None | `api_version` (1), `athleticstandard_version`, `storage` (`directory` or `libsql`), `permissions` (the three grant names), `operator_disclosure`, `open_enrollment`. |
| `GET /v1/me` | Signed in | None | `principal_id`, `client_id`, `coach_id` (the ID written into this person's coach notes), and `athletes`: the resource IDs they own. |
| `POST /v1/athletes` | Signed in | Optional `display_name`, optional `invitation`. An empty body is allowed. | `201`: `athlete_id`, `database_revision`, `operator_disclosure`. Without open enrollment, a valid unused invitation is required. |
| `GET /v1/athletes` | Signed in | None | `athletes`: each record the caller owns, with `athlete_id`, `display_name`, and `state`. |
| `GET /v1/athletes/{id}` | Owner or coach | None | `athlete_id`, `role` (`owner` or `coach`), `state`, `operator_disclosure`, and, for the owner, `database_revision`. |

### 7.4 Owner routes

| Method and path | Request | Response |
|---|---|---|
| `PUT /v1/athletes/{id}/package` | A zip of a complete package folder (the files `ath export` writes, at the root of the zip). Not JSON. No `Idempotency-Key`. | `database_revision`. Only an empty record accepts a package: otherwise `422 refused`. |
| `GET /v1/athletes/{id}/export` | None | The complete package as `application/zip`. The `x-manifest-digest` header is the SHA-256 of `export-manifest.json`. |
| `GET /v1/athletes/{id}/changes?since=<revision>` | `since` is required. | `from`, `to`, `complete`, and `changes`: each with `database_revision`, `record_id`, `record_revision`, and `kind` (`revision`, `deletion`, or `erasure`; an erasure has `record_revision` 0). When `complete` is false, the log does not reach back to `since`; download the export instead. A `since` beyond the current revision gets `409 stale`. |
| `GET /v1/athletes/{id}/records?record_type=<type>&limit=<n>` | Both parameters optional. | `records`: current records, oldest first, at most `limit` (default and maximum `page_size`). There is no next-page cursor in version 1. Use the export or the changes route to read everything. |
| `GET /v1/athletes/{id}/records/{record_id}` | None | `record` and `revisions`, the complete history. |
| `POST /v1/athletes/{id}/records` | `record_type`, `data` (the record's data, as in `ath log` structured input), optional `record_id`, optional `attachments`: upload IDs. `Idempotency-Key` required. | `201`: `database_revision` and `records`, each with `record_id` and `revision`. New attachment records come first, then the record. Attachments can belong only to an athlete report or a nutrition entry. |
| `POST /v1/athletes/{id}/records/{record_id}/corrections` | `expected_revision`, `data` (the complete new data). `Idempotency-Key` required. | `database_revision` and `records`. An out-of-date `expected_revision` gets `409 stale`; keep the person's version for review. |
| `POST /v1/athletes/{id}/records/{record_id}/deletion` | `expected_revision`. `Idempotency-Key` required. | `database_revision` and `records`. |
| `POST /v1/athletes/{id}/records/{record_id}/erasure` | `expected_revision`, `reason_code`: `athlete_request`, `legal_requirement`, `recorded_in_error`, or `other`. No `Idempotency-Key`. | `database_revision`. Erasure runs as in `ath erase` (D107). Repeating it fails, because the record is gone. |
| `POST /v1/athletes/{id}/uploads` | The file's bytes, not JSON: a PNG, JPEG, WebP, or PDF file. | `201`: `upload_id`, `byte_count`, `sha256`, `media_type`. Other file types get `422 refused`. Pass the `upload_id` in `attachments` when creating the record. |
| `GET /v1/athletes/{id}/attachments/{record_id}` | `record_id` is the attachment record's ID. | The file's bytes, with its media type. In remote mode the host fetches the file from the bucket and sends it on. |

### 7.5 Grant routes

| Method and path | Who | Request | Response |
|---|---|---|---|
| `POST /v1/athletes/{id}/grant-offers` | Owner | `permissions` (one or more of `read_workout_summaries`, `add_coach_notes`, `add_programming`), `client_id` (the app the coach must use), optional `coach_label`, optional `valid_seconds`. | `201`: `code`, `expires_at`, `permissions`, `client_id`. An offer lasts 7 days unless `valid_seconds` says otherwise, from 60 seconds to 30 days. |
| `GET /v1/athletes/{id}/grants` | Owner | None | `grants`, revoked ones included. Each has `grant_id`, `resource_id`, `coach_principal_id`, `client_id`, `permissions`, `coach_label`, `created_at`, and `revoked_at`. |
| `DELETE /v1/athletes/{id}/grants/{grant_id}` | Owner | None | `revoked` (the grant ID) and `access_epoch`. A grant that is unknown or already revoked gets `404 not_found`. |
| `POST /v1/grant-offers/accept` | Signed in, through the app the offer names | `code` | `grant`. A code that is unknown, used, expired, for another app, or the caller's own offer gets `404 not_found`. |

### 7.6 Coach routes

| Method and path | Who | Request | Response |
|---|---|---|---|
| `GET /v1/roster` | Signed in | None | `athletes`: each record that granted the caller access through this app, with `athlete_id`, `display_name`, `permissions`, and `grant_id`. |
| `GET /v1/athletes/{id}/workouts?limit=<n>&after=<record_id>` | Coach with `read_workout_summaries` | Both parameters optional. | `workouts`, newest start first, each with exactly `record_id`, `revision`, `start_at`, `end_at`, and `activity_type`. `next` is the `after` value for the next page, or null. |
| `POST /v1/athletes/{id}/notes` | Coach with `add_coach_notes` | `note`, optional `title`, `applies_from`, `applies_to`, and `evidence`: a list of `record_id` and `revision`. `Idempotency-Key` required. | `201`: `database_revision` and `records`. Evidence needs the read permission too, and each item must be the current revision of a workout that is not deleted. |
| `POST /v1/athletes/{id}/programs` | Coach with `add_programming` | `plan`: training plan data. `sessions`: a list of planned session data. Neither may contain `authorship`. A session may not contain `training_plan_id`, `completion_status`, or `completed_workout_session_id`. `Idempotency-Key` required. | `201`: `database_revision` and `records`: the plan, then its sessions. |

The host writes the authorship of notes and programming. The author is
`{"author_type": "coach", "coach": <name>, "coach_id": <coach_id>}` (D255). The name
is the `coach_label` the athlete gave in the offer, or else the coach's `name` claim,
or else `Coach`. Sessions are linked to the new plan and start as `planned`.

### 7.7 Not in version 1 yet

- Moves between hosts (D267). The routes are reserved for build step 8.
- A description of the API generated into `schema/host-api/1/` (D266).
- Signed download links for attachments, and upload expiry and cancellation.

## 8. Grants and revocation

The athlete decides who sees what (D262).

1. The owner makes an offer for one app, with the permissions they choose. No
   permission is chosen by default.
2. The owner gives the code to the coach, outside the host.
3. The coach signs in through the named app and accepts the code. The code works
   once. A coach cannot accept their own offer.
4. The coach sees the athlete in `/v1/roster` for that app.

What each permission allows:

| Permission | Allows | Does not allow |
|---|---|---|
| `read_workout_summaries` | Listing current workouts: record ID, revision, start, end, and activity type. | Routes, descriptions, samples, source IDs, reports, attachments, extensions, full record JSON. |
| `add_coach_notes` | Adding a coaching note with an optional title and dates. Evidence may name only workouts the read permission shows. | Reading notes, correcting, deleting. |
| `add_programming` | Creating a training plan with its planned sessions in one request. | Personal records, completion, links to completed workouts, editing existing plans, deleting. |

The owner keeps every other right: full reads, export, corrections, imports, deletion,
erasure, and grants. A coach cannot correct, delete, or export the athlete's history.
Notes and plans keep their coach author after the grant ends.

**Revocation.** `DELETE /v1/athletes/{id}/grants/{grant_id}` marks the grant revoked
and raises the access epoch, a counter kept inside the athlete's own database. Every
coach request checks the grant first. A coach write also records the epoch it saw,
and commits only if the epoch is still the same, in the same transaction as the
change. So:

- a coach request that starts after the revocation gets `404 not_found`;
- a coach write that was already waiting when the revocation happened gets
  `409 stale` and writes nothing.

Data already sent to the coach cannot be recalled. To change coach, revoke the old
grant and make a new offer. The record does not move (D267).

## 9. Backups and restores

The plan for backups is D264. Build step 8 builds and tests it. Until then, use the
steps below and test a restore yourself.

### 9.1 What to keep

- **Directory mode:** the whole data folder. `host.sqlite` holds the people, records,
  invitations, grants, and the receipt key. Each athlete folder holds a database and
  its attachments.
- **Remote mode:** the directory database, every athlete database on the server, and
  the bucket. The host's data folder holds only working copies and staged uploads.

Keep backups with credentials separate from the host's own (D264).

### 9.2 Directory mode

- **Copy with the host stopped.** Stop the host, copy the data folder, and start it
  again. With the process stopped, a file copy is consistent.
- **Litestream.** The plan copies each athlete database continuously with
  [Litestream](https://litestream.io/guides/directory/). Litestream needs to control
  SQLite's checkpoints, and the library's own checkpoints and restores have not been
  tested with it. The recipe is not published until build step 8 tests it. Do not
  rely on Litestream for these databases before then.
- **Package exports.** The plan adds a scheduled export of each athlete's complete
  package. It is not built. Until then, the owner can download `GET /export`, and an
  operator can run `ath export <folder>` with `ath` 2.1.3 inside an athlete's folder
  (`<data>/athletes/<resource-id>/`). A package opens in any copy of `ath` and needs
  no host.

### 9.3 Remote mode

- **Provider restore.** Use the provider's restore of one database to an earlier
  moment. The host finds an athlete's database only by its name, `ath-<hex>`, so the
  restored database must end up under that name. Check how your provider does this
  before you need it. Then restart every host copy, which deletes the working copies.
  A copy that sees the server at an earlier revision also rebuilds itself.
- **The bucket.** Copy the bucket with the service's own tools. Some services keep no
  earlier versions of an object (section 5.5).
- **Package exports.** The same as directory mode. The scheduled export is not built.

### 9.4 What a restore does not do yet

The plan says a restore covers one athlete, starts isolated, and replays later
erasures and revocations before it serves (D264). None of that is built.

- A restored copy brings back every record erased after the copy was made. Ask the
  athlete to repeat those erasures. In directory mode, run `ath check` in the restored
  athlete's folder.
- Grants live in the directory. Restoring an athlete database does not bring back a
  revoked grant. Restoring an older directory does.
- Without the directory, the host cannot tell who owns which record, and it has no
  command to rebuild that. A new directory also gets a new receipt key, so a request
  retried across the change gets `409 request_conflict` instead of its first
  result.

### 9.5 Leaving a host

The move protocol (D267) is not built. To leave a host today, the owner downloads the
export, creates a record on the new host, and sends the package with
`PUT /v1/athletes/{id}/package`. Changes made on the old host after the export do not
move. Grants do not move; the athlete makes new offers. Coach IDs in old notes give
no access at the new host (D255).

## 10. Security and trust

- **Who can read records.** Whoever runs the host and its storage can read every
  record on it. Keys for each athlete would limit what a copied disk or backup
  reveals. They would not stop someone who controls the running host. The host
  does not offer such keys yet (D264). Say who runs the host and the storage in
  `operator_disclosure`. Apps should show it when a person connects.
- **TLS.** Put TLS in front of the host (section 4.4). Use TLS to the database server
  and the bucket when the connection leaves a private network. Encryption of stored
  data is the storage provider's or the operator's.
- **Credentials.** The configuration file holds no secret (section 3). Clients never
  receive database or bucket credentials (D266). Invitation and offer codes are
  stored only as SHA-256 hashes. The receipt key is made when the host first starts
  and stays in the directory database.
- **Tokens.** The host checks every token on every request. It never stores tokens.
  Short-lived access tokens from the provider limit the harm of a stolen one.
- **Isolation.** Every athlete has a separate database, and a request names a
  resource ID (D256). Owner routes open the database only after the access check.
  Coach routes read the access epoch from the database first and then check the
  grant, so a revocation in between is caught (D262). A caller with no relation to a
  record gets the same `404 not_found` as for a record that does not exist.
- **Logs.** The host writes to standard error. It logs one line when it starts, and
  one line for each internal failure, with the step that failed and the error text.
  It does not log requests, tokens, or request bodies. Error text can include file
  paths.
- **Files.** Keep the data folder owner-only (section 4.3). Staged uploads stay in
  `staging/uploads/` after a record uses them (section 11).

## 11. Limits

| Limit | Value |
|---|---|
| Request body | `body_bytes`, 64 MiB by default. The host holds an uploaded package, and a package it exports, in memory. Size the machine for the largest record. |
| One attachment | 25 MiB, PNG, JPEG, WebP, or PDF. An upload can be larger than this; the record that names it is then refused. |
| Record list | `page_size`, 200 by default, oldest first, with no cursor. |
| Workout summaries | `page_size` per page. The host reads at most 10,000 workouts for one list. |
| `Idempotency-Key` | 1 to 200 characters. |
| Grant offer | 60 seconds to 30 days, 7 days by default. |
| Invitation | 1 to 90 days, 7 days by default. |
| Working copies | Deleted after `working_copy_idle_seconds` without use, 900 by default, and at every start. |
| Retries of a remote write | 3 attempts when another host copy wrote first. |

Not built yet (spec section 6.10): limits on total upload bytes for each athlete, on
job time, on concurrent jobs, and on temporary disk; expiry and cancellation of
uploads; and a limit on how many records one person can create when enrollment is
open. Staged uploads are never removed by the host. Remove files in
`staging/uploads/` that are more than a day old.
