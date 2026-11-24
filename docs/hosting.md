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
| Remote mode: databases on a libSQL server, attachments in an S3-compatible bucket | Built. Tested against a local stand-in for the server and the bucket. A test against a real libSQL server exists but has not been run (section 5.6). |
| Sign-in checks | Built. Tested with a fixed test key. The recipes in section 6 have not been run against live provider accounts. |
| Grants, revocation, coach notes, and programming | Built and covered by tests. |
| Moves between hosts (section 9) | Built and covered by tests between two hosts in directory mode. |
| `ath-host backup` and `ath-host restore` (section 10) | Built and covered by tests. `restore` works in directory mode only. |
| A Litestream recipe | Not published. It has not been run with the library (D264). |
| Clients: the npm host client `connectHost`, a starter app, and an example MCP server | Built. See section 7. |
| A generated description of the API in `schema/host-api/1/` (D266) | Not built. Section 7 is written from `host/src/api.rs` and `host/src/moves.rs`. |

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
ath-host serve   --config <file>
ath-host setup   --config <file> --issuer <issuer> --subject <subject> [--name <display name>]
ath-host invite  --config <file> [--days <n>]
ath-host backup  --config <file> --output <folder>
ath-host restore --config <file> --athlete <athlete id> --package <zip>
ath-host version
```

- `serve` runs the host until it receives an interrupt signal (SIGINT, or Ctrl-C).
  It then stops taking connections and finishes open requests. SIGTERM ends it at
  once, without finishing them, so configure your service manager to stop it with
  SIGINT. When it starts, it deletes what the last process left: working copies,
  staged uploads, and temporary export, package, and restore folders. It also
  finishes any erasure that an earlier run noted but did not complete (section 10.4).
  It then prints the address it listens on, with the real port when `listen` names
  port 0.
- `setup` claims the first owner (section 4.5).
- `invite` makes an invitation code (section 4.6).
- `backup` writes each athlete's complete package as a zip (section 10.2).
- `restore` replaces one athlete's record from such a zip, in directory mode only
  (section 10.3).
- `version` prints `ath-host 2.1.3`.

`setup`, `invite`, and `backup` can run while the host serves. They do not delete the
host's working copies or staged files. In remote mode, give `backup` its own data
folder: copy the configuration file and change `data_directory`. `backup` builds a
working copy of every athlete in its data folder, and two processes must not share
one working copy. `restore` refuses to run while a host serves the data folder.

### 4.3 The data folder

In directory mode the data folder looks like this:

```text
<data>/.ath-host.lock                              held by the running host
<data>/host.sqlite                                 the directory: people, records, invitations, grants, settings, remembered answers, erasures
<data>/athletes/<resource-id>/athlete.ath.sqlite   one athlete's database
<data>/athletes/<resource-id>/attachments/         that athlete's attachment files
<data>/athletes/<resource-id>.replaced-<time>/     the record a restore replaced, kept until you remove it
<data>/staging/uploads/<resource-id>/<upload-id>   uploaded files waiting for a record, for at most one hour
<data>/staging/exports/  packages/  verify/  restore/   temporary folders for exports, package uploads, backup checks, and restores
```

A resource ID is `ath_` followed by 32 lowercase hexadecimal characters. The host makes
it when the record is created. It is not the database ID inside the record.

Rules for the data folder:

- **One host process per data folder.** `serve` locks `.ath-host.lock` and refuses
  to start when another host holds it ("another ath-host is using this data
  folder"). SQLite allows one writer per file, and the host orders each athlete's
  writes inside its own process.
- **Run the host as its own system user.** Each command that reads the configuration
  sets the data folder to owner-only (`0700`), and `host.sqlite` to owner-only
  (`0600`). Athlete databases are
  created owner-only. `host.sqlite` holds the key the host uses for receipt digests.
- **Give a real path.** The library refuses a path that passes through a symbolic
  link, and a database in a folder that other local users can write.
- **Directory mode uses one machine.** The plan's routing of athletes between several
  machines (spec section 6.1) is not built. To spread athletes over machines, run
  separate hosts and move records between them (section 9).

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
itself only. Put a TLS proxy in front (section 4.4). The second mount is a folder for
backups, which must belong to user ID 10001.

```sh
docker volume create ath-host-data
docker run -d --name ath-host \
  -v ath-host-data:/data \
  -v /etc/ath-host:/config:ro \
  -v /srv/ath-host-backups:/backups \
  -p 127.0.0.1:8080:8080 \
  ath-host
```

Run `setup`, `invite`, and, in directory mode, `backup` inside the running container:

```sh
docker exec ath-host ath-host setup --config /config/ath-host.json \
  --issuer https://sign-in.example.org/realms/athletes --subject <subject>
docker exec ath-host ath-host invite --config /config/ath-host.json
docker exec ath-host ath-host backup --config /config/ath-host.json --output /backups
```

In remote mode, run `backup` in a separate container, which gets its own `/data`
(section 4.2):

```sh
docker run --rm -v /etc/ath-host:/config:ro -v /srv/ath-host-backups:/backups \
  --env-file /etc/ath-host.env ath-host backup --config /config/ath-host.json --output /backups
```

To restore one athlete in directory mode, stop the host first:

```sh
docker stop ath-host
docker run --rm -v ath-host-data:/data -v /etc/ath-host:/config:ro -v /srv/ath-host-backups:/backups:ro \
  ath-host restore --config /config/ath-host.json --athlete <athlete id> --package /backups/<athlete id>/<file>.zip
docker start ath-host
```

Other rules:

- The image stops the host with SIGINT, the signal on which it finishes open requests.
- A folder from the machine mounted at `/data` must belong to user ID 10001.
- In directory mode, run one serving container per volume. A second one on the same
  volume refuses to start.
- In remote mode, give each container its own `/data`. Any number of containers can
  share the databases and the bucket.

## 5. Remote mode

Remote mode has run only against a local stand-in in the tests. No libSQL server and
no bucket service has passed the qualification suite yet (D258). Treat the setups
below as untested until a named run passes. Section 5.6 explains how to run the test
against your own server.

### 5.1 How remote mode works

- **Working copies.** The host keeps a working copy of each active athlete's database
  in `<data>/working/<resource-id>/`. It builds a copy by reading every table from the
  server, and keeps it current from the server's replication log. `serve` deletes
  every copy when it starts. A copy is also deleted after `working_copy_idle_seconds`
  without use. The host checks for idle copies once a minute.
- **A write** runs through the library on the working copy, which records the rows
  it changed. The host then commits those rows to the server in one transaction.
  The transaction first checks that the server's database revision and access epoch
  still match the copy (D268). If another host copy wrote first, the host refreshes
  the copy and runs the request again, up to three times. The receipt makes a second
  run safe (D261).
- **Round trips.** New rows go to the server in scripts of up to 500 statements, or
  1 MiB, with one round trip for each script. Updates and deletions go one statement
  at a time, so the host can check that each changed exactly the rows it expected.
- **Foreign keys.** The host refuses to commit when the server does not report
  foreign keys as on for the connection.
- **Several host copies** can serve the same athletes. They share the athletes'
  databases, the directory database, and the bucket. Each needs its own data folder,
  because of the lock in section 4.3, and enough local disk for the working copies of
  the athletes it serves.
- **Uploads wait on local disk.** An uploaded file stays in the `staging/uploads/`
  folder of the host copy that received it, for at most one hour. Send the upload and
  the request that uses it to the same host copy.
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

### 5.6 Testing your own server

`host/tests/host_api.rs` holds a test that runs the host's scenarios against a real
libSQL server: a report with a photo, a coach with grants, programming, a revocation
during a waiting write, and two host copies sharing one record. It then times twenty
report saves and the restore of the demo package. It uses two host copies, a test
sign-in key, and a stand-in bucket in memory, so it tests the server, not your bucket
or provider. It is skipped unless you ask for it.

Set these variables, then run the test:

| Variable | Value |
|---|---|
| `ATH_QUALIFY_LIBSQL_URL_TEMPLATE` | The same value as `url_template`, with `{database}`. |
| `ATH_QUALIFY_LIBSQL_DIRECTORY_URL` | A directory database that exists. Use one made for the test. |
| `ATH_QUALIFY_LIBSQL_TOKEN` | A token that opens every database the test uses. |
| `ATH_QUALIFY_PROVISION` | `sqld-admin:<admin address>`, `turso:<organization>:<group>`, or `existing`. |
| `ATH_QUALIFY_TURSO_PLATFORM_TOKEN` | For `turso:` only: a Platform API token. |

```sh
cargo test -p athleticstandard-host --test host_api -- --ignored --nocapture
```

The test creates new athlete databases on the server each time it runs and does not
delete them. Use a server, organization, or group set aside for testing. A run covers
part of the qualification suite in spec section 8, not all of it. Record the server,
region, crate versions, date, and printed timings with the result (D258).

`host/tests/scale.rs` times a large record in both modes on one machine, with the
local stand-in server for remote mode. It is also skipped unless asked for:

```sh
ATH_SCALE_RECORDS=100000 cargo test --release -p athleticstandard-host --test scale -- --ignored --nocapture
```

## 6. Sign-in

### 6.1 What the host checks

Every request except the discovery routes and the move check between hosts
(section 7.7) needs an access token in the `Authorization: Bearer <token>` header
(D260). The scheme name `Bearer` may be in any case. The host checks, in order:

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
failed. Every 401 answer carries a `WWW-Authenticate` header that names the
protected-resource metadata (section 6.4).

The host identifies a person by issuer and subject, never by email address. A person
signing in for the first time becomes a new principal. The optional `name` claim
becomes their display name. If you change provider or issuer, every person arrives as
a new principal, and the host has no command to give them their old records.

### 6.2 Apps

Every token names an app. A grant is for one coach through one app (section 8).

The host keeps a list of the apps an owner has used with each record (D270). An app
joins the list the first time the owner uses it with that record. The owner can list
the apps, disconnect one, and connect it again from another app (section 7.5). A
disconnected app's next request for that record gets `403 app_disconnected`. The host
refuses to disconnect a record's last connected app, so the owner always keeps a way
in.

`allowed_clients` limits the apps for everyone on the host. To stop an app from
signing a person in at all, end its sessions at the provider too.

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

The host serves the document at the root well-known address. Every 401 answer also
names it, as [RFC 9728](https://www.rfc-editor.org/rfc/rfc9728) section 5.1 describes:

```text
WWW-Authenticate: Bearer resource_metadata="https://records.example.org/.well-known/oauth-protected-resource"
```

The address is built from `public_url`, so set `public_url` to the address clients
really use.

[`examples/host-mcp/`](../examples/host-mcp/) is a small MCP server for coaches that
works through the host API. It runs over standard input and output, and takes the
coach's access token from its environment, as the MCP rules ask of servers on that
transport. It never receives a database credential.

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

The routes are described in OpenAPI 3.1 in
[`schema/host-api/1/openapi.json`](../schema/host-api/1/openapi.json). The description
is generated from the same Rust table the host builds its routes from, so it lists
exactly the routes the host serves (D266, D271). Records and revisions in it point to
the published package schemas in `schema/<version>/`.

JavaScript and TypeScript programs can call these routes through `connectHost` in the
npm package `athleticstandard`. [`examples/host-starter/`](../examples/host-starter/)
runs a coach's steps with it, and [`examples/host-mcp/`](../examples/host-mcp/) is an
MCP server built on it. [`examples/host-swift/`](../examples/host-swift/) is a Swift
client for macOS and iOS apps that runs the same coach steps.

### 7.1 Conventions

- Send JSON request bodies unless a route says otherwise. Each JSON body is checked
  against its shape in the API description before the host reads it. A body that does
  not match, including one with an unknown field or an empty required text, gets
  `400 invalid`.
- An athlete ID in a path is `ath_` followed by 32 lowercase hexadecimal characters.
  Any other value gets `404 not_found` before the host looks anything up.
- Who may call a route:
  - **Anyone**: no token needed.
  - **Signed in**: any accepted token.
  - **Owner**: the person who owns the record, through any accepted app they have not
    disconnected from that record.
  - **Coach with a permission**: a person with an active grant for the record,
    through the app that made the request, that includes the permission.
- A caller who is neither the owner nor a coach with an active grant gets
  `404 not_found`, the same answer as for a record that does not exist. The host
  checks this before it reads the athlete's database, so an unknown record and one
  that is not shared look the same. A coach on an owner route, and an owner on a
  coach route, get `403 permission_denied`.

**Retries.** Send an `Idempotency-Key` header of 1 to 200 characters with every
request that changes something, and send the same key when you repeat it (D261).

- Record writes refuse a request without a key: creating, correcting, and deleting a
  record, adding a coach note, and adding programming. They also keep a receipt inside
  the record.
- For any `POST`, `PUT`, or `DELETE` with a key, a repeat with the same key, method,
  path, and body gets the first answer again, with the same status, and changes
  nothing. The host compares JSON bodies with their keys sorted, so the same request
  with its keys in another order is a repeat. The same key with a different request
  gets `409 request_conflict`.
- The host remembers a key for each person and app across the whole host. Use a new
  key for each new request.
- Only successful answers are remembered. A request that failed can be sent again
  with the same key.
- Two kinds of request are not remembered: creating a grant offer, and the move
  steps. Their answers carry one-time codes, which the host stores only as hashes.
  Sections 7.5 and 7.7 say what a repeat of each one does.

### 7.2 Errors

Every error answer is JSON with a stable code, including errors the web layer makes
itself, such as a malformed query, an unknown route, or a body that is too large:

```json
{ "error": { "code": "stale", "message": "…" } }
```

| Status | Code | When |
|---|---|---|
| 400 | `invalid` | The request is malformed: bad JSON, a malformed query, an unknown field, an unknown record type, a missing `Idempotency-Key`, a workout cursor that did not come from the host, an upload that expired, or record data that fails validation. |
| 401 | `sign_in_required` | No token, or a token that fails a check in section 6.1. An expired token gets this code too. The answer carries a `WWW-Authenticate` header (section 6.4). |
| 403 | `permission_denied` | The caller is related to the record, but the route or permission is not theirs. Also returned when creating a record without an invitation on a host without open enrollment, and when the source host does not confirm a move. |
| 403 | `app_disconnected` | The owner disconnected the app that made the request from this record. Connect it again from another app (section 7.5). |
| 404 | `not_found` | No such record, resource, code, or route, an athlete ID the host could not have issued, or nothing the caller may see. |
| 405 | `invalid` | The route exists, but not for this method. |
| 409 | `stale` | An expected revision is out of date, access to the record changed while a write waited (a grant was revoked or an app disconnected), or a move step came out of order. |
| 409 | `request_conflict` | The `Idempotency-Key` was used before for a different request. |
| 409 | `host_moving` | The record is frozen for a move, or staged as an incoming move, and takes no writes. |
| 413 | `limit_reached` | The request body is larger than `body_bytes`. |
| 422 | `refused` | The library refused the change, such as a package restored into a record that is not empty, an attachment of the wrong type, or evidence the coach cannot read. |
| 500 | `internal` | The host could not finish. Details go to the host's log, never to the response. |

### 7.3 Discovery and account routes

| Method and path | Who | Request | Response |
|---|---|---|---|
| `GET /.well-known/oauth-protected-resource` | Anyone | None | Protected-resource metadata (section 6.4). |
| `GET /v1/capabilities` | Anyone | None | `api_version` (1), `athleticstandard_version`, `storage` (`directory` or `libsql`), `permissions` (the three grant names), `operator_disclosure`, `open_enrollment`. |
| `GET /v1/me` | Signed in | None | `principal_id`, `client_id`, `coach_id` (the ID written into this person's coach notes), and `athletes`: the resource IDs they own, leaving out records this app was disconnected from. |
| `POST /v1/athletes` | Signed in | Optional `display_name`, optional `invitation`, optional `incoming`. An empty body is allowed. | `201`: `athlete_id`, `database_revision`, `operator_disclosure`. Without open enrollment, a valid unused invitation is required. With `"incoming": true` the record is created `staged`, for a record arriving by a move (section 9). |
| `GET /v1/athletes` | Signed in | None | `athletes`: each record the caller owns, with `athlete_id`, `display_name`, and `state` (`active`, `frozen`, or `staged`). Retired records, and records this app was disconnected from, are not listed. |
| `GET /v1/athletes/{id}` | Owner or coach | None | `athlete_id`, `role` (`owner` or `coach`), `state`, `operator_disclosure`, and, for the owner, `database_revision`. |

### 7.4 Owner routes

While a record is frozen or staged for a move, every write below gets
`409 host_moving`. Reads still work. A staged record accepts only its package.

| Method and path | Request | Response |
|---|---|---|
| `PUT /v1/athletes/{id}/package` | A zip of a complete package folder (the files `ath export` writes, at the root of the zip). Not JSON. | `database_revision`. Only an empty record accepts a package: otherwise `422 refused`. |
| `GET /v1/athletes/{id}/export` | None | The complete package as `application/zip`. The `x-manifest-digest` header is the SHA-256 of `export-manifest.json`. |
| `GET /v1/athletes/{id}/changes?since=<revision>` | `since` is required. | `from`, `to`, `complete`, and `changes`: each with `database_revision`, `record_id`, `record_revision`, and `kind` (`revision`, `deletion`, or `erasure`; an erasure has `record_revision` 0). When `complete` is false, the log does not reach back to `since`; download the export instead. A `since` beyond the current revision gets `409 stale`. |
| `GET /v1/athletes/{id}/records?record_type=<type>&limit=<n>` | Both parameters optional. | `records`: current records, oldest first, at most `limit` (default and maximum `page_size`). There is no next-page cursor in version 1. Use the export or the changes route to read everything. |
| `GET /v1/athletes/{id}/records/{record_id}` | None | `record` and `revisions`, the complete history. |
| `POST /v1/athletes/{id}/records` | `record_type`, `data` (the record's data, as in `ath log` structured input), optional `record_id`, optional `attachments`: upload IDs. `Idempotency-Key` required. | `201`: `database_revision` and `records`, each with `record_id` and `revision`. New attachment records come first, then the record. Attachments can belong only to an athlete report or a nutrition entry. |
| `POST /v1/athletes/{id}/records/{record_id}/corrections` | `expected_revision`, `data` (the complete new data). `Idempotency-Key` required. | `database_revision` and `records`. An out-of-date `expected_revision` gets `409 stale`; keep the person's version for review. |
| `POST /v1/athletes/{id}/records/{record_id}/deletion` | `expected_revision`. `Idempotency-Key` required. | `database_revision` and `records`. |
| `POST /v1/athletes/{id}/records/{record_id}/erasure` | `expected_revision`, `reason_code`: `athlete_request`, `legal_requirement`, `recorded_in_error`, or `other`. | `database_revision`. Erasure runs as in `ath erase` (D107). The host notes the erasure in its directory before it applies it (section 10.4). Without a key, a repeat fails, because the record is gone. |
| `POST /v1/athletes/{id}/uploads` | The file's bytes, not JSON: a PNG, JPEG, WebP, or PDF file. | `201`: `upload_id`, `byte_count`, `sha256`, `media_type`. Other file types get `422 refused`. Pass the `upload_id` in `attachments` when creating the record, within one hour. |
| `GET /v1/athletes/{id}/attachments/{record_id}` | `record_id` is the attachment record's ID. | The file's bytes, with its media type. In remote mode the host fetches the file from the bucket and sends it on. |

### 7.5 Grant and app routes

| Method and path | Who | Request | Response |
|---|---|---|---|
| `POST /v1/athletes/{id}/grant-offers` | Owner | `permissions` (one or more of `read_workout_summaries`, `add_coach_notes`, `add_programming`), `client_id` (the app the coach must use), optional `coach_label`, optional `valid_seconds`. | `201`: `code`, `expires_at`, `permissions`, `client_id`. An offer lasts 7 days unless `valid_seconds` says otherwise, from 60 seconds to 30 days. A repeat makes a second offer; cancel the one you do not use. |
| `GET /v1/athletes/{id}/grant-offers/pending` | Owner | None | `offers`: each offer that nobody accepted and that has not expired, with `offer_id`, `permissions`, `client_id`, `coach_label`, `created_at`, and `expires_at`. The code is never shown again. |
| `DELETE /v1/athletes/{id}/grant-offers/{offer_id}` | Owner | None | `cancelled` (the offer ID). An offer that is unknown, already accepted, or for another record gets `404 not_found`. |
| `GET /v1/athletes/{id}/grants` | Owner | None | `grants`, revoked ones included. Each has `grant_id`, `resource_id`, `coach_principal_id`, `client_id`, `permissions`, `coach_label`, `created_at`, and `revoked_at`. |
| `DELETE /v1/athletes/{id}/grants/{grant_id}` | Owner | None | `revoked` (the grant ID) and `access_epoch`. A grant that is unknown or already revoked gets `404 not_found`. |
| `GET /v1/athletes/{id}/apps` | Owner | None | `apps`: each app the owner has used with this record, with `client_id`, `state` (`connected` or `disconnected`), `connected_at`, `last_used_at` (updated at most once an hour), `disconnected_at`, and `this_app` (true for the app that asked). Coach apps are not listed here; they appear in the grants. |
| `DELETE /v1/athletes/{id}/apps/{client_id}` | Owner | None. Percent-encode the client ID. | `app` and `access_epoch`. The app's next request for this record gets `403 app_disconnected`, and an owner write from it that is already waiting fails. Disconnecting the record's last connected app gets `422 refused`. A repeat gives the same answer. An app this record never used gets `404 not_found`. |
| `PUT /v1/athletes/{id}/apps/{client_id}` | Owner, from a connected app | None | `app`, connected again. An app this record never used gets `404 not_found`. A disconnected app cannot connect itself: it gets `403 app_disconnected`. |
| `POST /v1/grant-offers/accept` | Signed in, through the app the offer names | `code` | `grant`. A code that is unknown, used, cancelled, expired, for another app, or the caller's own offer gets `404 not_found`. |

### 7.6 Coach routes

| Method and path | Who | Request | Response |
|---|---|---|---|
| `GET /v1/roster` | Signed in | None | `athletes`: each record that granted the caller access through this app, with `athlete_id`, `display_name`, `permissions`, and `grant_id`. |
| `GET /v1/athletes/{id}/workouts?limit=<n>&after=<next>` | Coach with `read_workout_summaries` | Both parameters optional. `after` is a `next` value from an earlier page. | `workouts`, newest start first, each with exactly `record_id`, `revision`, `start_at`, `end_at`, and `activity_type`. `next` is the value to send as `after` for the next page, or null on the last page. Treat it as opaque. A workout deleted between pages does not end the list early. |
| `POST /v1/athletes/{id}/notes` | Coach with `add_coach_notes` | `note`, optional `title`, `applies_from`, `applies_to`, and `evidence`: a list of `record_id` and `revision`. `Idempotency-Key` required. | `201`: `database_revision` and `records`. Evidence needs the read permission too, and each item must be the current revision of a workout that is not deleted. |
| `POST /v1/athletes/{id}/programs` | Coach with `add_programming` | `plan`: training plan data. `sessions`: a list of planned session data. Neither may contain `authorship`. A session may not contain `training_plan_id`, `completion_status`, or `completed_workout_session_id`. `Idempotency-Key` required. | `201`: `database_revision` and `records`: the plan, then its sessions. |

The host writes the authorship of notes and programming. The author is
`{"author_type": "coach", "coach": <name>, "coach_id": <coach_id>}` (D255). The name
is the `coach_label` the athlete gave in the offer, or else the coach's `name` claim,
or else `Coach`. Sessions are linked to the new plan and start as `planned`.

### 7.7 Move routes

Section 9 explains the order of the steps.

| Method and path | Who | Request | Response |
|---|---|---|---|
| `POST /v1/athletes/{id}/move/freeze` | Owner, on the source | `destination`: the destination host's `public_url`. | `transfer_id`, `manifest_digest`: the SHA-256 of the final package's manifest. The record stops taking writes. A repeat returns the same transfer. A record that is not active gets `409 host_moving`. |
| `POST /v1/athletes/{id}/move/abort` | Owner, on the source | `transfer_id` | `state`: `active`. The record takes writes again. After `retire`, `409 stale`. |
| `POST /v1/athletes/{id}/move/retire` | Owner, on the source | `transfer_id` | `transfer_id`, `activation` (a single-use code), `manifest_digest`. The source record is retired. A repeat issues a new code, and only the newest code works. |
| `POST /v1/athletes/{id}/move/activate` | Owner, on the destination | `source`: the source host's `public_url`, `transfer_id`, `activation`. | `state` (`active`) and `manifest_digest`. The destination asks the source to confirm the code for its own `public_url` and the digest of the package it holds. Without that, `403 permission_denied`. A record that is not staged gets `409 stale`. |
| `POST /v1/transfers/{transfer_id}/verify` | Anyone who holds the code; the destination host calls it | `activation`, `destination`, `manifest_digest` | `verified`: `true`. The code works once. Anything that does not match gets `404 not_found`. |
| `GET /v1/athletes/{id}/move/recovery-copy` | Owner, on the source, after `retire` | None | The retired record's package as `application/zip`, with `x-manifest-digest`. A record that is not retired gets `409 stale`. |

### 7.8 Not in version 1 yet

- Signed download links for attachments, and cancelling an upload.
- A label for a record restored from a device copy when the source cannot be reached,
  which the plan calls a recovered fork (D267).
- A way to remove a retired record from the source host.

## 8. Grants and revocation

The athlete decides who sees what (D262).

1. The owner makes an offer for one app, with the permissions they choose. No
   permission is chosen by default.
2. The owner gives the code to the coach, outside the host.
3. The coach signs in through the named app and accepts the code. The code works
   once. A coach cannot accept their own offer.
4. The coach sees the athlete in `/v1/roster` for that app.

The owner can list the offers nobody has accepted yet and cancel one (section 7.5). The
code itself is shown only once, when the offer is made.

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

## 9. Moving a record to another host

A move has one host that accepts writes at every step (D267). The owner's app drives
it, with a token for each host. The steps are the same in both modes. The tests move
a record between two hosts in directory mode, and move a record with a photo from
directory mode to remote mode and back.

1. **Freeze the source.** Send `POST /v1/athletes/{id}/move/freeze` to the source,
   with `destination` set to the destination's `public_url`. The source stops taking
   writes and returns `transfer_id` and `manifest_digest`. Reads still work, so
   coaches can still read workout summaries.
2. **Copy the package.** Download `GET /v1/athletes/{id}/export` from the source. Its
   `x-manifest-digest` header equals the digest from step 1. On the destination,
   create a record with `POST /v1/athletes` and `{"incoming": true}`, with an
   invitation unless the destination has open enrollment. Then send the package with
   `PUT /v1/athletes/{new id}/package`. The new record is staged: it
   takes no other write, and no coach can see it.
3. **Retire the source.** Send `POST /v1/athletes/{id}/move/retire` to the source with
   the transfer ID. The source retires its writer and returns a single-use
   `activation` code. From now on the source cannot reopen by itself. If the reply is
   lost, call `retire` again. It issues a new code, and only the newest code works.
4. **Activate the destination.** Send `POST /v1/athletes/{new id}/move/activate` to
   the destination with `source` (the source's `public_url`), `transfer_id`, and
   `activation`. The destination asks the source to confirm the code, its own
   `public_url`, and the digest of the package it holds. The source confirms once,
   and only when all three match. The destination then takes writes.
5. **Reconnect.** Apps and devices switch to the destination and the new athlete ID.
   Grants do not move, so the owner makes new offers. Coach IDs in old notes give no
   access at the new host (D255).

Before step 3, the owner can stop the move with `POST /v1/athletes/{id}/move/abort`,
and the source takes writes again. After step 3, only activation takes the record
forward.

What a move needs and leaves behind:

- In step 4 the destination calls the `source` address, so it must be able to reach
  it over the network. The `destination` sent in step 1 must equal the destination's
  `public_url`. Trailing slashes are ignored; nothing else may differ.
- The records, their revisions, and their attachments arrive unchanged. The
  destination's package has the same manifest digest as the source's.
- The source keeps the retired record. The owner can download it with
  `GET /v1/athletes/{id}/move/recovery-copy`. The host has no command to remove a
  retired record yet. After the owner confirms the move, an operator can remove the
  record's folder in directory mode, or its database in remote mode.
- If the source cannot be reached, a move cannot finish, because the destination
  cannot get the confirmation. The owner can instead create an ordinary record on the
  new host and send a package they hold: an export, a backup, or a device copy. The
  host does not label such a record as a recovered fork yet (D267). If the old host
  comes back, it may still take writes for the old record, so tell the people who use
  it.

## 10. Backups and restores

The plan for backups and restores is D264.

### 10.1 What to keep

- **Directory mode:** the whole data folder. `host.sqlite` holds the people, records,
  invitations, grants, remembered answers, erasure notes, and the receipt key. Each
  athlete folder holds a database and its attachments.
- **Remote mode:** the directory database, every athlete database on the server, and
  the bucket. The host's data folder holds only working copies and staged uploads.

Keep backups where the host's own credentials cannot change them (D264). For example,
copy the backup folder to storage that only the backup job can write.

A file copy of a whole directory-mode data folder is consistent only when the host is
stopped.

### 10.2 `ath-host backup`

```sh
ath-host backup --config /etc/ath-host/ath-host.json --output /srv/ath-host-backups
```

- For each record that is active or frozen, the command exports the complete package.
  It restores the package into an empty database in `staging/verify/` and runs the
  database check. Only a package that passes is kept.
- It writes `<output>/<athlete id>/<UTC time>-<first 12 characters of the digest>.zip`
  and prints one line for each athlete: the athlete ID, the manifest digest, and the
  file.
- It works in both modes. In remote mode it reads attachments from the bucket, so it
  needs the same secrets as the host. Give it its own data folder there (section 4.2).
- Staged and retired records are skipped.
- Run it on a schedule, such as a daily cron job or systemd timer. It never deletes an
  older backup.
- Each zip is a complete health record, so the command writes each file readable by
  its owner only. Keep the output folder owner-only too.
- Unzipped, a backup is an ordinary package folder. `ath import` restores it into an
  empty database, and `PUT /v1/athletes/{id}/package` restores it on any host.

### 10.3 `ath-host restore` (directory mode)

```sh
ath-host restore --config /etc/ath-host/ath-host.json \
  --athlete ath_0123456789abcdef0123456789abcdef \
  --package /srv/ath-host-backups/ath_0123456789abcdef0123456789abcdef/<file>.zip
```

1. Stop the host first. The command refuses while a host serves the data folder
   ("stop ath-host before restoring: it is using this data folder").
2. The command restores the package into a new folder, then puts it in place of the
   athlete's folder. The replaced record stays beside it as
   `athletes/<id>.replaced-<time>/` until you remove it.
3. It applies again every erasure ever noted for that athlete, oldest first
   (section 10.4). A record erased after the backup was made stays erased.
4. It raises the access epoch, so a coach write that started before the restore
   cannot commit after it (D262).
5. It prints the database revision the record is now at.

Restoring one athlete changes no other athlete. Grants live in the directory, so a
restore neither brings back a revoked grant nor removes a newer one. In remote mode the
command refuses; see section 10.5.

### 10.4 Erasures

- Before the host applies an erasure, it notes it in the directory. When the erasure
  is done, it marks the note complete. A note holds the athlete ID, the record ID, the
  reason, and the times. It holds no record values.
- If the host stops in between, `serve` finishes the erasure when it next starts. It
  logs "finished N erasure(s) that an earlier run did not complete".
- `ath-host restore` applies every noted erasure for the athlete again.
- An erasure cannot remove copies in backups that were made before it. A restore from
  such a backup removes the record again, as long as the directory still holds the
  note.

### 10.5 Remote mode restores

- **Provider restore.** Use the provider's restore of one database to an earlier
  moment. The host finds an athlete's database only by its name, `ath-<hex>`, so the
  restored database must end up under that name. Check how your provider does this
  before you need it.
- **Erasures.** The host does not apply erasures again after a provider restore by
  itself. Stop every host copy. In the directory database, mark the athlete's erasure
  notes as unfinished. Start one host and wait for its log line about finished
  erasures, then start the others. A host applies unfinished notes when it starts,
  and starting also deletes the old working copies. This procedure follows from the
  code and has not been run.

  ```sql
  UPDATE erasure_intents SET completed_at = NULL
  WHERE resource_id = 'ath_0123456789abcdef0123456789abcdef';
  ```

- **The bucket.** Copy the bucket with the service's own tools. Some services keep no
  earlier versions of an object (section 5.5).
- **Package backups.** `ath-host backup` works in remote mode (section 10.2).

### 10.6 Litestream

The plan copies each directory-mode athlete database continuously with
[Litestream](https://litestream.io/guides/directory/) (D264). The recipe is not
published, because it has not been run with the library. The host opens an athlete's
database for each request, and the library runs a truncating checkpoint when it
closes one. Litestream needs to control checkpoints itself. The plan asks the host to
keep connections open while Litestream runs (spec section 6.8). That is not built. Do
not rely on Litestream for these databases yet.

### 10.7 What a restore does not cover

- `ath-host backup` saves athletes' packages only, not the directory. Without the
  directory, the host cannot tell who owns which record, and it has no command to
  rebuild that. Back up the directory as well: in directory mode, copy `host.sqlite`
  with the host stopped, or with SQLite's `.backup` command while it runs; in remote
  mode, use the provider's backup of the directory database.
- A new directory also gets a new receipt key. A request retried across that change
  gets `409 request_conflict` instead of its first answer.
- The host has no sessions of its own to end after a restore. Sign-in sessions belong
  to the provider.

## 11. Security and trust

- **Who can read records.** Whoever runs the host and its storage can read every
  record on it. Keys for each athlete would limit what a copied disk or backup
  reveals. They would not stop someone who controls the running host. The host
  does not offer such keys yet (D264). Say who runs the host and the storage in
  `operator_disclosure`. Apps should show it when a person connects.
- **TLS.** Put TLS in front of the host (section 4.4). Use TLS to the database server
  and the bucket when the connection leaves a private network. Encryption of stored
  data is the storage provider's or the operator's.
- **Credentials.** The configuration file holds no secret (section 3). Clients never
  receive database or bucket credentials (D266). Invitation codes, offer codes, and
  move activation codes are stored only as SHA-256 hashes. The receipt key is made
  when the host first starts and stays in the directory database.
- **Tokens.** The host checks every token on every request. It never stores tokens.
  Short-lived access tokens from the provider limit the harm of a stolen one.
- **Isolation.** Every athlete has a separate database, and a request names an
  athlete ID (D256). The host refuses an ID it could not have issued before the ID
  reaches a file path or a database address. It checks the caller's relation to the
  athlete before it reads the athlete's database. Coach routes then read the access
  epoch and check the grant again, so a revocation in between is caught (D262). A
  caller with no relation to a record gets the same `404 not_found` as for a record
  that does not exist.
- **Remembered answers.** The directory keeps the answer to each request sent with an
  `Idempotency-Key` (section 7.1). Answers hold IDs, revisions, and grant details, not
  record values.
- **Logs.** The host writes to standard error. It logs the address it listens on when
  it starts, and one line for each internal failure, with the step that failed and the
  error text. It does not log requests, tokens, or request bodies. Error text can
  include file paths.
- **Files.** The host keeps the data folder at `0700` and `host.sqlite` at `0600`
  (section 4.3). Backup files are owner-only (section 10.2).
- **Apps.** An owner has full owner rights through any app the host accepts, and the
  host cannot disconnect one app (section 6.2).

## 12. Limits

| Limit | Value |
|---|---|
| Request body | `body_bytes`, 64 MiB by default. The host holds an uploaded package, and a package it exports, in memory. Size the machine for the largest record. |
| One attachment | 25 MiB, PNG, JPEG, WebP, or PDF. An upload can be larger than this; the record that names it is then refused. |
| An upload | Removed one hour after it arrives, used or not. `serve` also removes every upload when it starts. |
| Record list | `page_size`, 200 by default, oldest first, with no cursor. |
| Workout summaries | `page_size` per page. The host reads at most 10,000 workouts for one list. |
| `Idempotency-Key` | 1 to 200 characters. The host keeps remembered answers and has no cleanup for them yet. |
| Grant offer | 60 seconds to 30 days, 7 days by default. |
| Invitation | 1 to 90 days, 7 days by default. |
| Working copies | Deleted after `working_copy_idle_seconds` without use, 900 by default, and when `serve` starts. |
| Retries of a remote write | 3 attempts when another host copy wrote first. |
| Remote commits | New rows in scripts of up to 500 statements or 1 MiB each. |

Not built yet (spec section 6.10): limits on total upload bytes for each athlete, on
job time, on concurrent jobs, and on temporary disk; cancelling an upload; and a limit
on how many records one person can create when enrollment is open.

`host/tests/faults.rs` checks that a large job for one athlete does not delay a small
write for another, in both modes, and that a failed remote commit leaves the server
unchanged. These run with every `cargo test`.
