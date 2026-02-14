import AlfredAPIClient

extension AppModel {
    func retryLastAction() async {
        guard let retryAction = errorBanner?.retryAction else {
            return
        }

        switch retryAction {
        case .startGoogleOAuth(let redirect):
            redirectURI = redirect
            await startGoogleOAuth()
        case .completeGoogleOAuth(let code, let state, let error, let errorDescription):
            googleCode = code ?? ""
            googleState = state
            googleCallbackError = error ?? ""
            googleErrorDescription = errorDescription ?? ""
            await completeGoogleOAuth()
        case .loadPreferences:
            await loadPreferences()
        case .savePreferences(let payload):
            await savePreferences(payload: payload)
        case .revokeConnector(let id):
            connectorID = id
            await revokeConnector()
        case .requestDeleteAll:
            await requestDeleteAll()
        case .loadAuditEvents(let reset):
            await loadAuditEvents(reset: reset)
        }
    }

    func dismissError() { errorBanner = nil }
}
