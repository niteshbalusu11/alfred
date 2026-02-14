import Foundation

actor SessionAccessTokenProvider {
    private let tokenLoader: @Sendable () async throws -> String?

    init(sessionManager: SessionManager) {
        self.tokenLoader = {
            try await MainActor.run {
                try sessionManager.accessToken()
            }
        }
    }

    func accessToken() async throws -> String? {
        try await tokenLoader()
    }
}
