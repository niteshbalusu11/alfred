import SwiftUI
import AlfredAPIClient

struct DashboardView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        Form {
            googleSection
            preferencesSection
            privacySection
            activitySection
        }
    }

    private var googleSection: some View {
        Section("Google Connect") {
            TextField("Redirect URI", text: $model.redirectURI)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()

            Button("Start Google OAuth") {
                Task {
                    await model.startGoogleOAuth()
                }
            }
            .disabled(model.isLoading(.startGoogleOAuth))

            if let authURL = URL(string: model.googleAuthURL), !model.googleAuthURL.isEmpty {
                Link("Open Google Consent", destination: authURL)
                    .font(.footnote)
            }

            if !model.googleState.isEmpty {
                Text("State: \(model.googleState)")
                    .font(.footnote)
                    .textSelection(.enabled)
            }

            Text("After consent, Alfred completes OAuth automatically when you return to the app.")
                .font(.footnote)
                .foregroundStyle(.secondary)

            if model.isLoading(.startGoogleOAuth) || model.isLoading(.completeGoogleOAuth) {
                ProgressView()
            }
        }
    }

    private var preferencesSection: some View {
        Section("Preferences") {
            TextField("Meeting reminder minutes", text: $model.meetingReminderMinutes)
                .keyboardType(.numberPad)

            TextField("Morning brief local time (HH:mm)", text: $model.morningBriefLocalTime)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()

            TextField("Quiet hours start (HH:mm)", text: $model.quietHoursStart)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()

            TextField("Quiet hours end (HH:mm)", text: $model.quietHoursEnd)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()

            Toggle("High-risk actions require confirmation", isOn: $model.highRiskRequiresConfirm)

            HStack {
                Button("Load") {
                    Task {
                        await model.loadPreferences()
                    }
                }
                .disabled(model.isLoading(.loadPreferences))

                Button("Save") {
                    Task {
                        await model.savePreferences()
                    }
                }
                .disabled(model.isLoading(.savePreferences))
            }

            if model.isLoading(.loadPreferences) || model.isLoading(.savePreferences) {
                ProgressView()
            }
        }
    }

    private var privacySection: some View {
        Section("Privacy") {
            TextField("Connector ID", text: $model.connectorID)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()

            Button("Revoke Connector") {
                Task {
                    await model.revokeConnector()
                }
            }
            .disabled(model.isLoading(.revokeConnector))

            if !model.revokeStatus.isEmpty {
                Text(model.revokeStatus)
                    .font(.footnote)
            }

            Button("Request Delete All") {
                Task {
                    await model.requestDeleteAll()
                }
            }
            .disabled(model.isLoading(.requestDeleteAll))

            if !model.deleteAllStatus.isEmpty {
                Text(model.deleteAllStatus)
                    .font(.footnote)
            }

            if model.isLoading(.revokeConnector) || model.isLoading(.requestDeleteAll) {
                ProgressView()
            }
        }
    }

    private var activitySection: some View {
        Section("Activity Log") {
            Button("Refresh Activity") {
                Task {
                    await model.loadAuditEvents(reset: true)
                }
            }
            .disabled(model.isLoading(.loadAuditEvents))

            if model.auditEvents.isEmpty {
                Text("No events yet.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(model.auditEvents, id: \.id) { event in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(event.eventType)
                            .font(.headline)
                        Text(event.timestamp.formatted(date: .abbreviated, time: .shortened))
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                        Text("Result: \(event.result)")
                            .font(.footnote)
                    }
                }
            }

            if model.canLoadMoreAuditEvents {
                Button("Load More") {
                    Task {
                        await model.loadAuditEvents(reset: false)
                    }
                }
                .disabled(model.isLoading(.loadAuditEvents))
            }

            if model.isLoading(.loadAuditEvents) {
                ProgressView()
            }
        }
    }
}
