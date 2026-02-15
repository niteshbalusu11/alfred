import AlfredAPIClient
import SwiftUI

struct HomeView: View {
    @ObservedObject var model: AppModel

    private var isHomeLoading: Bool {
        model.isLoading(.loadPreferences)
            || model.isLoading(.loadAuditEvents)
            || model.isLoading(.startGoogleOAuth)
            || model.isLoading(.completeGoogleOAuth)
    }

    private var hasConnector: Bool {
        !model.connectorID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var homeErrorBanner: AppModel.ErrorBanner? {
        guard let banner = model.errorBanner, let source = banner.sourceAction else { return nil }
        let homeActions: Set<AppModel.Action> = [
            .loadPreferences,
            .loadAuditEvents,
            .startGoogleOAuth,
            .completeGoogleOAuth
        ]
        return homeActions.contains(source) ? banner : nil
    }

    private var assistantBadge: (title: String, style: AppStatusBadge.Style) {
        if homeErrorBanner != nil {
            return ("Needs attention", .danger)
        }
        if isHomeLoading {
            return ("Syncing", .warning)
        }
        return hasConnector ? ("Live", .success) : ("Setup needed", .warning)
    }

    private var lastActivityTimestamp: String? {
        model.auditEvents.first?.timestamp.formatted(date: .abbreviated, time: .shortened)
    }

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                summarySection
                statusCardsSection
                quickActionsSection
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
    }

    private var summarySection: some View {
        AppCard {
            AppSectionHeader("Today's Command Center", subtitle: "Assistant status at a glance") {
                AppStatusBadge(title: assistantBadge.title, style: assistantBadge.style)
            }

            if isHomeLoading {
                HomeLoadingStateView()
            } else if let banner = homeErrorBanner {
                HomeInlineErrorView(
                    message: banner.message,
                    onRetry: banner.retryAction == nil ? nil : {
                        Task {
                            await model.retryLastAction()
                        }
                    },
                    onDismiss: {
                        model.dismissError()
                    }
                )
            } else if !hasConnector {
                HomeEmptyStateView(
                    title: "Connect Google to activate Alfred",
                    subtitle: "Link your account to enable reminders, daily briefs, and urgent alerts.",
                    actionTitle: "Go to Connectors",
                    action: { model.selectedTab = .connectors }
                )
            } else {
                HomeSummaryRow(title: "Connectors", status: model.googleStatusBadge)
                HomeSummaryRow(title: "Preferences", status: model.preferencesStatusBadge)
                HomeSummaryRow(title: "Privacy", status: model.privacyStatusBadge)
                HomeSummaryRow(title: "Activity", status: model.activityStatusBadge)

                if let lastActivityTimestamp {
                    Text("Last activity: \(lastActivityTimestamp)")
                        .font(.footnote)
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                } else {
                    Text("No activity yet. Alfred will show updates once events begin flowing.")
                        .font(.footnote)
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                }
            }
        }
    }

    private var statusCardsSection: some View {
        VStack(alignment: .leading, spacing: AppTheme.Layout.sectionSpacing) {
            AppSectionHeader("Today's Signals", subtitle: "Reminders, briefs, and urgent alerts")

            if isHomeLoading {
                HomeStatusCardPlaceholder()
                HomeStatusCardPlaceholder()
                HomeStatusCardPlaceholder()
            } else if !hasConnector {
                AppCard {
                    HomeEmptyStateView(
                        title: "Signals are paused",
                        subtitle: "Connect Google to enable reminders and alerting.",
                        actionTitle: "Connect Google",
                        action: { model.selectedTab = .connectors }
                    )
                }
            } else {
                HomeStatusCard(
                    title: "Meeting Reminders",
                    subtitle: "Calendar-driven nudges",
                    status: ("Scheduled", .success),
                    detail: "Remind \(model.meetingReminderMinutes) minutes before meetings.",
                    actionTitle: "Adjust Preferences",
                    action: { model.selectedTab = .profile }
                )

                HomeStatusCard(
                    title: "Morning Brief",
                    subtitle: "Daily summary delivery",
                    status: ("Scheduled", .success),
                    detail: "Arrives at \(model.morningBriefLocalTime) local time.",
                    actionTitle: "Change Brief Time",
                    action: { model.selectedTab = .profile }
                )

                HomeStatusCard(
                    title: "Urgent Alerts",
                    subtitle: "High-signal email detection",
                    status: (model.highRiskRequiresConfirm ? "Confirming" : "Auto-send", .warning),
                    detail: model.highRiskRequiresConfirm
                        ? "High-risk alerts require confirmation."
                        : "Urgent alerts send automatically.",
                    actionTitle: "Review Settings",
                    action: { model.selectedTab = .profile }
                )
            }
        }
    }

    private var quickActionsSection: some View {
        AppCard {
            AppSectionHeader("Quick Actions", subtitle: "Jump to the most-used areas")

            if !hasConnector {
                Button("Connect Google") {
                    model.selectedTab = .connectors
                }
                .buttonStyle(.appPrimary)
            } else {
                Button("Refresh Home") {
                    Task {
                        await model.loadPreferences()
                        await model.loadAuditEvents(reset: true)
                    }
                }
                .buttonStyle(.appPrimary)
                .disabled(isHomeLoading)
            }

            Button("View Activity") {
                model.selectedTab = .activity
            }
            .buttonStyle(.appSecondary)

            Button("Profile & Preferences") {
                model.selectedTab = .profile
            }
            .buttonStyle(.appSecondary)

            Button("Open Connectors") {
                model.selectedTab = .connectors
            }
            .buttonStyle(.appSecondary)
        }
    }
}

private struct HomeSummaryRow: View {
    let title: String
    let status: (title: String, style: AppStatusBadge.Style)

    var body: some View {
        HStack {
            Text(title)
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Spacer()

            AppStatusBadge(title: status.title, style: status.style)
        }
        .padding(.vertical, 6)
    }
}

private struct HomeLoadingStateView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                ProgressView()
                    .tint(AppTheme.Colors.accent)
                Text("Syncing your assistant status…")
                    .font(.subheadline)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            Text("Loading connector health, preferences, and activity.")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .redacted(reason: .placeholder)

            Text("Loading schedule signals.")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .redacted(reason: .placeholder)
        }
    }
}

private struct HomeInlineErrorView: View {
    let message: String
    let onRetry: (() -> Void)?
    let onDismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            AppStatusBadge(title: "Action needed", style: .danger)

            Text(message)
                .font(.subheadline)
                .foregroundStyle(AppTheme.Colors.textPrimary)

            HStack(spacing: 12) {
                if let onRetry {
                    Button("Retry", action: onRetry)
                        .buttonStyle(.appPrimary)
                }

                Button("Dismiss", action: onDismiss)
                    .buttonStyle(.appSecondary)
            }
        }
        .padding(12)
        .background(AppTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(AppTheme.Colors.danger, lineWidth: 1)
        )
    }
}

private struct HomeEmptyStateView: View {
    let title: String
    let subtitle: String
    let actionTitle: String
    let action: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.headline)
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Text(subtitle)
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)

            Button(actionTitle, action: action)
                .buttonStyle(.appSecondary)
        }
    }
}

private struct HomeStatusCard: View {
    let title: String
    let subtitle: String
    let status: (title: String, style: AppStatusBadge.Style)
    let detail: String
    let actionTitle: String
    let action: () -> Void

    var body: some View {
        AppCard {
            AppSectionHeader(title, subtitle: subtitle) {
                AppStatusBadge(title: status.title, style: status.style)
            }

            Text(detail)
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)

            Button(actionTitle, action: action)
                .buttonStyle(.appSecondary)
        }
    }
}

private struct HomeStatusCardPlaceholder: View {
    var body: some View {
        AppCard {
            VStack(alignment: .leading, spacing: 8) {
                Text("Loading signal")
                    .font(.headline)
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .redacted(reason: .placeholder)

                Text("Loading details…")
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                    .redacted(reason: .placeholder)

                ProgressView()
                    .tint(AppTheme.Colors.accent)
            }
        }
    }
}

#Preview {
    HomeView(model: AppModel())
}
