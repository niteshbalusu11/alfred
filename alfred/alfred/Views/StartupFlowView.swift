import SwiftUI

struct StartupBootstrapView: View {
    @State private var isAnimating = false

    var body: some View {
        VStack(spacing: 24) {
            StartupHeroMark()
                .scaleEffect(isAnimating ? 1.02 : 0.98)
                .animation(
                    .easeInOut(duration: 0.9).repeatForever(autoreverses: true),
                    value: isAnimating
                )
                .onAppear {
                    isAnimating = true
                }

            Text("Bootstrapping Alfred")
                .font(.title3.weight(.black))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Text("Checking your session and preparing your workspace.")
                .font(.subheadline)
                .multilineTextAlignment(.center)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .padding(.horizontal, 28)

            ProgressView()
                .progressViewStyle(.circular)
                .tint(AppTheme.Colors.textPrimary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        .padding(.horizontal, AppTheme.Layout.screenPadding)
    }
}

struct StartupSignedOutView: View {
    let apiBaseURL: URL
    let onOpenAuth: () -> Void

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                StartupHeroMark()

                AppCard {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Your private assistant, in classic ink.")
                            .font(.title3.weight(.black))
                            .foregroundStyle(AppTheme.Colors.textPrimary)

                        Text("Log in or sign up with Clerk to continue.")
                            .font(.subheadline)
                            .foregroundStyle(AppTheme.Colors.textSecondary)
                    }
                }

                VStack(spacing: 12) {
                    Button("Log In") {
                        onOpenAuth()
                    }
                    .buttonStyle(.appPrimary)

                    Button("Sign Up") {
                        onOpenAuth()
                    }
                    .buttonStyle(.appSecondary)
                }

                Text(apiBaseURL.absoluteString)
                    .font(.caption.monospaced())
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                    .textSelection(.enabled)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(AppTheme.Colors.surface)
                    .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                    .overlay(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .stroke(AppTheme.Colors.outline, lineWidth: 2)
                    )
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.top, 48)
            .padding(.bottom, 32)
        }
    }
}

struct StartupAuthBootstrapFailureView: View {
    let message: String
    let onRetry: () -> Void
    let onSignOut: () -> Void

    var body: some View {
        VStack(spacing: 20) {
            StartupHeroMark()

            AppCard {
                VStack(alignment: .leading, spacing: 10) {
                    AppStatusBadge(title: "Session needs attention", style: .danger)

                    Text(message)
                        .font(.subheadline)
                        .foregroundStyle(AppTheme.Colors.textPrimary)

                    Text("Retry session bootstrap or sign out to restart.")
                        .font(.footnote)
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                }
            }

            Button("Retry") {
                onRetry()
            }
            .buttonStyle(.appPrimary)

            Button("Sign Out") {
                onSignOut()
            }
            .buttonStyle(.appSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .padding(.horizontal, AppTheme.Layout.screenPadding)
        .padding(.top, 56)
    }
}

private struct StartupHeroMark: View {
    var body: some View {
        VStack(spacing: 12) {
            Image("alfred_home")
                .resizable()
                .scaledToFit()
                .frame(width: 170, height: 170)
                .padding(12)
                .background(AppTheme.Colors.paper)
                .clipShape(RoundedRectangle(cornerRadius: 24, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 24, style: .continuous)
                        .stroke(AppTheme.Colors.outline, lineWidth: 4)
                )
                .shadow(color: AppTheme.Colors.shadow, radius: 0, x: 0, y: 8)

            Text("ALFRED")
                .font(.title.weight(.black))
                .tracking(1.5)
                .foregroundStyle(AppTheme.Colors.textPrimary)
        }
    }
}
