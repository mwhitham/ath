/**
 * Remote libSQL targets.
 *
 * 2.1.0 parsed a URL such as `libsql://host/db` as a remote target and gave it
 * to the libSQL client. Version 2.1.1 keeps that parse and never treats a URL
 * as a local file name, but it opens no network connection: the native
 * library is local-first and adds no remote capability. REMOTE-TARGETS.md
 * lists what each export did and does with a remote URL.
 *
 * The refusals keep the 2.1.0 text:
 *
 * - A scheme the libSQL client did not support failed before any network use
 *   with `URL_SCHEME_NOT_SUPPORTED`. That error is reproduced exactly.
 * - A supported scheme reached the network. Each libSQL request runs on a new
 *   server connection, where SQLite starts with foreign keys off, so the open
 *   check refused with "SQLite foreign-key enforcement could not be enabled".
 *   That refusal is kept; its cause says no connection was attempted.
 */

const SUPPORTED_SCHEMES = new Set(["libsql:", "wss:", "ws:", "https:", "http:"]);

function urlScheme(url: string): string {
  const colon = url.indexOf(":");
  return `${url.slice(0, colon).toLowerCase()}:`;
}

/** The error 2.1.0 returned when it opened a remote target. */
export function remoteTargetRefusal(url: string): Error {
  const scheme = urlScheme(url);
  if (!SUPPORTED_SCHEMES.has(scheme)) {
    const error = new Error(
      `URL_SCHEME_NOT_SUPPORTED: The client supports only "libsql:", "wss:", "ws:", "https:", "http:" and "file:" URLs, got "${scheme}". For more information, please read https://github.com/libsql/libsql-client-ts#supported-urls`,
    );
    error.name = "LibsqlError";
    Object.defineProperty(error, "code", { value: "URL_SCHEME_NOT_SUPPORTED", enumerable: true });
    Object.defineProperty(error, "rawCode", { value: undefined, enumerable: true });
    return error;
  }
  return new Error("SQLite foreign-key enforcement could not be enabled", {
    cause: new Error(
      `athleticstandard 2.1.1 opens local database files only and made no connection to ${url}. ` +
        "In 2.1.0 a remote libSQL target reached this same refusal, because each request ran on a new " +
        "server connection with foreign keys off.",
    ),
  });
}
