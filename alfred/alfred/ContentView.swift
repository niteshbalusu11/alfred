import ClerkKit
import ClerkKitUI
import SwiftUI

struct ContentView: View {
    @Environment(Clerk.self) private var clerk
    @ObservedObject var model: AppModel
    @State private var authIsPresented = false

    var body: some View {
        NavigationStack {
            Group {
                if model.isAuthenticated {
                    DashboardView(model: model)
                } else {
                    signedOutView
                }
            }
            .navigationTitle("Alfred")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    if clerk.user != nil {
                        UserButton()
                            .frame(width: 36, height: 36)
                    } else {
                        Button("Sign in") {
                            authIsPresented = true
                        }
                    }
                }
            }
            .overlay(alignment: .top) {
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
        }
        .sheet(isPresented: $authIsPresented) {
            AuthView()
        }
    }

    private var signedOutView: some View {
        VStack(spacing: 16) {
            Text("You are signed out")
                .font(.headline)

            Text(model.apiBaseURL.absoluteString)
                .font(.footnote)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)

            Button("Sign in") {
                authIsPresented = true
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .padding(.top, 40)
    }
}

private struct ErrorBannerView: View {
    let message: String
    let onRetry: (() -> Void)?
    let onDismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(message)
                .font(.subheadline)

            HStack {
                if let onRetry {
                    Button("Retry", action: onRetry)
                        .buttonStyle(.borderedProminent)
                }

                Button("Dismiss", action: onDismiss)
                    .buttonStyle(.bordered)
            }
        }
        .padding()
        .background(.thinMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .shadow(radius: 4)
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
