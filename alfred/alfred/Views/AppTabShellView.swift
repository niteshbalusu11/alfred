import ClerkKit
import ClerkKitUI
import SwiftUI

struct AppTabShellView: View {
    @Environment(Clerk.self) private var clerk
    @ObservedObject var model: AppModel
    @State private var homePath = NavigationPath()
    @State private var activityPath = NavigationPath()
    @State private var connectorsPath = NavigationPath()
    @State private var profilePath = NavigationPath()

    var body: some View {
        TabView(selection: $model.selectedTab) {
            tabRoot(title: AppTab.home.title, path: $homePath) {
                HomeView(model: model)
            }
            .tabItem {
                Label(AppTab.home.title, systemImage: AppTab.home.systemImage)
            }
            .tag(AppTab.home)

            tabRoot(title: AppTab.activity.title, path: $activityPath) {
                ActivityView(model: model)
            }
            .tabItem {
                Label(AppTab.activity.title, systemImage: AppTab.activity.systemImage)
            }
            .tag(AppTab.activity)

            tabRoot(title: AppTab.connectors.title, path: $connectorsPath) {
                ConnectorsView(model: model)
            }
            .tabItem {
                Label(AppTab.connectors.title, systemImage: AppTab.connectors.systemImage)
            }
            .tag(AppTab.connectors)

            tabRoot(title: AppTab.profile.title, path: $profilePath) {
                ProfileView(model: model)
            }
            .tabItem {
                Label(AppTab.profile.title, systemImage: AppTab.profile.systemImage)
            }
            .tag(AppTab.profile)
        }
    }

    private func tabRoot<Content: View>(
        title: String,
        path: Binding<NavigationPath>,
        @ViewBuilder content: () -> Content
    ) -> some View {
        NavigationStack(path: path) {
            content()
                .navigationTitle(title)
                .toolbarBackground(AppTheme.Colors.background, for: .navigationBar)
                .toolbarBackground(.visible, for: .navigationBar)
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        if clerk.user != nil {
                            UserButton()
                                .frame(width: 36, height: 36)
                        }
                    }
                }
        }
    }
}

#Preview {
    let clerk = Clerk.preview()
    AppTabShellView(model: AppModel(clerk: clerk))
        .environment(clerk)
}
