import ClerkKit
import ClerkKitUI
import SwiftUI

struct AppTabShellView: View {
    @Environment(Clerk.self) private var clerk
    @ObservedObject var model: AppModel
    @State private var tabPaths: [AppTab: NavigationPath] = AppTabShellView.defaultPaths()

    var body: some View {
        TabView(selection: $model.selectedTab) {
            ForEach(AppTab.allCases, id: \.self) { tab in
                NavigationStack(path: binding(for: tab)) {
                    tabContent(for: tab)
                        .navigationTitle(tab.title)
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
                .tabItem {
                    Label(tab.title, systemImage: tab.systemImage)
                }
                .tag(tab)
            }
        }
    }

    private static func defaultPaths() -> [AppTab: NavigationPath] {
        Dictionary(uniqueKeysWithValues: AppTab.allCases.map { ($0, NavigationPath()) })
    }

    private func binding(for tab: AppTab) -> Binding<NavigationPath> {
        Binding(
            get: { tabPaths[tab] ?? NavigationPath() },
            set: { tabPaths[tab] = $0 }
        )
    }

    @ViewBuilder
    private func tabContent(for tab: AppTab) -> some View {
        switch tab {
        case .home:
            HomeView(model: model)
        case .activity:
            ActivityView(model: model)
        case .connectors:
            ConnectorsView(model: model)
        }
    }
}

#Preview {
    let clerk = Clerk.preview()
    AppTabShellView(model: AppModel(clerk: clerk))
        .environment(clerk)
}
