import Foundation
import Testing
@testable import alfred

struct GoogleOAuthCallbackParserTests {
    private let redirectURI = "alfred://oauth/google/callback"

    @Test
    func parsesCodeCallbackParameters() throws {
        let url = try #require(URL(string: "alfred://oauth/google/callback?code=oauth-code&state=state-123"))

        let payload = try #require(
            try GoogleOAuthCallbackParser.parse(
                callbackURL: url,
                redirectURI: redirectURI,
                expectedState: "state-123"
            )
        )

        #expect(payload.code == "oauth-code")
        #expect(payload.state == "state-123")
        #expect(payload.error == nil)
        #expect(payload.errorDescription == nil)
    }

    @Test
    func parsesErrorCallbackParameters() throws {
        let url = try #require(
            URL(string: "alfred://oauth/google/callback?error=access_denied&error_description=User%20cancelled&state=state-123")
        )

        let payload = try #require(
            try GoogleOAuthCallbackParser.parse(
                callbackURL: url,
                redirectURI: redirectURI,
                expectedState: "state-123"
            )
        )

        #expect(payload.code == nil)
        #expect(payload.error == "access_denied")
        #expect(payload.errorDescription == "User cancelled")
    }

    @Test
    func returnsNilForNonOAuthDeepLinks() throws {
        let url = try #require(URL(string: "alfred://settings?tab=privacy"))
        let payload = try GoogleOAuthCallbackParser.parse(
            callbackURL: url,
            redirectURI: redirectURI,
            expectedState: "state-123"
        )

        #expect(payload == nil)
    }

    @Test
    func throwsWhenStateIsMissing() throws {
        let url = try #require(URL(string: "alfred://oauth/google/callback?code=oauth-code"))

        #expect(throws: GoogleOAuthCallbackParsingError.self) {
            _ = try GoogleOAuthCallbackParser.parse(
                callbackURL: url,
                redirectURI: redirectURI,
                expectedState: "state-123"
            )
        }
    }

    @Test
    func throwsWhenStateDoesNotMatch() throws {
        let url = try #require(URL(string: "alfred://oauth/google/callback?code=oauth-code&state=unexpected"))

        #expect(throws: GoogleOAuthCallbackParsingError.self) {
            _ = try GoogleOAuthCallbackParser.parse(
                callbackURL: url,
                redirectURI: redirectURI,
                expectedState: "state-123"
            )
        }
    }

    @Test
    func throwsWhenCodeAndErrorAreBothMissing() throws {
        let url = try #require(URL(string: "alfred://oauth/google/callback?state=state-123"))

        #expect(throws: GoogleOAuthCallbackParsingError.self) {
            _ = try GoogleOAuthCallbackParser.parse(
                callbackURL: url,
                redirectURI: redirectURI,
                expectedState: "state-123"
            )
        }
    }
}
