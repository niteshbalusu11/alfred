import SwiftUI

struct ConnectorsView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                googleSection
                futureSection
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
    }

    private var googleSection: some View {
        AppCard {
            AppSectionHeader("Google Connect", subtitle: "Calendar + Gmail permissions") {
                AppStatusBadge(title: model.googleStatusBadge.title, style: model.googleStatusBadge.style)
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

    private var futureSection: some View {
        AppCard {
            AppSectionHeader("More Connectors", subtitle: "Additional providers are coming soon")

            Text("We’ll add new integrations here without changing the layout.")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
        }
    }
}

#Preview {
    ConnectorsView(model: AppModel())
}
