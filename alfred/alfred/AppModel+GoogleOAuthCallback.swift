import AlfredAPIClient
import Foundation

extension AppModel {
    func handleOAuthCallbackURL(
        _ url: URL,
        completionForTesting: ((CompleteGoogleConnectRequest) async -> Void)? = nil
    ) async {
        do {
            guard let payload = try GoogleOAuthCallbackParser.parse(
                callbackURL: url,
                redirectURI: redirectURI,
                expectedState: trimmedOrNil(googleState)
            ) else {
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
            errorBanner = ErrorBanner(
                message: parseError.localizedDescription,
                retryAction: nil,
                sourceAction: .completeGoogleOAuth
            )
        } catch {
            errorBanner = ErrorBanner(
                message: Self.errorMessage(from: error),
                retryAction: nil,
                sourceAction: .completeGoogleOAuth
            )
        }
    }
}
