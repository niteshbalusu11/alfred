import Foundation

enum AppConfiguration {
    static let defaultAPIBaseURL = URL(string: "http://100.122.127.87:8080")!

    static var clerkPublishableKey: String {
        let envValue = ProcessInfo.processInfo.environment["CLERK_PUBLISHABLE_KEY"]
        let bundleValue =
            Bundle.main
            .object(forInfoDictionaryKey: "CLERK_PUBLISHABLE_KEY") as? String

        // Prefer per-run override (scheme/env) and fall back to bundled value for app relaunches.
        return [envValue, bundleValue]
            .compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
            .first(where: { !$0.isEmpty }) ?? ""
    }

    static var requiredClerkPublishableKey: String {
        let key = clerkPublishableKey
        precondition(!key.isEmpty, "CLERK_PUBLISHABLE_KEY is required to initialize Clerk.")
        return key
    }
}
