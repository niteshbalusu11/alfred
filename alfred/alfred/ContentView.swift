import AlfredAPIClient
import Combine
import ClerkKit
import ClerkKitUI
import SwiftUI

struct ContentView: View {
    @ObservedObject var model: AppModel
    @State private var authIsPresented = false
    private let outputHistoryStore = AutomationOutputHistoryStore()

    var body: some View {
        ZStack(alignment: .top) {
            Group {
                if case .bootstrapping = model.startupRoute {
                    StartupBootstrapView()
                } else if case .signedOut = model.startupRoute {
                    StartupSignedOutView(
                        apiBaseURL: model.apiBaseURL,
                        onOpenAuth: { presentAuthFlow() }
                    )
                } else if case .signedIn = model.startupRoute {
                    AppTabShellView(model: model)
                } else if case .authBootstrapFailed(let message) = model.startupRoute {
                    StartupAuthBootstrapFailureView(
                        message: message,
                        onRetry: { retryAuthBootstrap() },
                        onSignOut: { signOut() }
                    )
                }
            }
            .appScreenBackground()

            if let banner = model.errorBanner {
                ErrorBannerView(
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
                .padding()
            }
        }
        .sheet(isPresented: $authIsPresented) {
            AuthView()
        }
        .onChange(of: model.startupRoute, initial: false) { _, route in
            if case .signedIn = route {
                authIsPresented = false
                routeToPendingAutomationOutputIfNeeded()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: PushNotificationEvents.didOpenAutomationNotification)) { _ in
            if case .signedIn = model.startupRoute {
                model.selectedTab = .automations
            }
            routeToPendingAutomationOutputIfNeeded()
        }
        .task {
            routeToPendingAutomationOutputIfNeeded()
        }
        .onOpenURL { url in
            Task {
                await model.handleOAuthCallbackURL(url)
            }
        }
    }

    private func presentAuthFlow() {
        authIsPresented = true
    }

    private func retryAuthBootstrap() {
        Task {
            await model.retryAuthBootstrap()
        }
    }

    private func signOut() {
        Task {
            await model.signOut()
        }
    }

    private func routeToPendingAutomationOutputIfNeeded() {
        Task { @MainActor in
            guard case .signedIn = model.startupRoute else {
                return
            }
            let pendingRequestID = try? await outputHistoryStore.peekPendingOpenRequestID()
            if pendingRequestID != nil {
                model.selectedTab = .automations
            }
        }
    }
}

private struct ErrorBannerView: View {
    let message: String
    let onRetry: (() -> Void)?
    let onDismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            AppStatusBadge(title: "Action needed", style: .danger)

            Text(message)
                .font(.subheadline)
                .foregroundStyle(AppTheme.Colors.textPrimary)

            HStack {
                if let onRetry {
                    Button("Retry", action: onRetry)
                        .buttonStyle(.appPrimary)
                }

                Button("Dismiss", action: onDismiss)
                    .buttonStyle(.appSecondary)
            }
        }
        .padding()
        .background(AppTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(AppTheme.Colors.danger, lineWidth: 1)
        )
    }
}

#Preview("Signed Out") {
    let clerk = Clerk.preview { preview in
        preview.isSignedIn = false
    }
    ContentView(model: AppModel(clerk: clerk))
        .environment(clerk)
}

#Preview("Signed In") {
    let clerk = Clerk.preview()
    ContentView(model: AppModel(clerk: clerk))
        .environment(clerk)
}
