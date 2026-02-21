import SwiftUI

extension AppModel {
    var googleStatusBadge: (title: String, style: AppStatusBadge.Style) {
        if isLoading(.startGoogleOAuth) || isLoading(.completeGoogleOAuth) {
            return ("Connecting", .warning)
        }

        if !connectorID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return ("Connected", .success)
        }

        if !googleState.isEmpty {
            return ("Pending", .warning)
        }

        return ("Not connected", .neutral)
    }

    var privacyStatusBadge: (title: String, style: AppStatusBadge.Style) {
        if isLoading(.revokeConnector) || isLoading(.requestDeleteAll) {
            return ("Processing", .warning)
        }

        return ("Ready", .neutral)
    }

    var activityStatusBadge: (title: String, style: AppStatusBadge.Style) {
        if isLoading(.loadAuditEvents) {
            return ("Loading", .warning)
        }

        if auditEvents.isEmpty {
            return ("Empty", .neutral)
        }

        return ("Updated", .success)
    }
}
