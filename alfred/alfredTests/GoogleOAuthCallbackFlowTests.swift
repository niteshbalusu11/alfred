import AlfredAPIClient
import ClerkKit
import Foundation
import Testing
@testable import alfred

struct GoogleOAuthCallbackFlowTests {
    @MainActor
    private func makeSignedOutModel() -> AppModel {
        let clerk = Clerk.preview { preview in
            preview.isSignedIn = false
        }
        return AppModel(clerk: clerk)
    }

    @Test
    @MainActor
    func callbackCodeTriggersCompletionPayload() async throws {
        let model = makeSignedOutModel()
        model.googleState = "state-123"
        let callbackURL = try #require(URL(string: "alfred://oauth/google/callback?code=oauth-code&state=state-123"))

        var capturedRequest: CompleteGoogleConnectRequest?

        await model.handleOAuthCallbackURL(callbackURL) { request in
            capturedRequest = request
        }

        let request = try #require(capturedRequest)
        #expect(request.code == "oauth-code")
        #expect(request.state == "state-123")
        #expect(request.error == nil)
        #expect(request.errorDescription == nil)
        #expect(model.googleCode == "oauth-code")
        #expect(model.errorBanner == nil)
    }

    @Test
    @MainActor
    func callbackErrorTriggersCompletionPayload() async throws {
        let model = makeSignedOutModel()
        model.googleState = "state-123"
        let callbackURL = try #require(
            URL(string: "alfred://oauth/google/callback?error=access_denied&error_description=User%20cancelled&state=state-123")
        )

        var capturedRequest: CompleteGoogleConnectRequest?

        await model.handleOAuthCallbackURL(callbackURL) { request in
            capturedRequest = request
        }

        let request = try #require(capturedRequest)
        #expect(request.code == nil)
        #expect(request.state == "state-123")
        #expect(request.error == "access_denied")
        #expect(request.errorDescription == "User cancelled")
        #expect(model.googleCallbackError == "access_denied")
        #expect(model.googleErrorDescription == "User cancelled")
        #expect(model.errorBanner == nil)
    }

    @Test
    @MainActor
    func stateMismatchSetsVisibleErrorWithoutCompleting() async throws {
        let model = makeSignedOutModel()
        model.googleState = "expected-state"
        let callbackURL = try #require(URL(string: "alfred://oauth/google/callback?code=oauth-code&state=wrong-state"))

        var completionCalled = false

        await model.handleOAuthCallbackURL(callbackURL) { _ in
            completionCalled = true
        }

        #expect(completionCalled == false)
        #expect(model.errorBanner?.message == GoogleOAuthCallbackParsingError.stateMismatch.localizedDescription)
    }
}
