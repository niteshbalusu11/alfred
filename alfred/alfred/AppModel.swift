import AlfredAPIClient
import ClerkKit
import Combine
import Foundation

@MainActor
final class AppModel: ObservableObject {
    enum Action: Hashable {
        case startGoogleOAuth
        case completeGoogleOAuth
        case loadConnectors
        case loadPreferences
        case savePreferences
        case revokeConnector
        case requestDeleteAll
        case loadAuditEvents
        case queryAssistant
    }

    enum RetryAction {
        case startGoogleOAuth(redirectURI: String)
        case completeGoogleOAuth(code: String?, state: String, error: String?, errorDescription: String?)
        case loadConnectors
        case loadPreferences
        case savePreferences(Preferences)
        case revokeConnector(connectorID: String)
        case requestDeleteAll
        case loadAuditEvents(reset: Bool)
        case queryAssistant(query: String)
    }

    enum StartupRoute: Equatable {
        case bootstrapping
        case signedOut
        case signedIn
        case authBootstrapFailed(message: String)
    }

    struct ErrorBanner {
        let message: String
        let retryAction: RetryAction?
        let sourceAction: Action?
    }

    @Published private(set) var isAuthenticated = false
    @Published private(set) var startupRoute: StartupRoute = .bootstrapping
    @Published var selectedTab: AppTab = .home
    @Published private(set) var inFlightActions: Set<Action> = []
    @Published var errorBanner: ErrorBanner?

    @Published var redirectURI = "alfred://oauth/google/callback"
    @Published var googleAuthURL = ""
    @Published var googleState = ""
    @Published var googleCode = ""
    @Published var googleCallbackError = ""
    @Published var googleErrorDescription = ""

    @Published var connectorID = ""
    @Published var revokeStatus = ""
    @Published var deleteAllStatus = ""
    @Published var preferencesStatus = ""

    @Published var meetingReminderMinutes = "15"
    @Published var morningBriefLocalTime = "08:00"
    @Published var quietHoursStart = "22:00"
    @Published var quietHoursEnd = "07:00"
    @Published var timeZone = TimeZone.current.identifier
    @Published var highRiskRequiresConfirm = true

    @Published private(set) var auditEvents: [AuditEvent] = []
    @Published private(set) var nextAuditCursor: String?
    @Published private(set) var assistantConversation: [AssistantConversationMessage] = []
    @Published private(set) var assistantResponseText = ""

    let apiBaseURL: URL

    private let clerk: Clerk
    private let apiClient: AlfredAPIClient
    private var authEventsTask: Task<Void, Never>?
    private var lastBootstrappedUserID: String?
    private var assistantSessionID: UUID?

    init(apiBaseURL: URL? = nil, clerk: Clerk? = nil) {
        let clerk = clerk ?? Clerk.shared
        let resolvedAPIBaseURL = apiBaseURL ?? AppConfiguration.defaultAPIBaseURL
        self.apiBaseURL = resolvedAPIBaseURL
        self.clerk = clerk

        let accessTokenProvider = ClerkAccessTokenProvider(tokenSource: clerk.auth)
        self.apiClient = AlfredAPIClient(
            baseURL: resolvedAPIBaseURL,
            tokenProvider: {
                try await accessTokenProvider.accessToken()
            }
        )

        startAuthEventObserver()
    }

    deinit {
        authEventsTask?.cancel()
    }

    var canLoadMoreAuditEvents: Bool { nextAuditCursor != nil }
    func isLoading(_ action: Action) -> Bool { inFlightActions.contains(action) }

    func signOut() async {
        do {
            try await clerk.auth.signOut()
        } catch {
            AppLogger.error("Sign-out failed.", category: .auth)
            errorBanner = ErrorBanner(
                message: Self.errorMessage(from: error),
                retryAction: nil,
                sourceAction: nil
            )
        }

        resetAuthenticationState()
        resetGoogleOAuthState()
        resetRequestStatusState()
    }

    func retryAuthBootstrap(showLoadingState: Bool = true) async {
        if showLoadingState {
            startupRoute = .bootstrapping
        }
        clearAuthBootstrapErrorBannerIfNeeded()
        await synchronizeAuthenticationState(shouldLoadData: true)
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
            // OAuth state/auth URL are one-time values; clear them once connect completes.
            googleAuthURL = ""
            googleState = ""
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
            timeZone = prefs.timeZone
            highRiskRequiresConfirm = prefs.highRiskRequiresConfirm
            preferencesStatus = "Preferences synced."
        }
    }

    func loadConnectors() async {
        await run(action: .loadConnectors, retryAction: .loadConnectors) { [self] in
            let response = try await apiClient.listConnectors()
            applyConnectorSnapshot(response)
        }
    }

    func applyConnectorSnapshot(_ response: ListConnectorsResponse) {
        let googleConnector = response.items.first { $0.provider == "google" }
        resetGoogleOAuthState()

        guard let googleConnector else {
            connectorID = ""
            revokeStatus = ""
            return
        }

        switch googleConnector.status {
        case .active:
            connectorID = googleConnector.connectorId
            revokeStatus = "Connector status: \(googleConnector.status.rawValue)."
        case .revoked:
            connectorID = ""
            revokeStatus = "Connector status: \(googleConnector.status.rawValue)."
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
            timeZone: normalizedTimeZoneIdentifier(from: timeZone),
            highRiskRequiresConfirm: highRiskRequiresConfirm
        )

        await savePreferences(payload: payload)
    }

    func savePreferences(payload: Preferences) async {
        await run(action: .savePreferences, retryAction: .savePreferences(payload)) { [self] in
            _ = try await apiClient.updatePreferences(payload)
            preferencesStatus = "Preferences saved."
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
            if response.status == .revoked {
                connectorID = ""
                googleAuthURL = ""
                googleState = ""
            }
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

    func queryAssistant(query: String) async {
        let trimmedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedQuery.isEmpty else {
            errorBanner = ErrorBanner(
                message: "Voice transcript is empty. Record something first.",
                retryAction: nil,
                sourceAction: nil
            )
            return
        }

        await run(action: .queryAssistant, retryAction: .queryAssistant(query: trimmedQuery)) { [self] in
            let response = try await apiClient.queryAssistantEncrypted(
                query: trimmedQuery,
                sessionId: assistantSessionID,
                attestationConfig: AppConfiguration.assistantAttestationVerificationConfig
            )
            assistantSessionID = response.sessionId
            assistantConversation.append(
                AssistantConversationMapper.userMessage(from: trimmedQuery)
            )

            let assistantMessage = AssistantConversationMapper.assistantMessage(from: response)
            assistantConversation.append(assistantMessage)
            assistantResponseText = assistantMessage.text
        }
    }

    func clearAssistantConversation() {
        assistantConversation = []
        assistantResponseText = ""
        assistantSessionID = nil
    }

    private func run(action: Action, retryAction: RetryAction?, operation: () async throws -> Void) async {
        guard !inFlightActions.contains(action) else {
            AppLogger.debug("Skipped duplicate action \(String(describing: action)).", category: .app)
            return
        }

        AppLogger.debug("Starting action \(String(describing: action)).", category: .app)
        inFlightActions.insert(action)
        defer { inFlightActions.remove(action) }

        do {
            try await operation()
            AppLogger.debug("Completed action \(String(describing: action)).", category: .app)
            if errorBanner?.sourceAction == action {
                errorBanner = nil
            }
        } catch {
            if case AlfredAPIClientError.unauthorized = error {
                AppLogger.warning(
                    "Unauthorized during action \(String(describing: action)). Resetting auth session.",
                    category: .auth
                )
                try? await clerk.auth.signOut()
                resetAuthenticationState()
                resetGoogleOAuthState()
                resetRequestStatusState()
            } else {
                AppLogger.error(
                    "Action \(String(describing: action)) failed with \(String(describing: type(of: error))).",
                    category: .network
                )
            }

            errorBanner = ErrorBanner(
                message: Self.errorMessage(from: error),
                retryAction: retryAction,
                sourceAction: action
            )
        }
    }

    private func normalizedTimeZoneIdentifier(from value: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            return TimeZone.current.identifier
        }
        return TimeZone(identifier: trimmed)?.identifier ?? TimeZone.current.identifier
    }

    private func startAuthEventObserver() {
        authEventsTask?.cancel()
        authEventsTask = Task { [weak self] in
            guard let self else { return }

            await self.retryAuthBootstrap()
            for await event in self.clerk.auth.events {
                switch event {
                case .signInCompleted, .signUpCompleted:
                    AppLogger.info("Authentication completed.", category: .auth)
                    await self.retryAuthBootstrap()
                case .sessionChanged(_, let newSession):
                    AppLogger.debug(
                        "Auth session changed. Active session: \(newSession != nil).",
                        category: .auth
                    )
                    if newSession != nil {
                        let currentUserID = self.clerk.user?.id
                        if self.shouldSkipSessionRefresh(for: currentUserID) {
                            AppLogger.debug(
                                "Ignored redundant auth session refresh for current user.",
                                category: .auth
                            )
                            continue
                        }
                        await self.retryAuthBootstrap(showLoadingState: false)
                    } else {
                        self.resetAuthenticationState()
                        self.resetGoogleOAuthState()
                        self.resetRequestStatusState()
                    }
                case .signedOut:
                    AppLogger.info("Signed out.", category: .auth)
                    self.resetAuthenticationState()
                    self.resetGoogleOAuthState()
                    self.resetRequestStatusState()
                case .tokenRefreshed:
                    AppLogger.debug("Auth token refreshed.", category: .auth)
                    break
                }
            }
        }
    }

    private func synchronizeAuthenticationState(shouldLoadData: Bool) async {
        let isCurrentlyAuthenticated = clerk.user != nil
        let wasAuthenticated = isAuthenticated
        isAuthenticated = isCurrentlyAuthenticated

        guard isCurrentlyAuthenticated else {
            if wasAuthenticated {
                resetAuthenticationState()
                resetGoogleOAuthState()
                resetRequestStatusState()
            } else {
                startupRoute = .signedOut
            }
            return
        }

        if shouldLoadData || !wasAuthenticated {
            clearAuthBootstrapErrorBannerIfNeeded()
            await loadConnectors()
            await loadPreferences()
            await loadAuditEvents(reset: true)

            if clerk.user == nil {
                startupRoute = .signedOut
                return
            }

            if hasAuthBootstrapFailure {
                startupRoute = .authBootstrapFailed(message: errorBanner?.message ?? "Authentication bootstrap failed.")
                return
            }
        }

        lastBootstrappedUserID = clerk.user?.id
        startupRoute = .signedIn
    }

    private func resetAuthenticationState() {
        isAuthenticated = false
        startupRoute = .signedOut
        lastBootstrappedUserID = nil
        connectorID = ""
        auditEvents = []
        nextAuditCursor = nil
    }

    private func shouldSkipSessionRefresh(for userID: String?) -> Bool {
        guard case .signedIn = startupRoute else { return false }
        guard isAuthenticated else { return false }
        guard !hasAuthBootstrapFailure else { return false }
        return userID == lastBootstrappedUserID
    }

    private func resetGoogleOAuthState() {
        googleAuthURL = ""
        googleState = ""
        googleCode = ""
        googleCallbackError = ""
        googleErrorDescription = ""
    }

    private func resetRequestStatusState() {
        deleteAllStatus = ""
        revokeStatus = ""
        preferencesStatus = ""
        assistantConversation = []
        assistantResponseText = ""
        assistantSessionID = nil
    }

    private func clearAuthBootstrapErrorBannerIfNeeded() {
        guard let sourceAction = errorBanner?.sourceAction else { return }
        if sourceAction == .loadConnectors || sourceAction == .loadPreferences || sourceAction == .loadAuditEvents {
            errorBanner = nil
        }
    }

    private var hasAuthBootstrapFailure: Bool {
        guard let sourceAction = errorBanner?.sourceAction else { return false }
        return sourceAction == .loadConnectors || sourceAction == .loadPreferences || sourceAction == .loadAuditEvents
    }
}
