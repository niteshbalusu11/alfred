import ClerkKit
import ClerkKitUI
import SwiftUI

struct AppTabShellView: View {
    @Environment(Clerk.self) private var clerk
    @ObservedObject var model: AppModel
    @State private var tabPaths: [AppTab: NavigationPath] = AppTabShellView.defaultPaths()

    var body: some View {
        VStack(spacing: 0) {
            topTabHeader

            TabView(selection: $model.selectedTab) {
                ForEach(AppTab.allCases, id: \.self) { tab in
                    NavigationStack(path: binding(for: tab)) {
                        tabContent(for: tab)
                            .toolbar(.hidden, for: .navigationBar)
                    }
                    .tag(tab)
                }
            }
            .tabViewStyle(.page(indexDisplayMode: .never))
        }
        .appScreenBackground()
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
    private var topTabHeader: some View {
        if #available(iOS 26, *) {
            GlassEffectContainer(spacing: 12) {
                HStack(spacing: 10) {
                    tabPicker
                        .padding(.horizontal, 10)
                        .padding(.vertical, 8)
                        .glassEffect(
                            .regular.tint(AppTheme.Colors.paper.opacity(0.12)).interactive(),
                            in: .rect(cornerRadius: 18)
                        )

                    if clerk.user != nil {
                        UserButton()
                            .frame(width: 36, height: 36)
                            .padding(8)
                            .glassEffect(.regular.interactive(), in: .circle)
                    }
                }
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.top, 8)
            .padding(.bottom, 10)
        } else {
            HStack(spacing: 10) {
                tabPicker

                if clerk.user != nil {
                    UserButton()
                        .frame(width: 36, height: 36)
                }
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.top, 8)
            .padding(.bottom, 10)
        }
    }

    private var tabPicker: some View {
        Picker("Top Tabs", selection: $model.selectedTab) {
            ForEach(AppTab.allCases, id: \.self) { tab in
                Text(tab.title)
                    .tag(tab)
            }
        }
        .pickerStyle(.segmented)
        .accessibilityLabel("Top tabs")
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
