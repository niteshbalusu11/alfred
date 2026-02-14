import AlfredAPIClient
import Combine
import Foundation

@MainActor
final class AppModel: ObservableObject {
    enum Action: Hashable {
        case restoreSession
        case signIn
        case startGoogleOAuth
        case completeGoogleOAuth
        case loadPreferences
        case savePreferences
        case revokeConnector
        case requestDeleteAll
        case loadAuditEvents
    }
    enum RetryAction {
        case restoreSession
        case signIn(appleIdentityToken: String, deviceID: String)
        case startGoogleOAuth(redirectURI: String)
        case completeGoogleOAuth(code: String?, state: String, error: String?, errorDescription: String?)
        case loadPreferences
        case savePreferences(Preferences)
        case revokeConnector(connectorID: String)
        case requestDeleteAll
        case loadAuditEvents(reset: Bool)
    }
    struct ErrorBanner {
        let message: String
        let retryAction: RetryAction?
        let sourceAction: Action?
    }

    @Published private(set) var isAuthenticated = false
    @Published private(set) var inFlightActions: Set<Action> = []
    @Published var errorBanner: ErrorBanner?

    @Published var appleIdentityToken = ""
    @Published var deviceID = UUID().uuidString

    @Published var redirectURI = "alfred://oauth/google/callback"
    @Published var googleAuthURL = ""
    @Published var googleState = ""
    @Published var googleCode = ""
    @Published var googleCallbackError = ""
    @Published var googleErrorDescription = ""

    @Published var connectorID = ""
    @Published var revokeStatus = ""
    @Published var deleteAllStatus = ""

    @Published var meetingReminderMinutes = "15"
    @Published var morningBriefLocalTime = "08:00"
    @Published var quietHoursStart = "22:00"
    @Published var quietHoursEnd = "07:00"
    @Published var highRiskRequiresConfirm = true

    @Published private(set) var auditEvents: [AuditEvent] = []
    @Published private(set) var nextAuditCursor: String?

    let apiBaseURL: URL

    private let sessionManager: SessionManager
    private let apiClient: AlfredAPIClient

    init(apiBaseURL: URL? = nil) {
        let resolvedAPIBaseURL = apiBaseURL ?? AppConfiguration.defaultAPIBaseURL
        self.apiBaseURL = resolvedAPIBaseURL

        let tokenStore = KeychainSessionTokenStore()
        let authClient = AlfredAPIClient(baseURL: resolvedAPIBaseURL)
        let sessionManager = SessionManager(authClient: authClient, tokenStore: tokenStore)
        let accessTokenProvider = SessionAccessTokenProvider(sessionManager: sessionManager)

        self.sessionManager = sessionManager
        self.apiClient = AlfredAPIClient(
            baseURL: resolvedAPIBaseURL,
            tokenProvider: {
                try await accessTokenProvider.accessToken()
            }
        )

        Task {
            await restoreSession()
        }
    }

    var canLoadMoreAuditEvents: Bool { nextAuditCursor != nil }
    func isLoading(_ action: Action) -> Bool { inFlightActions.contains(action) }

    func restoreSession() async {
        await run(action: .restoreSession, retryAction: .restoreSession) { [self] in
            sessionManager.restoreSession()
            isAuthenticated = sessionManager.isAuthenticated()
        }
    }

    func signIn() async {
        let token = appleIdentityToken.trimmingCharacters(in: .whitespacesAndNewlines)
        let id = deviceID.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !token.isEmpty else {
            errorBanner = ErrorBanner(message: "Apple identity token is required.", retryAction: nil, sourceAction: nil)
            return
        }

        let resolvedDeviceID = id.isEmpty ? UUID().uuidString : id

        await run(action: .signIn, retryAction: .signIn(appleIdentityToken: token, deviceID: resolvedDeviceID)) { [self] in
            try await sessionManager.createSession(appleIdentityToken: token, deviceID: resolvedDeviceID)
            isAuthenticated = true
            deviceID = resolvedDeviceID
            appleIdentityToken = ""
        }

        if isAuthenticated {
            await loadPreferences()
            await loadAuditEvents(reset: true)
        }
    }

    func signOut() async {
        sessionManager.clearSession()
        isAuthenticated = false
        auditEvents = []
        nextAuditCursor = nil
        deleteAllStatus = ""
        revokeStatus = ""
        appleIdentityToken = ""
        googleAuthURL = ""
        googleState = ""
        googleCode = ""
        googleCallbackError = ""
        googleErrorDescription = ""
        errorBanner = nil
    }

    func startGoogleOAuth() async {
        let redirect = redirectURI.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !redirect.isEmpty else {
            errorBanner = ErrorBanner(message: "Redirect URI is required.", retryAction: nil, sourceAction: nil)
            return
        }

        await run(action: .startGoogleOAuth, retryAction: .startGoogleOAuth(redirectURI: redirect)) { [self] in
            let response = try await apiClient.startGoogleOAuth(StartGoogleConnectRequest(redirectURI: redirect))
            googleAuthURL = response.authURL
            googleState = response.state
        }
    }

    func completeGoogleOAuth() async {
        let state = googleState.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !state.isEmpty else {
            errorBanner = ErrorBanner(message: "OAuth state is required. Start connect first.", retryAction: nil, sourceAction: nil)
            return
        }

        let code = trimmedOrNil(googleCode)
        let callbackError = trimmedOrNil(googleCallbackError)
        let errorDescription = trimmedOrNil(googleErrorDescription)

        await run(
            action: .completeGoogleOAuth,
            retryAction: .completeGoogleOAuth(code: code, state: state, error: callbackError, errorDescription: errorDescription)
        ) { [self] in
            let response = try await apiClient.completeGoogleOAuth(
                CompleteGoogleConnectRequest(
                    code: code,
                    state: state,
                    error: callbackError,
                    errorDescription: errorDescription
                )
            )

            connectorID = response.connectorId
            revokeStatus = "Connector status: \(response.status.rawValue)."
            googleCode = ""
            googleCallbackError = ""
            googleErrorDescription = ""
        }
    }

    func loadPreferences() async {
        await run(action: .loadPreferences, retryAction: .loadPreferences) { [self] in
            let prefs = try await apiClient.getPreferences()
            meetingReminderMinutes = String(prefs.meetingReminderMinutes)
            morningBriefLocalTime = prefs.morningBriefLocalTime
            quietHoursStart = prefs.quietHoursStart
            quietHoursEnd = prefs.quietHoursEnd
            highRiskRequiresConfirm = prefs.highRiskRequiresConfirm
        }
    }

    func savePreferences() async {
        guard let minutes = Int(meetingReminderMinutes.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            errorBanner = ErrorBanner(message: "Meeting reminder minutes must be a whole number.", retryAction: nil, sourceAction: nil)
            return
        }

        let payload = Preferences(
            meetingReminderMinutes: minutes,
            morningBriefLocalTime: morningBriefLocalTime.trimmingCharacters(in: .whitespacesAndNewlines),
            quietHoursStart: quietHoursStart.trimmingCharacters(in: .whitespacesAndNewlines),
            quietHoursEnd: quietHoursEnd.trimmingCharacters(in: .whitespacesAndNewlines),
            highRiskRequiresConfirm: highRiskRequiresConfirm
        )

        await run(action: .savePreferences, retryAction: .savePreferences(payload)) { [self] in
            _ = try await apiClient.updatePreferences(payload)
        }
    }

    func revokeConnector() async {
        let id = connectorID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !id.isEmpty else {
            errorBanner = ErrorBanner(message: "Connector ID is required.", retryAction: nil, sourceAction: nil)
            return
        }

        await run(action: .revokeConnector, retryAction: .revokeConnector(connectorID: id)) { [self] in
            let response = try await apiClient.revokeConnector(connectorID: id)
            revokeStatus = "Connector status: \(response.status.rawValue)."
        }
    }

    func requestDeleteAll() async {
        await run(action: .requestDeleteAll, retryAction: .requestDeleteAll) { [self] in
            let response = try await apiClient.requestDeleteAll()
            deleteAllStatus = "Delete-all status: \(response.status) (request: \(response.requestId))."
        }
    }

    func loadAuditEvents(reset: Bool = false) async {
        await run(action: .loadAuditEvents, retryAction: .loadAuditEvents(reset: reset)) { [self] in
            let cursor = reset ? nil : nextAuditCursor
            let response = try await apiClient.listAuditEvents(cursor: cursor)

            if reset {
                auditEvents = response.items
            } else {
                auditEvents.append(contentsOf: response.items)
            }

            nextAuditCursor = response.nextCursor
        }
    }

    private func run(action: Action, retryAction: RetryAction?, operation: () async throws -> Void) async {
        guard !inFlightActions.contains(action) else {
            return
        }

        inFlightActions.insert(action)
        defer { inFlightActions.remove(action) }

        do {
            try await operation()
            if errorBanner?.sourceAction == action {
                errorBanner = nil
            }
        } catch {
            if case AlfredAPIClientError.unauthorized = error {
                sessionManager.clearSession()
                isAuthenticated = false
                auditEvents = []
                nextAuditCursor = nil
            }

            errorBanner = ErrorBanner(
                message: Self.errorMessage(from: error),
                retryAction: retryAction,
                sourceAction: action
            )
        }
    }
}
