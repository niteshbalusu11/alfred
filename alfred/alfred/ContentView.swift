import SwiftUI

struct ContentView: View {
    @StateObject private var model = AppModel()

    var body: some View {
        NavigationStack {
            Group {
                if model.isAuthenticated {
                    DashboardView(model: model)
                } else {
                    SignInView(model: model)
                }
            }
            .navigationTitle("Alfred")
            .toolbar {
                if model.isAuthenticated {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Sign Out") {
                            Task {
                                await model.signOut()
                            }
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

#Preview {
    ContentView()
}
