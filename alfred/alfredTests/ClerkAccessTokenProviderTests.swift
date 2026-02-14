import Foundation
import Testing
@testable import alfred

@MainActor
private final class MockClerkTokenSource: ClerkTokenSource {
    let token: String?
    let error: Error?

    init(token: String?, error: Error? = nil) {
        self.token = token
        self.error = error
    }

    func fetchToken() async throws -> String? {
        if let error {
            throw error
        }
        return token
    }
}

private enum MockTokenSourceError: Error {
    case failed
}

struct ClerkAccessTokenProviderTests {
    @Test
    @MainActor
    func returnsTokenFromSource() async throws {
        let tokenSource = MockClerkTokenSource(token: "clerk-token")
        let provider = ClerkAccessTokenProvider(tokenSource: tokenSource)

        let token = try await provider.accessToken()
        #expect(token == "clerk-token")
    }

    @Test
    @MainActor
    func propagatesSourceError() async {
        let tokenSource = MockClerkTokenSource(token: nil, error: MockTokenSourceError.failed)
        let provider = ClerkAccessTokenProvider(tokenSource: tokenSource)

        await #expect(throws: MockTokenSourceError.self) {
            _ = try await provider.accessToken()
        }
    }
}
