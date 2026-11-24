import Foundation

/// A client for version 1 of the `ath-host` API (spec 2.1.3 section 6.5).
///
/// The client sends an OAuth access token with each request. The host checks the
/// token, then applies the athlete's grants. The client never holds database
/// credentials (D266).
///
/// Every method throws `HostError` when the host answers with an error, and
/// `URLError` when the request does not reach the host.
public struct HostClient: Sendable {
    /// The host's address, such as `https://records.example.org`.
    public let baseURL: URL
    private let session: URLSession
    private let accessToken: @Sendable () async throws -> String

    /// A client that asks `accessToken` for a current token before each request. An
    /// app passes a function that refreshes the token when it has expired.
    public init(
        baseURL: URL, session: URLSession = .shared,
        accessToken: @escaping @Sendable () async throws -> String
    ) {
        self.baseURL = baseURL
        self.session = session
        self.accessToken = accessToken
    }

    /// A client with one fixed access token.
    public init(baseURL: URL, token: String, session: URLSession = .shared) {
        self.init(baseURL: baseURL, session: session, accessToken: { token })
    }

    // MARK: - Discovery

    /// The host's resource identifier and its sign-in provider (RFC 9728). An app
    /// reads this before sign-in. It needs no token.
    public func protectedResource() async throws -> ProtectedResource {
        try await send("GET", [".well-known", "oauth-protected-resource"], signedIn: false)
    }

    /// The API version, the storage kind, and who runs the host. It needs no token.
    public func capabilities() async throws -> Capabilities {
        try await send("GET", ["v1", "capabilities"], signedIn: false)
    }

    /// The signed-in person and the records they own.
    public func me() async throws -> Me {
        try await send("GET", ["v1", "me"])
    }

    // MARK: - Owner

    /// Create an empty record owned by the signed-in person. The host refuses this
    /// without an invitation unless it allows open enrollment.
    public func createAthlete(displayName: String? = nil, invitation: String? = nil) async throws
        -> CreatedAthlete
    {
        struct Body: Encodable {
            let displayName: String?
            let invitation: String?
            enum CodingKeys: String, CodingKey {
                case displayName = "display_name"
                case invitation
            }
        }
        return try await send(
            "POST", ["v1", "athletes"], body: Self.json(Body(displayName: displayName, invitation: invitation)))
    }

    /// Restore a complete package, as a zip of the package folder, into the owner's
    /// empty record. Returns the record's database revision.
    public func restorePackage(athleteID: String, zip: Data) async throws -> Int {
        struct Reply: Decodable {
            let databaseRevision: Int
            enum CodingKeys: String, CodingKey { case databaseRevision = "database_revision" }
        }
        let reply: Reply = try await send(
            "PUT", ["v1", "athletes", athleteID, "package"], body: zip, contentType: "application/zip")
        return reply.databaseRevision
    }

    /// Offer a coach access. The athlete gives the returned code to the coach, who
    /// accepts it with the app named by `coachClientID`. Nothing is granted until then.
    public func createGrantOffer(
        athleteID: String, permissions: [Permission], coachClientID: String, coachLabel: String? = nil,
        validSeconds: Int? = nil
    ) async throws -> GrantOffer {
        struct Body: Encodable {
            let permissions: [Permission]
            let clientID: String
            let coachLabel: String?
            let validSeconds: Int?
            enum CodingKeys: String, CodingKey {
                case permissions
                case clientID = "client_id"
                case coachLabel = "coach_label"
                case validSeconds = "valid_seconds"
            }
        }
        let body = Body(
            permissions: permissions, clientID: coachClientID, coachLabel: coachLabel, validSeconds: validSeconds)
        return try await send("POST", ["v1", "athletes", athleteID, "grant-offers"], body: Self.json(body))
    }

    /// Every grant on the owner's record, revoked ones included.
    public func grants(athleteID: String) async throws -> [Grant] {
        struct Reply: Decodable { let grants: [Grant] }
        let reply: Reply = try await send("GET", ["v1", "athletes", athleteID, "grants"])
        return reply.grants
    }

    /// Revoke a grant. The coach's next request about this athlete gets `notFound`,
    /// and a coach write that was waiting fails.
    public func revokeGrant(athleteID: String, grantID: String) async throws -> Revocation {
        try await send("DELETE", ["v1", "athletes", athleteID, "grants", grantID])
    }

    // MARK: - Coach

    /// Accept an athlete's offer with the app this token was issued to. A code works
    /// once. An unknown, used, or expired code, or a code for another app, gives
    /// `notFound`.
    public func acceptGrantOffer(code: String) async throws -> Grant {
        struct Body: Encodable { let code: String }
        struct Reply: Decodable { let grant: Grant }
        let reply: Reply = try await send("POST", ["v1", "grant-offers", "accept"], body: Self.json(Body(code: code)))
        return reply.grant
    }

    /// The athletes who granted the signed-in coach access through this app.
    public func roster() async throws -> [RosterEntry] {
        struct Reply: Decodable { let athletes: [RosterEntry] }
        let reply: Reply = try await send("GET", ["v1", "roster"])
        return reply.athletes
    }

    /// One page of an athlete's workout summaries, newest first. Pass the previous
    /// page's `next` as `after` for the following page. The host caps `limit` at its
    /// page size.
    public func workoutSummaries(athleteID: String, limit: Int? = nil, after: String? = nil) async throws
        -> WorkoutPage
    {
        var query: [URLQueryItem] = []
        if let limit { query.append(URLQueryItem(name: "limit", value: String(limit))) }
        if let after { query.append(URLQueryItem(name: "after", value: after)) }
        return try await send("GET", ["v1", "athletes", athleteID, "workouts"], query: query)
    }

    /// Add a coaching note.
    ///
    /// Make `idempotencyKey` once, when the coach saves the note, for example with
    /// `UUID().uuidString`. Keep it with the note until the host answers. To retry
    /// after a lost reply, send the same note with the same key: the host returns
    /// the first result and writes nothing new. The same key with a different note
    /// is `requestConflict`.
    public func addCoachingNote(_ note: CoachingNote, athleteID: String, idempotencyKey: String) async throws
        -> WriteResult
    {
        try await send(
            "POST", ["v1", "athletes", athleteID, "notes"], body: Self.json(note), idempotencyKey: idempotencyKey)
    }

    // MARK: - Requests

    /// JSON with sorted keys. The host compares the bytes of a retried write with the
    /// first request, so the same note must encode to the same bytes every time.
    static func json(_ value: some Encodable) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(value)
    }

    private func send<Reply: Decodable>(
        _ method: String, _ path: [String], query: [URLQueryItem] = [], body: Data? = nil,
        contentType: String = "application/json", idempotencyKey: String? = nil, signedIn: Bool = true
    ) async throws -> Reply {
        var request = URLRequest(url: try url(path, query: query))
        request.httpMethod = method
        // A cached reply could show data after the athlete revoked a grant.
        request.cachePolicy = .reloadIgnoringLocalCacheData
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if signedIn {
            request.setValue("Bearer \(try await accessToken())", forHTTPHeaderField: "Authorization")
        }
        if let idempotencyKey {
            request.setValue(idempotencyKey, forHTTPHeaderField: "Idempotency-Key")
        }
        if let body {
            request.httpBody = body
            request.setValue(contentType, forHTTPHeaderField: "Content-Type")
        }
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw URLError(.badServerResponse)
        }
        guard (200..<300).contains(http.statusCode) else {
            throw HostError(status: http.statusCode, body: data)
        }
        return try JSONDecoder().decode(Reply.self, from: data)
    }

    /// The base URL's path, then each segment percent-encoded on its own, so an ID
    /// can never add or remove a path segment.
    private func url(_ segments: [String], query: [URLQueryItem]) throws -> URL {
        guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
            throw URLError(.badURL)
        }
        var allowed = CharacterSet.urlPathAllowed
        allowed.remove("/")
        let encoded = segments.map { segment in
            if segment == "." || segment == ".." {
                return segment.replacingOccurrences(of: ".", with: "%2E")
            }
            return segment.addingPercentEncoding(withAllowedCharacters: allowed) ?? segment
        }
        var base = components.percentEncodedPath
        while base.hasSuffix("/") { base.removeLast() }
        components.percentEncodedPath = base + "/" + encoded.joined(separator: "/")
        components.queryItems = query.isEmpty ? nil : query
        guard let url = components.url else {
            throw URLError(.badURL)
        }
        return url
    }
}
