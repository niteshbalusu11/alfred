import SwiftUI

struct ProfileView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                preferencesSection
                privacySection
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
    }

    private var preferencesSection: some View {
        AppCard {
            AppSectionHeader("Preferences", subtitle: "Control reminders and quiet hours") {
                AppStatusBadge(title: model.preferencesStatusBadge.title, style: model.preferencesStatusBadge.style)
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
                AppStatusBadge(title: model.privacyStatusBadge.title, style: model.privacyStatusBadge.style)
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
}

#Preview {
    ProfileView(model: AppModel())
}
