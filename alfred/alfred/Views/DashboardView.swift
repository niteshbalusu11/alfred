import SwiftUI
import AlfredAPIClient

struct DashboardView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                googleSection
                preferencesSection
                privacySection
                activitySection
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
    }

    private var googleSection: some View {
        AppCard {
            AppSectionHeader("Google Connect", subtitle: "Calendar + Gmail permissions") {
                AppStatusBadge(title: googleStatus.title, style: googleStatus.style)
            }

            TextField("Redirect URI", text: $model.redirectURI)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .appFieldStyle()

            Button("Start Google OAuth") {
                Task {
                    await model.startGoogleOAuth()
                }
            }
            .buttonStyle(.appPrimary)
            .disabled(model.isLoading(.startGoogleOAuth))

            if let authURL = URL(string: model.googleAuthURL), !model.googleAuthURL.isEmpty {
                Link("Open Google Consent", destination: authURL)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.accent)
            }

            if !model.googleState.isEmpty {
                Text("State: \(model.googleState)")
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                    .textSelection(.enabled)
            }

            Text("After consent, Alfred completes OAuth automatically when you return to the app.")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)

            if model.isLoading(.startGoogleOAuth) || model.isLoading(.completeGoogleOAuth) {
                ProgressView()
                    .tint(AppTheme.Colors.accent)
            }
        }
    }

    private var preferencesSection: some View {
        AppCard {
            AppSectionHeader("Preferences", subtitle: "Control reminders and quiet hours") {
                AppStatusBadge(title: preferencesStatus.title, style: preferencesStatus.style)
            }

            TextField("Meeting reminder minutes", text: $model.meetingReminderMinutes)
                .keyboardType(.numberPad)
                .appFieldStyle()

            TextField("Morning brief local time (HH:mm)", text: $model.morningBriefLocalTime)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .appFieldStyle()

            TextField("Quiet hours start (HH:mm)", text: $model.quietHoursStart)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .appFieldStyle()

            TextField("Quiet hours end (HH:mm)", text: $model.quietHoursEnd)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .appFieldStyle()

            Toggle("High-risk actions require confirmation", isOn: $model.highRiskRequiresConfirm)
                .tint(AppTheme.Colors.accent)

            HStack(spacing: 12) {
                Button("Load") {
                    Task {
                        await model.loadPreferences()
                    }
                }
                .buttonStyle(.appSecondary)
                .disabled(model.isLoading(.loadPreferences))

                Button("Save") {
                    Task {
                        await model.savePreferences()
                    }
                }
                .buttonStyle(.appPrimary)
                .disabled(model.isLoading(.savePreferences))
            }

            if model.isLoading(.loadPreferences) || model.isLoading(.savePreferences) {
                ProgressView()
                    .tint(AppTheme.Colors.accent)
            }
        }
    }

    private var privacySection: some View {
        AppCard {
            AppSectionHeader("Privacy", subtitle: "Revoke access or delete data") {
                AppStatusBadge(title: privacyStatus.title, style: privacyStatus.style)
            }

            TextField("Connector ID", text: $model.connectorID)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .appFieldStyle()

            Button("Revoke Connector") {
                Task {
                    await model.revokeConnector()
                }
            }
            .buttonStyle(.appSecondary)
            .disabled(model.isLoading(.revokeConnector))

            if !model.revokeStatus.isEmpty {
                Text(model.revokeStatus)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            Button("Request Delete All") {
                Task {
                    await model.requestDeleteAll()
                }
            }
            .buttonStyle(.appPrimary)
            .disabled(model.isLoading(.requestDeleteAll))

            if !model.deleteAllStatus.isEmpty {
                Text(model.deleteAllStatus)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            if model.isLoading(.revokeConnector) || model.isLoading(.requestDeleteAll) {
                ProgressView()
                    .tint(AppTheme.Colors.accent)
            }
        }
    }

    private var activitySection: some View {
        AppCard {
            AppSectionHeader("Activity Log", subtitle: "Recent events and outcomes") {
                AppStatusBadge(title: activityStatus.title, style: activityStatus.style)
            }

            Button("Refresh Activity") {
                Task {
                    await model.loadAuditEvents(reset: true)
                }
            }
            .buttonStyle(.appSecondary)
            .disabled(model.isLoading(.loadAuditEvents))

            if model.auditEvents.isEmpty {
                Text("No events yet.")
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            } else {
                ForEach(model.auditEvents, id: \.id) { event in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(event.eventType)
                            .font(.headline)
                            .foregroundStyle(AppTheme.Colors.textPrimary)
                        Text(event.timestamp.formatted(date: .abbreviated, time: .shortened))
                            .font(.footnote)
                            .foregroundStyle(AppTheme.Colors.textSecondary)
                        Text("Result: \(event.result)")
                            .font(.footnote)
                            .foregroundStyle(AppTheme.Colors.textSecondary)
                    }
                    .padding(.vertical, 6)
                }
            }

            if model.canLoadMoreAuditEvents {
                Button("Load More") {
                    Task {
                        await model.loadAuditEvents(reset: false)
                    }
                }
                .buttonStyle(.appSecondary)
                .disabled(model.isLoading(.loadAuditEvents))
            }

            if model.isLoading(.loadAuditEvents) {
                ProgressView()
                    .tint(AppTheme.Colors.accent)
            }
        }
    }

    private var googleStatus: (title: String, style: AppStatusBadge.Style) {
        if model.isLoading(.startGoogleOAuth) || model.isLoading(.completeGoogleOAuth) {
            return ("Connecting", .warning)
        }

        if !model.connectorID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return ("Connected", .success)
        }

        if !model.googleState.isEmpty {
            return ("Pending", .warning)
        }

        return ("Not connected", .neutral)
    }

    private var preferencesStatus: (title: String, style: AppStatusBadge.Style) {
        if model.isLoading(.savePreferences) {
            return ("Saving", .warning)
        }

        return ("Ready", .neutral)
    }

    private var privacyStatus: (title: String, style: AppStatusBadge.Style) {
        if model.isLoading(.revokeConnector) || model.isLoading(.requestDeleteAll) {
            return ("Processing", .warning)
        }

        return ("Ready", .neutral)
    }

    private var activityStatus: (title: String, style: AppStatusBadge.Style) {
        if model.isLoading(.loadAuditEvents) {
            return ("Loading", .warning)
        }

        if model.auditEvents.isEmpty {
            return ("Empty", .neutral)
        }

        return ("Updated", .success)
    }
}
