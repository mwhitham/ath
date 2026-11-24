# Host starter: a coach's steps

`coach.mjs` is a small Node.js program that works with a record on an `ath-host`
server through the npm host client (`connectHost` in `athleticstandard`). It is an
example to copy from. It is not published.

It does what a coach's app does:

1. It shows who runs the host. An app shows this before a person connects.
2. It accepts an athlete's offer, when you give it the code.
3. It lists the athletes who shared a record with this app.
4. It reads one athlete's latest workout summaries.
5. It adds a note that cites the latest workout, when you give it the text.

The program never receives a database credential. It sends the coach's access token,
and the host checks that coach's grant on every request.

## Run it

You need Node.js 22 or newer, a running `ath-host`, and an access token for the coach.
The token must be issued to the app the athlete named in the offer. The host matches
grants to the app as well as the person.

Build the npm package once, from the repository root:

```sh
(cd packages/npm && npm ci && npm run build)
```

Then run the program:

```sh
ATH_HOST_URL=https://records.example.org \
ATH_HOST_TOKEN="$COACH_TOKEN" \
ATH_GRANT_CODE=<code from the athlete> \
ATH_NOTE="Keep the easy runs easy this week." \
node examples/host-starter/coach.mjs
```

| Setting | Required | Meaning |
|---|---|---|
| `ATH_HOST_URL` | yes | The host's address. |
| `ATH_HOST_TOKEN` | yes | The coach's access token from the host's sign-in provider. |
| `ATH_GRANT_CODE` | no | An athlete's offer code. The program accepts it first. A code works once. |
| `ATH_ATHLETE_ID` | no | The athlete to work with. The first athlete on the roster is used otherwise. |
| `ATH_NOTE` | no | The note to add. Without it, the program only reads. |
| `ATH_NOTE_KEY` | no | The request key for the note. |

## Retries

Every write carries a request key (the `Idempotency-Key` header). The program prints
the note's key. If the reply is lost, run the program again with the same
`ATH_NOTE_KEY` and the same note. The host returns the first result and adds nothing.
The same key with a different note is refused as a conflict.

## Errors

A refused step prints the host's message and its stable code, such as `not_found` or
`permission_denied`. After an athlete revokes access, the next request answers
`not_found`. The host gives the same answer for a record that does not exist, so a
coach cannot tell the two apart.

## Try it on one machine

`packages/npm/test/host.test.ts` runs this program against a real `ath-host`. It
starts a key server with a test signing key, starts the host in directory mode, and
signs a token for each person. Read it to see a complete local setup.
