import Foundation

struct GoogleOAuthCallbackPayload: Equatable {
    let code: String?
    let state: String
    let error: String?
    let errorDescription: String?
}

enum GoogleOAuthCallbackParsingError: LocalizedError, Equatable {
    case invalidRedirectURI
    case missingState
    case stateMismatch
    case missingCodeAndError

    var errorDescription: String? {
        switch self {
        case .invalidRedirectURI:
            return "OAuth redirect URI is invalid. Check app configuration."
        case .missingState:
            return "OAuth callback is missing state. Start Google connect again."
        case .stateMismatch:
            return "OAuth state mismatch. Restart Google connect for a new consent link."
        case .missingCodeAndError:
            return "OAuth callback is missing required parameters. Retry Google connect."
        }
    }
}

enum GoogleOAuthCallbackParser {
    static func parse(
        callbackURL: URL,
        redirectURI: String,
        expectedState: String?
    ) throws -> GoogleOAuthCallbackPayload? {
        guard let redirectComponents = URLComponents(string: redirectURI) else {
            throw GoogleOAuthCallbackParsingError.invalidRedirectURI
        }

        guard matchesRedirectURI(callbackURL: callbackURL, redirectComponents: redirectComponents) else {
            return nil
        }

        let queryItems = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let state = value(for: "state", in: queryItems)
        guard let state else {
            throw GoogleOAuthCallbackParsingError.missingState
        }

        if let expectedState, !expectedState.isEmpty, state != expectedState {
            throw GoogleOAuthCallbackParsingError.stateMismatch
        }

        let code = value(for: "code", in: queryItems)
        let error = value(for: "error", in: queryItems)
        let errorDescription = value(for: "error_description", in: queryItems)

        guard code != nil || error != nil else {
            throw GoogleOAuthCallbackParsingError.missingCodeAndError
        }

        return GoogleOAuthCallbackPayload(
            code: code,
            state: state,
            error: error,
            errorDescription: errorDescription
        )
    }

    private static func matchesRedirectURI(callbackURL: URL, redirectComponents: URLComponents) -> Bool {
        guard let callbackComponents = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false) else {
            return false
        }

        return callbackComponents.scheme?.lowercased() == redirectComponents.scheme?.lowercased()
            && callbackComponents.host?.lowercased() == redirectComponents.host?.lowercased()
            && normalizedPath(callbackComponents.path) == normalizedPath(redirectComponents.path)
    }

    private static func normalizedPath(_ path: String) -> String {
        guard !path.isEmpty else { return "/" }
        let trimmed = path.hasSuffix("/") ? String(path.dropLast()) : path
        return trimmed.isEmpty ? "/" : trimmed
    }

    private static func value(for name: String, in queryItems: [URLQueryItem]) -> String? {
        queryItems
            .first(where: { $0.name == name })?
            .value?
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .nonEmpty
    }
}

private extension String {
    var nonEmpty: String? {
        isEmpty ? nil : self
    }
}
