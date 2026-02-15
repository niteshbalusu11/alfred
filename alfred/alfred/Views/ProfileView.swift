import ClerkKit
import SwiftUI

struct ProfileView: View {
    @Environment(Clerk.self) private var clerk
    @ObservedObject var model: AppModel
    @State private var showSignOutConfirmation = false
    @State private var showRevokeConfirmation = false
    @State private var showDeleteConfirmation = false

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                accountSection
                preferencesSection
                privacySection
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
    }

    private var accountSection: some View {
        AppCard {
            AppSectionHeader("Account", subtitle: "Signed-in identity and session") {
                AppStatusBadge(title: accountBadge.title, style: accountBadge.style)
            }

            VStack(alignment: .leading, spacing: 12) {
                if let name = accountDisplayName {
                    Text(name)
                        .font(.title3.weight(.semibold))
                        .foregroundStyle(AppTheme.Colors.textPrimary)
                }

                if let email = accountEmail {
                    Text(email)
                        .font(.subheadline)
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                }

                ProfileInfoRow(title: "Session", value: clerk.user == nil ? "Inactive" : "Active")
                ProfileInfoRow(title: "Account ID", value: accountID ?? "Unavailable")
            }

            Button("Sign out") {
                showSignOutConfirmation = true
            }
            .buttonStyle(.appSecondary)
            .confirmationDialog("Sign out of Alfred?", isPresented: $showSignOutConfirmation) {
                Button("Sign out", role: .destructive) {
                    Task {
                        await model.signOut()
                    }
                }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("You will need to sign in again to manage connectors, preferences, and privacy settings.")
            }
        }
    }

    private var preferencesSection: some View {
        AppCard {
            AppSectionHeader("Preferences", subtitle: "Reminders, briefs, and quiet hours") {
                AppStatusBadge(title: model.preferencesStatusBadge.title, style: model.preferencesStatusBadge.style)
            }

            Text("Tune when Alfred reaches out and how confirmations behave.")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)

            PreferenceField(
                title: "Meeting reminder lead time",
                helper: "Minutes before a meeting starts",
                placeholder: "15",
                text: $model.meetingReminderMinutes,
                keyboard: .numberPad
            )

            PreferenceField(
                title: "Morning brief time",
                helper: "Local time (24-hour format)",
                placeholder: "08:00",
                text: $model.morningBriefLocalTime,
                keyboard: .numbersAndPunctuation
            )

            PreferenceField(
                title: "Quiet hours start",
                helper: "Start time (24-hour format)",
                placeholder: "22:00",
                text: $model.quietHoursStart,
                keyboard: .numbersAndPunctuation
            )

            PreferenceField(
                title: "Quiet hours end",
                helper: "End time (24-hour format)",
                placeholder: "07:00",
                text: $model.quietHoursEnd,
                keyboard: .numbersAndPunctuation
            )

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

            Text(model.preferencesStatus.isEmpty ? "No preference updates yet." : model.preferencesStatus)
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
        }
    }

    private var privacySection: some View {
        AppCard {
            AppSectionHeader("Privacy", subtitle: "Revoke access or delete data") {
                AppStatusBadge(title: model.privacyStatusBadge.title, style: model.privacyStatusBadge.style)
            }

            Text("Privacy actions affect stored data and connected services. Each step includes a confirmation.")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)

            PreferenceField(
                title: "Connector ID",
                helper: "Use the Google connector ID when revoking access",
                placeholder: "connector_xxx",
                text: $model.connectorID,
                keyboard: .default
            )

            PrivacyActionRow(
                title: "Revoke Google access",
                detail: "Stops Alfred from fetching Google data and revokes the connector.",
                buttonTitle: "Revoke Connector",
                isPrimary: false,
                isDisabled: model.isLoading(.revokeConnector)
            ) {
                showRevokeConfirmation = true
            }

            if !model.revokeStatus.isEmpty {
                Text(model.revokeStatus)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            PrivacyActionRow(
                title: "Request delete-all",
                detail: "Deletes stored Alfred data and disconnects all providers. This cannot be undone.",
                buttonTitle: "Request Delete-All",
                isPrimary: true,
                isDisabled: model.isLoading(.requestDeleteAll)
            ) {
                showDeleteConfirmation = true
            }

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
        .confirmationDialog("Revoke Google access?", isPresented: $showRevokeConfirmation) {
            Button("Revoke access", role: .destructive) {
                Task {
                    await model.revokeConnector()
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This disconnects Google and stops reminders and briefs until you reconnect.")
        }
        .confirmationDialog("Request delete-all?", isPresented: $showDeleteConfirmation) {
            Button("Delete all data", role: .destructive) {
                Task {
                    await model.requestDeleteAll()
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This permanently deletes Alfred data and revokes all connectors. This action cannot be undone.")
        }
    }

    private var accountDisplayName: String? {
        let firstName = clerk.user?.firstName?.trimmingCharacters(in: .whitespacesAndNewlines)
        let lastName = clerk.user?.lastName?.trimmingCharacters(in: .whitespacesAndNewlines)
        let username = clerk.user?.username?.trimmingCharacters(in: .whitespacesAndNewlines)

        if let firstName, let lastName, !firstName.isEmpty, !lastName.isEmpty {
            return "\(firstName) \(lastName)"
        }
        if let firstName, !firstName.isEmpty {
            return firstName
        }
        if let lastName, !lastName.isEmpty {
            return lastName
        }
        if let username, !username.isEmpty {
            return username
        }
        return nil
    }

    private var accountEmail: String? {
        clerk.user?.primaryEmailAddress?.emailAddress
    }

    private var accountID: String? {
        clerk.user?.id
    }

    private var accountBadge: (title: String, style: AppStatusBadge.Style) {
        clerk.user == nil ? ("Signed out", .neutral) : ("Active", .success)
    }
}

private struct PreferenceField: View {
    let title: String
    let helper: String?
    let placeholder: String
    let text: Binding<String>
    let keyboard: UIKeyboardType

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            if let helper {
                Text(helper)
                    .font(.caption)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            TextField(placeholder, text: text)
                .keyboardType(keyboard)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .appFieldStyle()
        }
    }
}

private struct ProfileInfoRow: View {
    let title: String
    let value: String

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textSecondary)

            Spacer(minLength: 12)

            Text(value)
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .multilineTextAlignment(.trailing)
        }
    }
}

private struct PrivacyActionRow: View {
    let title: String
    let detail: String
    let buttonTitle: String
    let isPrimary: Bool
    let isDisabled: Bool
    let action: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)

                Text(detail)
                    .font(.caption)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            if isPrimary {
                Button(buttonTitle, action: action)
                    .buttonStyle(.appPrimary)
                    .disabled(isDisabled)
            } else {
                Button(buttonTitle, action: action)
                    .buttonStyle(.appSecondary)
                    .disabled(isDisabled)
            }
        }
        .padding(12)
        .background(AppTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(AppTheme.Colors.outline, lineWidth: 1)
        )
    }
}

#Preview {
    let clerk = Clerk.preview()
    ProfileView(model: AppModel(clerk: clerk))
        .environment(clerk)
}
