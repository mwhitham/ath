// host-example: the coach flow against a running ath-host.
//
//   ATH_HOST_URL     the host's address, such as https://records.example.org
//   ATH_HOST_TOKEN   a coach's access token, issued to the coach app, with the host
//                    as its audience
//   ATH_GRANT_CODE   optional: an athlete's offer code to accept first
//   ATH_NOTE         optional: the text of the note to add
//
// It prints each step in plain words.

import AthleticStandardHost
import Foundation

let environment = ProcessInfo.processInfo.environment
guard let address = environment["ATH_HOST_URL"], let baseURL = URL(string: address),
    let token = environment["ATH_HOST_TOKEN"], !token.isEmpty
else {
    fail("Set ATH_HOST_URL to the host's address and ATH_HOST_TOKEN to a coach's access token.", status: 2)
}

do {
    try await coachFlow(
        host: HostClient(baseURL: baseURL, token: token),
        grantCode: environment["ATH_GRANT_CODE"].flatMap { $0.isEmpty ? nil : $0 },
        noteText: environment["ATH_NOTE"] ?? "Keep the first half of the next workout easy.")
} catch let error as HostError {
    fail("The host refused a request: \(error.message) (\(error.code?.rawValue ?? "status \(error.status)")).")
} catch {
    fail("The request did not finish: \(error.localizedDescription)")
}

func coachFlow(host: HostClient, grantCode: String?, noteText: String) async throws {
    let capabilities = try await host.capabilities()
    say("The host speaks API version \(capabilities.apiVersion) and keeps records in \(capabilities.storage) storage.")
    say("Who runs it: \(capabilities.operatorDisclosure)")

    let me = try await host.me()
    say("You are signed in through the app \(me.clientID). Your coach ID is \(me.coachID).")

    if let grantCode {
        let grant = try await host.acceptGrantOffer(code: grantCode)
        say("You accepted an athlete's offer. You can now \(describe(grant.permissions)).")
    }

    let roster = try await host.roster()
    guard let athlete = roster.first else {
        say("No athlete has shared a record with you through this app.")
        return
    }
    say("You coach \(roster.count) \(roster.count == 1 ? "athlete" : "athletes") through this app.")
    for entry in roster {
        say("- \(entry.displayName ?? entry.athleteID): you can \(describe(entry.permissions)).")
    }
    let name = athlete.displayName ?? athlete.athleteID

    var latest: WorkoutSummary?
    if athlete.permissions.contains(.readWorkoutSummaries) {
        let page = try await host.workoutSummaries(athleteID: athlete.athleteID, limit: 5)
        latest = page.workouts.first
        say("\(name) has \(page.workouts.isEmpty ? "no" : "these") recent workouts\(page.workouts.isEmpty ? "." : ":")")
        for workout in page.workouts {
            say("- \(workout.activityType ?? "workout") from \(workout.startAt) to \(workout.endAt)")
        }
        if page.next != nil {
            say("There are more. Pass the last workout's record ID as `after` to read the next page.")
        }
    } else {
        say("You cannot read \(name)'s workouts.")
    }

    guard athlete.permissions.contains(.addCoachNotes) else {
        say("You cannot add notes for \(name).")
        return
    }
    let note = CoachingNote(note: noteText, evidence: latest.map { [Evidence($0)] } ?? [])
    // One key for this note. A retry reuses it, so the note is written once.
    let key = UUID().uuidString
    let added = try await host.addCoachingNote(note, athleteID: athlete.athleteID, idempotencyKey: key)
    let evidence = latest.map { " about the \($0.activityType ?? "workout") on \($0.startAt)" } ?? ""
    say("You added a note\(evidence). The host saved it as record \(added.records[0].recordID).")

    let retried = try await host.addCoachingNote(note, athleteID: athlete.athleteID, idempotencyKey: key)
    if retried == added {
        say("You sent the same note again with the same key. The host returned the first result and saved nothing new.")
    } else {
        say("You sent the same note again with the same key, but the host gave a different result.")
    }
}

func describe(_ permissions: [Permission]) -> String {
    let words = permissions.map { permission in
        switch permission {
        case .readWorkoutSummaries: "read workout summaries"
        case .addCoachNotes: "add coaching notes"
        case .addProgramming: "add training plans"
        default: permission.rawValue
        }
    }
    switch words.count {
    case 0: return "do nothing yet"
    case 1: return words[0]
    default: return words.dropLast().joined(separator: ", ") + " and " + words.last!
    }
}

func say(_ line: String) {
    print(line)
}

func fail(_ message: String, status: Int32 = 1) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(status)
}
