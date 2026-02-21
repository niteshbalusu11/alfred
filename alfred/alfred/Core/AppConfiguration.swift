import AlfredAPIClient
import CryptoKit
import Foundation

enum AppConfiguration {
    private static let fallbackAPIBaseURL = "http://100.122.127.87:8080"
    private static let fallbackGoogleOAuthRedirectURI = "alfred://oauth/google/callback"
    private static let fallbackAppGroupIdentifier = "group.com.prodata.alfred.shared"
    private static let fallbackAutomationNotificationKeychainService =
        "com.prodata.alfred.automation-notification"

    static var defaultAPIBaseURL: URL {
        let rawValue = configValue(
            envKey: "ALFRED_API_BASE_URL",
            bundleKey: "ALFRED_API_BASE_URL",
            fallback: fallbackAPIBaseURL
        )
        guard let parsed = URL(string: rawValue) else {
            preconditionFailure("ALFRED_API_BASE_URL must be a valid URL.")
        }
        return parsed
    }

    static var defaultGoogleOAuthRedirectURI: String {
        configValue(
            envKey: "GOOGLE_OAUTH_REDIRECT_URI",
            bundleKey: "GOOGLE_OAUTH_REDIRECT_URI",
            fallback: fallbackGoogleOAuthRedirectURI
        )
    }

    static var appGroupIdentifier: String {
        configValue(
            envKey: "ALFRED_APP_GROUP_IDENTIFIER",
            bundleKey: "ALFRED_APP_GROUP_IDENTIFIER",
            fallback: fallbackAppGroupIdentifier
        )
    }

    static var automationNotificationKeychainService: String {
        configValue(
            envKey: "ALFRED_NOTIFICATION_KEYCHAIN_SERVICE",
            bundleKey: "ALFRED_NOTIFICATION_KEYCHAIN_SERVICE",
            fallback: fallbackAutomationNotificationKeychainService
        )
    }

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

    static var assistantAttestationVerificationConfig: AssistantAttestationVerificationConfig {
        AssistantAttestationVerificationConfig(
            expectedRuntime: configValue(
                envKey: "TEE_EXPECTED_RUNTIME",
                bundleKey: "TEE_EXPECTED_RUNTIME",
                fallback: "nitro"
            ),
            allowedMeasurements: Set(
                csvConfigValue(
                    envKey: "TEE_ALLOWED_MEASUREMENTS",
                    bundleKey: "TEE_ALLOWED_MEASUREMENTS",
                    fallback: ["dev-local-enclave"]
                )
            ),
            attestationPublicKeyBase64: configValue(
                envKey: "TEE_ATTESTATION_PUBLIC_KEY",
                bundleKey: "TEE_ATTESTATION_PUBLIC_KEY",
                fallback: defaultDevAttestationPublicKeyBase64
            )
        )
    }

    private static var defaultDevAttestationPublicKeyBase64: String {
        let seed = Data(repeating: 7, count: 32)
        guard let signingKey = try? Curve25519.Signing.PrivateKey(rawRepresentation: seed) else {
            return ""
        }
        return signingKey.publicKey.rawRepresentation.base64EncodedString()
    }

    private static func configValue(envKey: String, bundleKey: String, fallback: String) -> String {
        let envValue = ProcessInfo.processInfo.environment[envKey]
        let bundleValue = Bundle.main.object(forInfoDictionaryKey: bundleKey) as? String
        return [envValue, bundleValue]
            .compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
            .first(where: { !$0.isEmpty }) ?? fallback
    }

    private static func csvConfigValue(envKey: String, bundleKey: String, fallback: [String]) -> [String] {
        let rawValue = configValue(envKey: envKey, bundleKey: bundleKey, fallback: "")
        let parsed = rawValue
            .split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
        return parsed.isEmpty ? fallback : parsed
    }
}
