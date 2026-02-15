import SwiftUI

struct ConnectorsView: View {
    @ObservedObject var model: AppModel

    private struct FutureConnector: Identifiable {
        let id: String
        let title: String
        let subtitle: String
    }

    private var hasConnector: Bool {
        !model.connectorID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var hasPendingConsent: Bool {
        !model.googleState.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var hasConsentURL: Bool {
        !model.googleAuthURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var isGoogleActionInFlight: Bool {
        model.isLoading(.startGoogleOAuth)
            || model.isLoading(.completeGoogleOAuth)
            || model.isLoading(.revokeConnector)
    }

    private var connectorsErrorBanner: AppModel.ErrorBanner? {
        guard let banner = model.errorBanner, let source = banner.sourceAction else {
            return nil
        }
        let connectorActions: Set<AppModel.Action> = [
            .startGoogleOAuth,
            .completeGoogleOAuth,
            .revokeConnector
        ]
        return connectorActions.contains(source) ? banner : nil
    }

    private var hubStatusBadge: (title: String, style: AppStatusBadge.Style) {
        if connectorsErrorBanner != nil {
            return ("Action needed", .danger)
        }
        if isGoogleActionInFlight {
            return ("Syncing", .warning)
        }
        return hasConnector ? ("Operational", .success) : ("Setup pending", .warning)
    }

    private var googleHealthBadge: (title: String, style: AppStatusBadge.Style) {
        if connectorsErrorBanner != nil {
            return ("Issue", .danger)
        }
        if isGoogleActionInFlight {
            return ("Syncing", .warning)
        }
        if hasConnector {
            return ("Healthy", .success)
        }
        if hasPendingConsent {
            return ("Awaiting consent", .warning)
        }
        return ("Not configured", .neutral)
    }

    private var googleActionTitle: String {
        hasConnector ? "Reconnect Google" : "Connect Google"
    }

    private var futureConnectors: [FutureConnector] {
        [
            FutureConnector(id: "microsoft", title: "Microsoft 365", subtitle: "Calendar + Outlook"),
            FutureConnector(id: "slack", title: "Slack", subtitle: "Priority message alerts"),
            FutureConnector(id: "notion", title: "Notion", subtitle: "Tasks and project status")
        ]
    }

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                summarySection
                if let banner = connectorsErrorBanner {
                    errorSection(banner: banner)
                }
                googleSection
                futureSection
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
    }

    private var summarySection: some View {
        AppCard {
            AppSectionHeader("Connectors Hub", subtitle: "Connection health and provider controls") {
                AppStatusBadge(title: hubStatusBadge.title, style: hubStatusBadge.style)
            }

            ConnectorSignalRow(title: "Connected providers", value: hasConnector ? "1 of 1 live" : "0 of 1 live")
            ConnectorSignalRow(
                title: "Google health",
                value: googleHealthBadge.title,
                valueStyle: googleHealthBadge.style
            )
            ConnectorSignalRow(
                title: "Expansion readiness",
                value: "Future providers can be added without redesign"
            )
        }
    }

    private func errorSection(banner: AppModel.ErrorBanner) -> some View {
        AppCard {
            AppSectionHeader("Connector issue", subtitle: "Review and retry") {
                AppStatusBadge(title: "Needs attention", style: .danger)
            }

            Text(banner.message)
                .font(.subheadline)
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Text("If this looks transient or network-related, retry first. If it persists, reconnect Google.")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)

            HStack(spacing: 12) {
                if banner.retryAction != nil {
                    Button("Retry") {
                        Task {
                            await model.retryLastAction()
                        }
                    }
                    .buttonStyle(.appPrimary)
                }

                Button("Dismiss") {
                    model.dismissError()
                }
                .buttonStyle(.appSecondary)
            }
        }
    }

    private var googleSection: some View {
        AppCard {
            AppSectionHeader("Google Connect", subtitle: "Calendar + Gmail permissions") {
                AppStatusBadge(title: model.googleStatusBadge.title, style: model.googleStatusBadge.style)
            }

            ConnectorSignalRow(
                title: "Connector health",
                value: googleHealthBadge.title,
                valueStyle: googleHealthBadge.style
            )

            Button(googleActionTitle) {
                Task {
                    await model.startGoogleOAuth()
                }
            }
            .buttonStyle(.appPrimary)
            .disabled(isGoogleActionInFlight)

            if let authURL = URL(string: model.googleAuthURL), hasConsentURL {
                Link("Open Google Consent Screen", destination: authURL)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.accent)
            }

            if hasPendingConsent {
                Text("Consent is pending. Return here after Google approval and Alfred will finish setup automatically.")
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            } else if !hasConnector {
                Text("No active Google connector yet. Connect to enable reminders and assistant signals.")
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            if hasConnector {
                Button("Revoke Google Access") {
                    Task {
                        await model.revokeConnector()
                    }
                }
                .buttonStyle(.appSecondary)
                .disabled(isGoogleActionInFlight)
            }

            if !model.connectorID.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Connector ID")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(AppTheme.Colors.textSecondary)

                    Text(model.connectorID)
                        .font(.footnote.monospaced())
                        .foregroundStyle(AppTheme.Colors.textPrimary)
                        .textSelection(.enabled)
                }
            }

            if !model.revokeStatus.isEmpty {
                Text(model.revokeStatus)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            Text("Redirect URI: \(model.redirectURI)")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)

            if isGoogleActionInFlight {
                ProgressView()
                    .tint(AppTheme.Colors.accent)
            }
        }
    }

    private var futureSection: some View {
        AppCard {
            AppSectionHeader("More Connectors", subtitle: "Additional providers are coming soon")

            ForEach(futureConnectors) { connector in
                FutureConnectorRow(title: connector.title, subtitle: connector.subtitle)
            }
        }
    }
}

private struct ConnectorSignalRow: View {
    let title: String
    let value: String
    var valueStyle: AppStatusBadge.Style?

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Text(title)
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Spacer(minLength: 12)

            if let valueStyle {
                AppStatusBadge(title: value, style: valueStyle)
            } else {
                Text(value)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                    .multilineTextAlignment(.trailing)
            }
        }
        .padding(.vertical, 4)
    }
}

private struct FutureConnectorRow: View {
    let title: String
    let subtitle: String

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)

                Text(subtitle)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            Spacer(minLength: 12)

            AppStatusBadge(title: "Planned", style: .neutral)
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
    ConnectorsView(model: AppModel())
}
