// Test support: a real `ath-host` process in directory mode, a key server for its
// sign-in check, and access tokens signed with a fixed test key.

import AthleticStandardHost
import CryptoKit
import Darwin
import Foundation

struct SetupError: Error, CustomStringConvertible {
    let description: String
}

// MARK: - Paths

enum Paths {
    /// This package: three folders above this file.
    static let package = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    /// The repository: two folders above this package.
    static let repository = package.deletingLastPathComponent().deletingLastPathComponent()

    /// `ATH_HOST_EXECUTABLE`, or the one `cargo build` makes.
    static func hostExecutable() throws -> URL {
        try executable(
            environment: "ATH_HOST_EXECUTABLE", default: repository.appending(path: "target/debug/ath-host"),
            hint: "Build the host first with `cargo build -p athleticstandard-host`, or set ATH_HOST_EXECUTABLE.")
    }

    /// `ATH_NATIVE_EXECUTABLE`, or the one `cargo build` makes. The tests use it to
    /// export the demo record as a package.
    static func athExecutable() throws -> URL {
        try executable(
            environment: "ATH_NATIVE_EXECUTABLE", default: repository.appending(path: "target/debug/ath"),
            hint: "Build ath first with `cargo build`, or set ATH_NATIVE_EXECUTABLE.")
    }

    /// The `host-example` program `swift build` makes, next to the test bundle.
    static func exampleExecutable() throws -> URL {
        let bundles = Bundle.allBundles.filter { $0.bundlePath.hasSuffix(".xctest") }
        let folders = bundles.map { $0.bundleURL.deletingLastPathComponent() } + [package.appending(path: ".build/debug")]
        for folder in folders {
            let candidate = folder.appending(path: "host-example")
            if FileManager.default.isExecutableFile(atPath: candidate.path) {
                return candidate
            }
        }
        throw SetupError(description: "host-example is missing. Run `swift build` in examples/host-swift first.")
    }

    private static func executable(environment name: String, default fallback: URL, hint: String) throws -> URL {
        let url = ProcessInfo.processInfo.environment[name].map { URL(fileURLWithPath: $0) } ?? fallback
        guard FileManager.default.isExecutableFile(atPath: url.path) else {
            throw SetupError(description: "\(url.path) is missing. \(hint)")
        }
        return url
    }

    /// A new folder under the system's temporary folder, by its real path. The
    /// Athletic Standard library refuses paths through a symbolic link, and the
    /// temporary folder on macOS is often reached through one.
    static func temporaryFolder(_ prefix: String) throws -> URL {
        guard let real = realpath(FileManager.default.temporaryDirectory.path, nil) else {
            throw SetupError(description: "cannot resolve the temporary folder")
        }
        defer { free(real) }
        let folder = URL(fileURLWithPath: String(cString: real)).appending(path: prefix + UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        return folder
    }
}

// MARK: - Tokens

/// The fixed, test-only Ed25519 key from `host/tests/support/mod.rs`. It signs
/// nothing outside tests.
enum TestKey {
    static let seed = "4828f8020207fb11233d01b011c5937bede2b268a8603620b80bf97e050503d0"
    static let publicX = "itrbr0O5BuqdMUzuGWjVxNLAdQjldIGhKq8Rxhhr3_A"
    static let keyID = "test-key"
    static let issuer = "https://sign-in.test"

    static func privateKey() throws -> Curve25519.Signing.PrivateKey {
        let bytes = stride(from: 0, to: seed.count, by: 2).map { offset in
            let start = seed.index(seed.startIndex, offsetBy: offset)
            return UInt8(seed[start..<seed.index(start, offsetBy: 2)], radix: 16)!
        }
        return try Curve25519.Signing.PrivateKey(rawRepresentation: Data(bytes))
    }

    /// The public key as a JSON Web Key Set, as a sign-in provider publishes it.
    static func keySet() throws -> Data {
        let x = base64URL(try privateKey().publicKey.rawRepresentation)
        let key = ["kty": "OKP", "crv": "Ed25519", "x": x, "kid": keyID, "alg": "EdDSA", "use": "sig"]
        return try JSONEncoder().encode(["keys": [key]])
    }

    /// An access token for `subject`, issued to the app `client`, for `audience`.
    static func token(subject: String, client: String, audience: String) throws -> String {
        struct Claims: Encodable {
            let iss: String
            let sub: String
            let aud: String
            let azp: String
            let name: String
            let exp: Int
        }
        let header = try JSONEncoder().encode(["alg": "EdDSA", "kid": keyID, "typ": "JWT"])
        let claims = try JSONEncoder().encode(
            Claims(
                iss: issuer, sub: subject, aud: audience, azp: client, name: subject,
                exp: Int(Date().timeIntervalSince1970) + 3600))
        let signingInput = base64URL(header) + "." + base64URL(claims)
        let signature = try privateKey().signature(for: Data(signingInput.utf8))
        return signingInput + "." + base64URL(signature)
    }

    static func base64URL(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}

// MARK: - Key server

/// A loopback HTTP server that answers every request with the test key set. The
/// host reads its signing keys from here.
final class KeyServer: Sendable {
    let url: String
    private let socket: Int32

    init() throws {
        let body = try TestKey.keySet()
        let socket = try Loopback.listeningSocket()
        self.socket = socket
        url = "http://127.0.0.1:\(Loopback.port(of: socket))/jwks.json"
        let head = [
            "HTTP/1.1 200 OK", "Content-Type: application/json", "Content-Length: \(body.count)", "Connection: close",
        ]
        let reply = Data((head.joined(separator: "\r\n") + "\r\n\r\n").utf8) + body
        Thread.detachNewThread {
            while true {
                let connection = accept(socket, nil, nil)
                guard connection >= 0 else { return }
                Loopback.readRequestHead(connection)
                reply.withUnsafeBytes { bytes in _ = write(connection, bytes.baseAddress, bytes.count) }
                close(connection)
            }
        }
    }

    func stop() {
        close(socket)
    }
}

enum Loopback {
    static func listeningSocket(port: UInt16 = 0) throws -> Int32 {
        let socket = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard socket >= 0 else { throw SetupError(description: "cannot open a socket") }
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = port.bigEndian
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(socket, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard bound == 0, listen(socket, 16) == 0 else {
            close(socket)
            throw SetupError(description: "cannot listen on a loopback port")
        }
        return socket
    }

    static func port(of socket: Int32) -> UInt16 {
        var address = sockaddr_in()
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(socket, $0, &length) }
        }
        return UInt16(bigEndian: address.sin_port)
    }

    /// A port that was free a moment ago. The host prints its configured address,
    /// not the port it bound, so the tests choose the port.
    static func freePort() throws -> UInt16 {
        let socket = try listeningSocket()
        defer { close(socket) }
        return port(of: socket)
    }

    static func readRequestHead(_ connection: Int32) {
        var received = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while received.range(of: Data("\r\n\r\n".utf8)) == nil {
            let count = read(connection, &buffer, buffer.count)
            if count <= 0 { return }
            received.append(contentsOf: buffer[0..<count])
        }
    }
}

// MARK: - The host

/// An `ath-host serve` process in native directory mode, with its own data folder.
final class TestHost {
    static let athleteApp = "athlete-app"
    static let coachApp = "coach-app"

    let url: URL
    let folder: URL
    private let process: Process
    private let keyServer: KeyServer

    private init(url: URL, folder: URL, process: Process, keyServer: KeyServer) {
        self.url = url
        self.folder = folder
        self.process = process
        self.keyServer = keyServer
    }

    static func start() async throws -> TestHost {
        let executable = try Paths.hostExecutable()
        let folder = try Paths.temporaryFolder("ath-host-swift-")
        let keyServer = try KeyServer()
        let log = folder.appending(path: "host.log")
        // Another program can take the chosen port before the host binds it. Try a
        // new port when that happens.
        for _ in 0..<3 {
            let port = try Loopback.freePort()
            let url = URL(string: "http://127.0.0.1:\(port)")!
            let config = folder.appending(path: "host-config.json")
            try configuration(url: url, port: port, folder: folder, keyServer: keyServer).write(to: config)
            FileManager.default.createFile(atPath: log.path, contents: nil)
            let output = try FileHandle(forWritingTo: log)
            let process = Process()
            process.executableURL = executable
            process.arguments = ["serve", "--config", config.path]
            process.standardOutput = output
            process.standardError = output
            try process.run()
            if await ready(url, process: process) {
                return TestHost(url: url, folder: folder, process: process, keyServer: keyServer)
            }
            process.terminate()
            process.waitUntilExit()
        }
        keyServer.stop()
        let text = (try? String(contentsOf: log, encoding: .utf8)) ?? ""
        throw SetupError(description: "ath-host did not start. Its log:\n\(text)")
    }

    /// The host's configuration (host/src/config.rs). The token audience is the
    /// host's own address.
    private static func configuration(url: URL, port: UInt16, folder: URL, keyServer: KeyServer) throws -> Data {
        let config: [String: Any] = [
            "listen": "127.0.0.1:\(port)",
            "public_url": url.absoluteString,
            "data_directory": folder.appending(path: "data").path,
            "storage": ["kind": "directory"],
            "sign_in": [
                "issuer": TestKey.issuer,
                "audience": url.absoluteString,
                "jwks_uri": keyServer.url,
            ],
            "operator_disclosure": "Test host run by the Swift example tests, storing records on this machine.",
            "open_enrollment": true,
        ]
        return try JSONSerialization.data(withJSONObject: config, options: [.prettyPrinted, .sortedKeys])
    }

    private static func ready(_ url: URL, process: Process) async -> Bool {
        let deadline = Date().addingTimeInterval(30)
        while Date() < deadline, process.isRunning {
            var request = URLRequest(url: url.appending(path: "v1/capabilities"))
            request.timeoutInterval = 2
            if let (_, response) = try? await URLSession.shared.data(for: request),
                (response as? HTTPURLResponse)?.statusCode == 200
            {
                return true
            }
            try? await Task.sleep(for: .milliseconds(100))
        }
        return false
    }

    /// A token for `subject` signed in through `app`, with this host as audience.
    func token(_ subject: String, app: String) throws -> String {
        try TestKey.token(subject: subject, client: app, audience: url.absoluteString)
    }

    func client(_ subject: String, app: String) throws -> HostClient {
        HostClient(baseURL: url, token: try token(subject, app: app))
    }

    func stop() {
        process.terminate()
        process.waitUntilExit()
        keyServer.stop()
        try? FileManager.default.removeItem(at: folder)
    }
}

// MARK: - Programs

/// Run a program and return what it printed. Throws when it exits with an error.
@discardableResult
func run(_ executable: URL, _ arguments: [String], in folder: URL? = nil, environment: [String: String] = [:])
    throws -> String
{
    let process = Process()
    let pipe = Pipe()
    process.executableURL = executable
    process.arguments = arguments
    process.currentDirectoryURL = folder
    process.environment = ProcessInfo.processInfo.environment.merging(environment) { _, new in new }
    process.standardOutput = pipe
    process.standardError = pipe
    try process.run()
    let output = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
    process.waitUntilExit()
    guard process.terminationStatus == 0 else {
        throw SetupError(
            description: "\(executable.lastPathComponent) exited with \(process.terminationStatus):\n\(output)")
    }
    return output
}

/// The demo record as a complete package zip, the way an athlete's device would
/// upload it: `ath export`, then zip the package folder's contents.
func demoPackageZip(in folder: URL) throws -> Data {
    let record = folder.appending(path: "demo-record")
    try FileManager.default.copyItem(at: Paths.repository.appending(path: "examples/demo-athlete"), to: record)
    let package = folder.appending(path: "demo-package")
    try run(
        try Paths.athExecutable(),
        ["export", package.path, "--file", record.appending(path: "athlete.ath.sqlite").path])
    let zip = folder.appending(path: "demo-package.zip")
    try run(URL(fileURLWithPath: "/usr/bin/zip"), ["-r", "-X", "-D", "-q", zip.path, "."], in: package)
    return try Data(contentsOf: zip)
}
