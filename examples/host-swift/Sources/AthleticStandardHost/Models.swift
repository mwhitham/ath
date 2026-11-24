// The JSON shapes of host API version 1, as `host/src/api.rs` writes them.
// Every type names its wire fields in `CodingKeys`, so a reader can match them to the
// host without knowing a key strategy.

import Foundation

/// One of the three things an athlete can grant a coach (D262). It is a string
/// wrapper, not an enum, so a newer host's permission does not break decoding.
public struct Permission: RawRepresentable, Codable, Hashable, Sendable, CustomStringConvertible {
    public let rawValue: String

    public init(rawValue: String) {
        self.rawValue = rawValue
    }

    public init(from decoder: any Decoder) throws {
        rawValue = try decoder.singleValueContainer().decode(String.self)
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }

    public var description: String { rawValue }

    /// List workouts: record ID, revision, start, end, and activity type. Nothing else.
    public static let readWorkoutSummaries = Permission(rawValue: "read_workout_summaries")
    /// Add a coaching note. Evidence can name only workouts the coach can read.
    public static let addCoachNotes = Permission(rawValue: "add_coach_notes")
    /// Add a training plan and its planned sessions.
    public static let addProgramming = Permission(rawValue: "add_programming")
}

/// `GET /v1/capabilities`: what this host offers.
public struct Capabilities: Decodable, Sendable {
    public let apiVersion: Int
    public let athleticStandardVersion: String
    /// `directory` or `libsql`.
    public let storage: String
    public let permissions: [Permission]
    /// Who runs the host and its storage. Show it to an athlete before they connect.
    public let operatorDisclosure: String
    public let openEnrollment: Bool

    enum CodingKeys: String, CodingKey {
        case apiVersion = "api_version"
        case athleticStandardVersion = "athleticstandard_version"
        case storage
        case permissions
        case operatorDisclosure = "operator_disclosure"
        case openEnrollment = "open_enrollment"
    }
}

/// `GET /.well-known/oauth-protected-resource` (RFC 9728): the host's resource
/// identifier and the sign-in provider that issues tokens for it.
public struct ProtectedResource: Decodable, Sendable {
    /// Ask the provider for a token with this audience.
    public let resource: String
    public let authorizationServers: [String]

    enum CodingKeys: String, CodingKey {
        case resource
        case authorizationServers = "authorization_servers"
    }
}

/// `GET /v1/me`: the signed-in person as the host knows them.
public struct Me: Decodable, Sendable {
    public let principalID: String
    /// The app the token was issued to.
    public let clientID: String
    /// The opaque ID the host writes into coach authorship (D255).
    public let coachID: String
    /// Records this person owns.
    public let athletes: [String]

    enum CodingKeys: String, CodingKey {
        case principalID = "principal_id"
        case clientID = "client_id"
        case coachID = "coach_id"
        case athletes
    }
}

/// `POST /v1/athletes`: a new, empty record owned by the caller.
public struct CreatedAthlete: Decodable, Sendable {
    public let athleteID: String
    public let databaseRevision: Int
    public let operatorDisclosure: String

    enum CodingKeys: String, CodingKey {
        case athleteID = "athlete_id"
        case databaseRevision = "database_revision"
        case operatorDisclosure = "operator_disclosure"
    }
}

/// `POST /v1/athletes/{id}/grant-offers`: a one-time code the athlete gives a coach.
public struct GrantOffer: Decodable, Sendable {
    public let code: String
    public let expiresAt: String
    public let permissions: [Permission]
    /// The coach app that can accept the offer.
    public let clientID: String

    enum CodingKeys: String, CodingKey {
        case code
        case expiresAt = "expires_at"
        case permissions
        case clientID = "client_id"
    }
}

/// A grant: one coach, through one app, for one athlete.
public struct Grant: Decodable, Sendable {
    public let grantID: String
    public let athleteID: String
    public let coachPrincipalID: String
    public let clientID: String
    public let permissions: [Permission]
    public let coachLabel: String?
    public let createdAt: String
    public let revokedAt: String?

    enum CodingKeys: String, CodingKey {
        case grantID = "grant_id"
        case athleteID = "resource_id"
        case coachPrincipalID = "coach_principal_id"
        case clientID = "client_id"
        case permissions
        case coachLabel = "coach_label"
        case createdAt = "created_at"
        case revokedAt = "revoked_at"
    }
}

/// `DELETE /v1/athletes/{id}/grants/{grant_id}`.
public struct Revocation: Decodable, Sendable {
    public let grantID: String
    /// The athlete's new access epoch. A coach write that checked an older epoch fails.
    public let accessEpoch: Int

    enum CodingKeys: String, CodingKey {
        case grantID = "revoked"
        case accessEpoch = "access_epoch"
    }
}

/// One athlete on a coach's roster, through the app that asked.
public struct RosterEntry: Decodable, Sendable {
    public let athleteID: String
    public let displayName: String?
    public let permissions: [Permission]
    public let grantID: String

    enum CodingKeys: String, CodingKey {
        case athleteID = "athlete_id"
        case displayName = "display_name"
        case permissions
        case grantID = "grant_id"
    }
}

/// A workout as the read grant shows it. Times are UTC timestamps as the record
/// stores them.
public struct WorkoutSummary: Decodable, Sendable, Hashable {
    public let recordID: String
    public let revision: Int
    public let startAt: String
    public let endAt: String
    public let activityType: String?

    enum CodingKeys: String, CodingKey {
        case recordID = "record_id"
        case revision
        case startAt = "start_at"
        case endAt = "end_at"
        case activityType = "activity_type"
    }
}

/// One page of workout summaries, newest first. Pass `next` as `after` to get the
/// following page. `next` is nil on the last page.
public struct WorkoutPage: Decodable, Sendable {
    public let workouts: [WorkoutSummary]
    public let next: String?
}

/// A workout revision a coaching note points to.
public struct Evidence: Codable, Sendable, Hashable {
    public var recordID: String
    public var revision: Int

    public init(recordID: String, revision: Int) {
        self.recordID = recordID
        self.revision = revision
    }

    /// The current revision of a workout from the summary list.
    public init(_ workout: WorkoutSummary) {
        self.init(recordID: workout.recordID, revision: workout.revision)
    }

    enum CodingKeys: String, CodingKey {
        case recordID = "record_id"
        case revision
    }
}

/// A coaching note to add. The host writes the author and the time.
public struct CoachingNote: Encodable, Sendable, Hashable {
    public var note: String
    public var title: String?
    /// A local date, such as `2026-12-01`, from which the note applies.
    public var appliesFrom: String?
    public var appliesTo: String?
    public var evidence: [Evidence]

    public init(
        note: String, title: String? = nil, appliesFrom: String? = nil, appliesTo: String? = nil,
        evidence: [Evidence] = []
    ) {
        self.note = note
        self.title = title
        self.appliesFrom = appliesFrom
        self.appliesTo = appliesTo
        self.evidence = evidence
    }

    enum CodingKeys: String, CodingKey {
        case note
        case title
        case appliesFrom = "applies_from"
        case appliesTo = "applies_to"
        case evidence
    }
}

/// The result of a write: the record's new database revision and each record written.
public struct WriteResult: Decodable, Sendable, Equatable {
    public struct Record: Decodable, Sendable, Equatable {
        public let recordID: String
        public let revision: Int

        enum CodingKeys: String, CodingKey {
            case recordID = "record_id"
            case revision
        }
    }

    public let databaseRevision: Int
    public let records: [Record]

    enum CodingKeys: String, CodingKey {
        case databaseRevision = "database_revision"
        case records
    }
}
