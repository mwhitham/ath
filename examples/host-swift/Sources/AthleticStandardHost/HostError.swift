import Foundation

/// An error reply from the host. The host sends `{"error": {"code", "message"}}`
/// with a stable code (spec 2.1.3 section 6.5). Network failures stay `URLError`.
public struct HostError: Error, Sendable, CustomStringConvertible {
    /// The host's stable error codes. It is a string wrapper, so a code added by a
    /// newer host still decodes.
    public struct Code: RawRepresentable, Hashable, Sendable, CustomStringConvertible {
        public let rawValue: String

        public init(rawValue: String) {
            self.rawValue = rawValue
        }

        public var description: String { rawValue }

        /// The token is missing, expired, for another audience, or from an app the
        /// host does not accept. Sign in again.
        public static let signInRequired = Code(rawValue: "sign_in_required")
        /// The caller can see the athlete but the grant does not cover this.
        public static let permissionDenied = Code(rawValue: "permission_denied")
        /// The athlete disconnected this app from the record. Connect it again from
        /// another app the athlete uses.
        public static let appDisconnected = Code(rawValue: "app_disconnected")
        /// No such athlete or record, or it is not shared with the caller. The host
        /// gives the same answer for both, so a caller cannot learn that it exists.
        public static let notFound = Code(rawValue: "not_found")
        /// The record changed since the caller read it, or access changed while the
        /// write waited: a grant was revoked or an app disconnected.
        public static let stale = Code(rawValue: "stale")
        /// This `Idempotency-Key` was already used for a different request.
        public static let requestConflict = Code(rawValue: "request_conflict")
        /// The record is moving to another host and accepts no changes.
        public static let hostMoving = Code(rawValue: "host_moving")
        public static let limitReached = Code(rawValue: "limit_reached")
        /// The request is malformed.
        public static let invalid = Code(rawValue: "invalid")
        /// The request is well formed, but the record rules refuse it.
        public static let refused = Code(rawValue: "refused")
        /// The host failed. Its own log has the details.
        public static let internalError = Code(rawValue: "internal")
    }

    /// The HTTP status.
    public let status: Int
    /// The host's code, or nil when the reply had no error body. The host's web
    /// framework answers an unknown path or an unreadable query without one.
    public let code: Code?
    public let message: String

    public var description: String {
        "\(code?.rawValue ?? "http_error") (\(status)): \(message)"
    }

    init(status: Int, body: Data) {
        struct Envelope: Decodable {
            struct Detail: Decodable {
                let code: String
                let message: String
            }
            let error: Detail
        }
        self.status = status
        if let envelope = try? JSONDecoder().decode(Envelope.self, from: body) {
            code = Code(rawValue: envelope.error.code)
            message = envelope.error.message
        } else {
            code = nil
            let text = String(decoding: body, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            message = text.isEmpty ? HTTPURLResponse.localizedString(forStatusCode: status) : text
        }
    }
}
