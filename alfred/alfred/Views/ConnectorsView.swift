import SwiftUI

struct ConnectorsView: View {
    @Environment(\.openURL) private var openURL
    @ObservedObject var model: AppModel
    @State private var showConnectConfirmation = false

    private var hasConnector: Bool {
        !model.connectorID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var hasPendingConsent: Bool {
        !model.googleState.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var isGoogleActionInFlight: Bool {
        model.isLoading(.startGoogleOAuth)
            || model.isLoading(.completeGoogleOAuth)
    }

    private var isToggleOn: Bool {
        hasConnector || hasPendingConsent || isGoogleActionInFlight
    }

    private var shouldDisableToggle: Bool {
        hasPendingConsent || isGoogleActionInFlight
    }

    private var googleConnectToggle: Binding<Bool> {
        Binding(
            get: { isToggleOn },
            set: { wantsOn in
                guard wantsOn else { return }
                guard !shouldDisableToggle else { return }
                showConnectConfirmation = true
            }
        )
    }

    private var helperText: String {
        if hasPendingConsent {
            return "Finish sign-in in your browser, then return to Alfred."
        }
        if isGoogleActionInFlight {
            return "Starting Google sign-in..."
        }
        if hasConnector {
            return "You're all set."
        }
        return "Turn on Google and continue to sign in from your browser."
    }

    private var pendingConsentURL: URL? {
        let trimmedURL = model.googleAuthURL.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedURL.isEmpty else {
            return nil
        }
        return URL(string: trimmedURL)
    }

    var body: some View {
        ScrollView {
            VStack(spacing: AppTheme.Layout.sectionSpacing) {
                googleConnectorCard
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
        .alert("Continue signing in with Google?", isPresented: $showConnectConfirmation) {
            Button("Cancel", role: .cancel) {}
            Button("Continue") {
                startGoogleConnect()
            }
        } message: {
            Text("Hey do you want to continue signing in with Google?")
        }
    }

    private var googleConnectorCard: some View {
        AppCard {
            VStack(alignment: .leading, spacing: 12) {
                AppSectionHeader("Google Connector", subtitle: "Simple browser-based sign-in")

                Toggle(isOn: googleConnectToggle) {
                    HStack(spacing: 8) {
                        Text("Google")
                            .font(.headline)
                            .foregroundStyle(AppTheme.Colors.textPrimary)

                        if hasConnector {
                            Image(systemName: "checkmark.circle.fill")
                                .foregroundStyle(AppTheme.Colors.success)
                                .font(.subheadline.weight(.bold))
                        }
                    }
                }
                .toggleStyle(.switch)
                .disabled(shouldDisableToggle)

                Text(helperText)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)

                if hasPendingConsent, pendingConsentURL != nil {
                    Button("Open Browser Again") {
                        openConsentURLIfAvailable()
                    }
                    .buttonStyle(.appSecondary)
                }

                if isGoogleActionInFlight {
                    ProgressView()
                        .tint(AppTheme.Colors.accent)
                }
            }
        }
    }

    private func startGoogleConnect() {
        Task { @MainActor in
            await model.startGoogleOAuth()
            if model.errorBanner?.sourceAction == .startGoogleOAuth {
                AppLogger.warning("Google OAuth start failed; browser handoff skipped.", category: .oauth)
                return
            }
            openConsentURLIfAvailable()
        }
    }

    private func openConsentURLIfAvailable() {
        guard let authURL = pendingConsentURL else {
            AppLogger.warning("Google OAuth URL unavailable for browser handoff.", category: .oauth)
            return
        }
        AppLogger.info("Opening Google OAuth consent in browser.", category: .oauth)
        openURL(authURL)
    }
}

#Preview {
    ConnectorsView(model: AppModel())
}
