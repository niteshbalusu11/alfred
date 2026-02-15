import AlfredAPIClient
import Foundation

extension AppModel {
    func handleOAuthCallbackURL(
        _ url: URL,
        completionForTesting: ((CompleteGoogleConnectRequest) async -> Void)? = nil
    ) async {
        AppLogger.debug("Handling OAuth callback URL.", category: .oauth)
        let normalizedRedirectURI = redirectURI.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedRedirectURI.isEmpty else {
            AppLogger.warning("OAuth callback rejected because redirect URI is empty.", category: .oauth)
            errorBanner = ErrorBanner(
                message: "Redirect URI is required.",
                retryAction: nil,
                sourceAction: .completeGoogleOAuth
            )
            return
        }

        do {
            guard let payload = try GoogleOAuthCallbackParser.parse(
                callbackURL: url,
                redirectURI: normalizedRedirectURI,
                expectedState: trimmedOrNil(googleState)
            ) else {
                AppLogger.debug("Ignored non-matching callback URL.", category: .oauth)
                return
            }

            googleState = payload.state
            googleCode = payload.code ?? ""
            googleCallbackError = payload.error ?? ""
            googleErrorDescription = payload.errorDescription ?? ""

            let callbackRequest = CompleteGoogleConnectRequest(
                code: payload.code,
                state: payload.state,
                error: payload.error,
                errorDescription: payload.errorDescription
            )

            if let completionForTesting {
                await completionForTesting(callbackRequest)
                return
            }

            await completeGoogleOAuth()
        } catch let parseError as GoogleOAuthCallbackParsingError {
            AppLogger.warning("OAuth callback parsing failed: \(parseError.localizedDescription)", category: .oauth)
            errorBanner = ErrorBanner(
                message: parseError.localizedDescription,
                retryAction: nil,
                sourceAction: .completeGoogleOAuth
            )
        } catch {
            AppLogger.error(
                "Unexpected OAuth callback handling error of type \(String(describing: type(of: error))).",
                category: .oauth
            )
            errorBanner = ErrorBanner(
                message: Self.errorMessage(from: error),
                retryAction: nil,
                sourceAction: .completeGoogleOAuth
            )
        }
    }
}
