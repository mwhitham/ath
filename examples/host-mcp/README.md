# MCP server example: workouts and coaching notes through a host

`server.mjs` is a small MCP server for a coach. It works with athletes' records on an
`ath-host` server through the npm host client (`connectHost` in `athleticstandard`).
It is an example to copy from. It is not published, and it is not part of the npm
package.

| Tool | Input | What it does |
|---|---|---|
| `list_workout_summaries` | `athlete_id`, optional `limit` | Lists the athlete's workouts, newest first: record ID, revision, start, end, and activity type. |
| `add_coaching_note` | `athlete_id`, `note`, optional `title` | Adds a coaching note. The host records the signed-in coach as its author. |

## How access works

The server sends the signed-in person's access token with every request to the host.
The host checks the token, then that person's grant, on every request. The server
never receives a database credential. A database credential would bypass the grants.

Each grant names an app as well as a person. The token must be issued to the app the
athlete named in the offer (the token's `azp` or `client_id` claim). The same coach
signed in through another app sees nothing.

When the host refuses a request, the tool returns an error result in plain words. An
athlete who never shared a record and an athlete who revoked access get the same
answer. The host answers both with `not_found`, so a coach cannot tell them apart.

Each `add_coaching_note` call sends a new request key (the `Idempotency-Key` header).
If no reply arrives, the server sends the request once more with the same key. The
host then returns the first result instead of writing a second note.

## Run it over stdio

You need Node.js 22 or newer and a running `ath-host`. From the repository root:

```sh
(cd packages/npm && npm ci && npm run build)
(cd examples/host-mcp && npm ci)
```

Add the server to your MCP client's configuration. Most clients accept this shape:

```json
{
  "mcpServers": {
    "athletic-standard-host": {
      "command": "node",
      "args": ["/path/to/ath/examples/host-mcp/server.mjs"],
      "env": {
        "ATH_HOST_URL": "https://records.example.org",
        "ATH_HOST_TOKEN_FILE": "/path/to/coach-access-token"
      }
    }
  }
}
```

| Setting | Meaning |
|---|---|
| `ATH_HOST_URL` | The host's address. |
| `ATH_HOST_TOKEN` | The person's access token for the host. |
| `ATH_HOST_TOKEN_FILE` | Instead of `ATH_HOST_TOKEN`: a file that holds the token. The server reads it before every request, so another program can renew it. |

The MCP authorization specification says a server on stdio takes its credentials from
the environment. It does not run the sign-in flow itself. So the person signs in with
the host's provider in another program, with the app the athlete granted, and asks for
a token for the host. The next section shows where to find the provider.

## Remote MCP clients

This example serves stdio only. A server that MCP clients reach over HTTP (the
Streamable HTTP transport) follows the
[MCP authorization specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization):

1. **Find the provider.** The host publishes protected-resource metadata
   ([RFC 9728](https://www.rfc-editor.org/rfc/rfc9728)) at
   `/.well-known/oauth-protected-resource`. `resource` is the host's address, and
   `authorization_servers` names the sign-in provider:

   ```json
   {
     "resource": "https://records.example.org",
     "authorization_servers": ["https://sign-in.example.org"],
     "bearer_methods_supported": ["header"],
     "resource_name": "Athletic Standard host"
   }
   ```

2. **Publish the server's own metadata.** The MCP server is a protected resource too.
   It serves its own metadata, naming the same provider. Clients find it at the
   server's well-known address, or in the header of a `401` answer:
   `WWW-Authenticate: Bearer resource_metadata="<the server's metadata address>"`.
3. **The MCP client signs the person in.** It reads the server's metadata and then the
   provider's metadata. It uses the authorization code flow with PKCE. It sends
   `resource` set to the MCP server's address
   ([RFC 8707](https://www.rfc-editor.org/rfc/rfc8707)), so the token is issued for the
   MCP server.
4. **The MCP server calls the host with a separate token.** It checks that each token
   it receives was issued for it. It must not pass that token on to the host: the
   specification forbids token passthrough. It uses a second token for the same
   person, issued for the host's `resource`. One way is the provider's token exchange
   ([RFC 8693](https://www.rfc-editor.org/rfc/rfc8693)), where the provider offers it.
5. **The athlete grants the MCP server's app.** The host matches the grant to the app
   the host token was issued to. For a remote server, that is the MCP server's own
   client ID.

## Tests

`npm test` starts a real `ath-host` in directory mode, with a key server and a test
signing key. An athlete grants a coach's app two permissions. An in-memory MCP client
then calls both tools as that coach, and a second client starts `server.mjs` over
stdio. One more test uses a stand-in host that loses the first reply to a note. It
checks that the server sends the note again with the same request key.

The tests use the host built by `cargo build` at the repository root, or the one named
by `ATH_HOST_EXECUTABLE`. They skip with a message when the host or the built npm
package is missing.
