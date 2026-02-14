import ClerkKit
import Foundation

@MainActor
protocol ClerkTokenSource {
    func fetchToken() async throws -> String?
}

@MainActor
extension Auth: ClerkTokenSource {
    func fetchToken() async throws -> String? {
        try await getToken()
    }
}

actor ClerkAccessTokenProvider {
    private let tokenSource: ClerkTokenSource

    init(tokenSource: ClerkTokenSource) {
        self.tokenSource = tokenSource
    }

    func accessToken() async throws -> String? {
        try await tokenSource.fetchToken()
    }
}
