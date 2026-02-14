import AlfredAPIClient
import Foundation
import Testing
@testable import alfred

private actor MockAuthSessionClient: AuthSessionClient {
    private(set) var lastRequest: CreateSessionRequest?
    private let response: CreateSessionResponse

    init(response: CreateSessionResponse) {
        self.response = response
    }

    func createIOSSession(_ request: CreateSessionRequest) async throws -> CreateSessionResponse {
        lastRequest = request
        return response
    }
}

private final class InMemorySessionTokenStore: SessionTokenStore, @unchecked Sendable {
    private var storedData: Data?

    func readSessionData() throws -> Data? {
        storedData
    }

    func writeSessionData(_ data: Data) throws {
        storedData = data
    }

    func clearSessionData() throws {
        storedData = nil
    }
}

@MainActor
struct SessionManagerTests {
    @Test
    func createSessionPersistsAndProvidesAccessToken() async throws {
        let store = InMemorySessionTokenStore()
        let mockClient = MockAuthSessionClient(
            response: makeSessionResponse(
                accessToken: "access-token",
                refreshToken: "refresh-token",
                expiresIn: 3600
            )
        )
        let fixedNow = Date(timeIntervalSince1970: 1_000)
        let manager = SessionManager(
            authClient: mockClient,
            tokenStore: store,
            now: { fixedNow }
        )

        try await manager.createSession(appleIdentityToken: "apple-token", deviceID: "device-123")

        #expect(try manager.accessToken() == "access-token")
        #expect(manager.isAuthenticated())
        #expect(try store.readSessionData() != nil)

        let request = await mockClient.lastRequest
        #expect(request?.appleIdentityToken == "apple-token")
        #expect(request?.deviceId == "device-123")
    }

    @Test
    func restoreSessionClearsExpiredTokenData() async throws {
        let store = InMemorySessionTokenStore()
        let fixedNow = Date(timeIntervalSince1970: 2_000)
        let expired = StoredSession(
            accessToken: "expired-access-token",
            refreshToken: "refresh-token",
            expiresAt: fixedNow.addingTimeInterval(-10)
        )

        try store.writeSessionData(JSONEncoder().encode(expired))

        let manager = SessionManager(
            authClient: MockAuthSessionClient(
                response: makeSessionResponse(accessToken: "unused", refreshToken: "unused", expiresIn: 60)
            ),
            tokenStore: store,
            now: { fixedNow }
        )

        manager.restoreSession()

        #expect(manager.isAuthenticated() == false)
        #expect(try store.readSessionData() == nil)
    }

    @Test
    func clearSessionRemovesPersistedData() async throws {
        let store = InMemorySessionTokenStore()
        let manager = SessionManager(
            authClient: MockAuthSessionClient(
                response: makeSessionResponse(accessToken: "token", refreshToken: "refresh", expiresIn: 60)
            ),
            tokenStore: store
        )

        try await manager.createSession(appleIdentityToken: "apple-token", deviceID: "device-123")
        manager.clearSession()

        #expect(manager.isAuthenticated() == false)
        #expect(try store.readSessionData() == nil)
    }
}

private func makeSessionResponse(accessToken: String, refreshToken: String, expiresIn: Int) -> CreateSessionResponse {
    let json = """
    {
      "access_token": "\(accessToken)",
      "refresh_token": "\(refreshToken)",
      "expires_in": \(expiresIn)
    }
    """

    let data = Data(json.utf8)
    return try! JSONDecoder().decode(CreateSessionResponse.self, from: data)
}
