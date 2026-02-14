import AlfredAPIClient
import Foundation

protocol AuthSessionClient: Sendable {
    func createIOSSession(_ request: CreateSessionRequest) async throws -> CreateSessionResponse
}

extension AlfredAPIClient: AuthSessionClient {}

@MainActor
final class SessionManager {
    private let authClient: AuthSessionClient
    private let tokenStore: SessionTokenStore
    private let now: @Sendable () -> Date
    private var session: StoredSession?

    init(
        authClient: AuthSessionClient,
        tokenStore: SessionTokenStore,
        now: @escaping @Sendable () -> Date = Date.init
    ) {
        self.authClient = authClient
        self.tokenStore = tokenStore
        self.now = now
    }

    func restoreSession() {
        let data: Data?
        do {
            data = try tokenStore.readSessionData()
        } catch {
            session = nil
            return
        }

        guard let data,
              let decoded = try? JSONDecoder().decode(StoredSession.self, from: data),
              decoded.isValid(at: now())
        else {
            try? tokenStore.clearSessionData()
            session = nil
            return
        }

        session = decoded
    }

    func createSession(appleIdentityToken: String, deviceID: String) async throws {
        let response = try await authClient.createIOSSession(
            CreateSessionRequest(appleIdentityToken: appleIdentityToken, deviceId: deviceID)
        )

        let expiresIn = TimeInterval(max(response.expiresIn, 0))
        let persisted = StoredSession(
            accessToken: response.accessToken,
            refreshToken: response.refreshToken,
            expiresAt: now().addingTimeInterval(expiresIn)
        )

        try persist(session: persisted)
    }

    func accessToken() throws -> String? {
        guard let session else {
            return nil
        }

        guard session.isValid(at: now()) else {
            try tokenStore.clearSessionData()
            self.session = nil
            return nil
        }

        return session.accessToken
    }

    func isAuthenticated() -> Bool {
        (try? accessToken()) != nil
    }

    func clearSession() {
        try? tokenStore.clearSessionData()
        session = nil
    }

    private func persist(session: StoredSession) throws {
        let data = try JSONEncoder().encode(session)
        try tokenStore.writeSessionData(data)
        self.session = session
    }
}
