// The coach flow against a real `ath-host` (spec 2.1.3 section 7): an athlete
// shares a record, a coach reads workout summaries and adds a note, and the
// athlete revokes the grant.

import AthleticStandardHost
import Foundation
import Testing

@Suite("HostClient against a real ath-host")
struct CoachFlowTests {
    @Test("The test key is the one the host's own tests use")
    func testKeyMatchesTheHostTests() throws {
        #expect(TestKey.base64URL(try TestKey.privateKey().publicKey.rawRepresentation) == TestKey.publicX)
    }

    @Test("An athlete shares a record, a coach adds a note, and the athlete revokes the grant")
    func coachFlow() async throws {
        let host = try await TestHost.start()
        defer { host.stop() }
        let owner = try host.client("athlete-1", app: TestHost.athleteApp)
        let coach = try host.client("coach-1", app: TestHost.coachApp)

        // The owner connects: an empty record, then the demo package restored into it.
        let capabilities = try await owner.capabilities()
        #expect(capabilities.apiVersion == 1)
        #expect(capabilities.storage == "directory")
        #expect(Set(capabilities.permissions) == [.readWorkoutSummaries, .addCoachNotes, .addProgramming])
        let resource = try await owner.protectedResource()
        #expect(resource.resource == host.url.absoluteString)
        #expect(resource.authorizationServers == [TestKey.issuer])

        let id = try await owner.createAthlete(displayName: "Demo athlete").athleteID
        let revision = try await owner.restorePackage(athleteID: id, zip: try demoPackageZip(in: host.folder))
        #expect(revision > 0)
        #expect(try await owner.me().athletes == [id])
        // Two more workouts, so the summaries take more than one page.
        let ownerToken = try host.token("athlete-1", app: TestHost.athleteApp)
        try await addWorkout(host, token: ownerToken, athleteID: id, day: "2026-08-12", activity: "rowing")
        try await addWorkout(host, token: ownerToken, athleteID: id, day: "2026-08-14", activity: "running")

        // The owner offers read and note access to the coach app.
        let offer = try await owner.createGrantOffer(
            athleteID: id, permissions: [.readWorkoutSummaries, .addCoachNotes], coachClientID: TestHost.coachApp,
            coachLabel: "Coach Sam")
        #expect(offer.clientID == TestHost.coachApp)

        // Nothing is shared before the coach accepts.
        #expect(try await coach.roster().isEmpty)
        #expect(await hostError { try await coach.workoutSummaries(athleteID: id) }?.code == .notFound)

        // The same coach in another app cannot accept the offer.
        let coachElsewhere = try host.client("coach-1", app: TestHost.athleteApp)
        #expect(await hostError { try await coachElsewhere.acceptGrantOffer(code: offer.code) }?.code == .notFound)

        // The coach accepts with the coach app. The code works once.
        let grant = try await coach.acceptGrantOffer(code: offer.code)
        #expect(grant.athleteID == id)
        #expect(grant.clientID == TestHost.coachApp)
        #expect(grant.coachLabel == "Coach Sam")
        #expect(Set(grant.permissions) == [.readWorkoutSummaries, .addCoachNotes])
        #expect(await hostError { try await coach.acceptGrantOffer(code: offer.code) }?.code == .notFound)

        let roster = try await coach.roster()
        #expect(roster.map(\.athleteID) == [id])
        #expect(roster.first?.grantID == grant.grantID)
        #expect(roster.first?.displayName == "Demo athlete")

        // Workout summaries, two to a page, newest first.
        let first = try await coach.workoutSummaries(athleteID: id, limit: 2)
        #expect(first.workouts.count == 2)
        let second = try await coach.workoutSummaries(athleteID: id, limit: 2, after: try #require(first.next))
        #expect(second.workouts.count == 1)
        #expect(second.next == nil)
        let workouts = first.workouts + second.workouts
        #expect(workouts.map(\.activityType) == ["running", "rowing", "crossfit"])
        #expect(Set(workouts.map(\.recordID)).count == 3)
        #expect(workouts.map(\.startAt) == workouts.map(\.startAt).sorted(by: >))

        // The coach adds a note about the newest workout. A retry with the same key
        // returns the first result and writes nothing new.
        let newest = try #require(workouts.first)
        let note = CoachingNote(note: "Keep the first half of the next run easy.", evidence: [Evidence(newest)])
        let key = UUID().uuidString
        let added = try await coach.addCoachingNote(note, athleteID: id, idempotencyKey: key)
        #expect(added.records.count == 1)
        #expect(added.records.first?.revision == 1)
        let retried = try await coach.addCoachingNote(note, athleteID: id, idempotencyKey: key)
        #expect(retried == added)
        // The same key with a different note is a conflict.
        let changed = CoachingNote(note: "A different note.")
        let conflict = await hostError { try await coach.addCoachingNote(changed, athleteID: id, idempotencyKey: key) }
        #expect(conflict?.code == .requestConflict)
        // A new key writes a second note.
        let another = try await coach.addCoachingNote(note, athleteID: id, idempotencyKey: UUID().uuidString)
        #expect(another.records.first?.recordID != added.records.first?.recordID)
        // Evidence must be the current revision of a workout the coach can read.
        let stale = CoachingNote(note: "Old revision.", evidence: [Evidence(recordID: newest.recordID, revision: 99)])
        let refused = await hostError {
            try await coach.addCoachingNote(stale, athleteID: id, idempotencyKey: UUID().uuidString)
        }
        #expect(refused?.code == .refused)

        // The grant does not cover the owner's operations.
        #expect(await hostError { try await coach.grants(athleteID: id) }?.code == .permissionDenied)

        // The owner revokes the grant. The coach's next requests find nothing.
        #expect(try await owner.grants(athleteID: id).map(\.grantID) == [grant.grantID])
        let revocation = try await owner.revokeGrant(athleteID: id, grantID: grant.grantID)
        #expect(revocation.grantID == grant.grantID)
        #expect(await hostError { try await coach.workoutSummaries(athleteID: id) }?.code == .notFound)
        let tooLate = CoachingNote(note: "Too late.")
        let late = await hostError {
            try await coach.addCoachingNote(tooLate, athleteID: id, idempotencyKey: UUID().uuidString)
        }
        #expect(late?.code == .notFound)
        #expect(try await coach.roster().isEmpty)
        #expect(try await owner.grants(athleteID: id).first?.revokedAt != nil)
        // Revoking again finds no active grant.
        let again = await hostError { try await owner.revokeGrant(athleteID: id, grantID: grant.grantID) }
        #expect(again?.code == .notFound)
    }

    @Test("The host refuses a token for another audience or with no signature it trusts")
    func signInChecks() async throws {
        let host = try await TestHost.start()
        defer { host.stop() }
        let elsewhere = try TestKey.token(
            subject: "coach-1", client: TestHost.coachApp, audience: "https://elsewhere.test")
        let wrongAudience = HostClient(baseURL: host.url, token: elsewhere)
        let refused = await hostError { try await wrongAudience.me() }
        #expect(refused?.code == .signInRequired)
        #expect(refused?.status == 401)
        let unsigned = HostClient(baseURL: host.url, token: "not-a-token")
        #expect(await hostError { try await unsigned.me() }?.code == .signInRequired)
        // Discovery needs no token.
        #expect(try await unsigned.capabilities().apiVersion == 1)
    }

    @Test("host-example runs the coach flow and says what it did")
    func commandLineExample() async throws {
        let host = try await TestHost.start()
        defer { host.stop() }
        let owner = try host.client("athlete-1", app: TestHost.athleteApp)
        let id = try await owner.createAthlete(displayName: "Demo athlete").athleteID
        _ = try await owner.restorePackage(athleteID: id, zip: try demoPackageZip(in: host.folder))
        let offer = try await owner.createGrantOffer(
            athleteID: id, permissions: [.readWorkoutSummaries, .addCoachNotes], coachClientID: TestHost.coachApp)

        let output = try run(
            try Paths.exampleExecutable(), [],
            environment: [
                "ATH_HOST_URL": host.url.absoluteString,
                "ATH_HOST_TOKEN": try host.token("coach-1", app: TestHost.coachApp),
                "ATH_GRANT_CODE": offer.code,
            ])
        #expect(output.contains("You can now read workout summaries and add coaching notes."))
        #expect(output.contains("- crossfit from 2026-08-10T17:25:00.000Z to 2026-08-10T17:48:00.000Z"))
        #expect(output.contains("You added a note about the crossfit on 2026-08-10T17:25:00.000Z."))
        #expect(output.contains("The host returned the first result and saved nothing new."))

        // Without a host address and a token, it stops with an error.
        #expect(throws: SetupError.self) {
            try run(try Paths.exampleExecutable(), [], environment: ["ATH_HOST_URL": "", "ATH_HOST_TOKEN": ""])
        }
    }
}

/// The `HostError` a call throws, or nil when it succeeds or fails another way.
func hostError<T>(_ call: () async throws -> T) async -> HostError? {
    do {
        _ = try await call()
        return nil
    } catch let error as HostError {
        return error
    } catch {
        return nil
    }
}

/// The owner logs a workout from the demo record's source. The client library has no
/// general record writes, so the test sends this owner request itself.
func addWorkout(_ host: TestHost, token: String, athleteID: String, day: String, activity: String) async throws {
    let start = "\(day)T17:00:00.000Z"
    let end = "\(day)T17:40:00.000Z"
    func input(_ time: String) -> [String: String] {
        [
            "timestamp_basis": "source_offset_timestamp",
            "timestamp_text": "\(day)T\(time)+00:00",
            "utc_offset_meaning": "provider_cycle_offset",
            "utc_offset_text": "+00:00",
        ]
    }
    let body: [String: Any] = [
        "record_type": "workout_session",
        "data": [
            "source_id": "01890f3e-e7d3-7cc0-88c8-000000000003",
            "received_at": end,
            "received_at_basis": "import_time",
            "start_at": start,
            "end_at": end,
            "input_start_timestamp": input("17:00:00"),
            "input_end_timestamp": input("17:40:00"),
            "activity_type": activity,
            "source_reported_summaries": [],
        ],
    ]
    var request = URLRequest(url: host.url.appending(path: "v1/athletes/\(athleteID)/records"))
    request.httpMethod = "POST"
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    request.setValue(UUID().uuidString, forHTTPHeaderField: "Idempotency-Key")
    request.httpBody = try JSONSerialization.data(withJSONObject: body)
    let (data, response) = try await URLSession.shared.data(for: request)
    let status = (response as? HTTPURLResponse)?.statusCode ?? 0
    guard status == 201 else {
        let text = String(decoding: data, as: UTF8.self)
        throw SetupError(description: "adding a workout failed with \(status): \(text)")
    }
}
