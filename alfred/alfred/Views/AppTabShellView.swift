import ClerkKit
import ClerkKitUI
import SwiftUI

struct AppTabShellView: View {
    @Environment(Clerk.self) private var clerk
    @ObservedObject var model: AppModel
    @State private var tabPaths: [AppTab: NavigationPath] = AppTabShellView.defaultPaths()
    private let swipeTabs: [AppTab] = [.threads, .home, .activity, .connectors]
    private let visibleTopTabs: [AppTab] = [.home, .activity, .connectors]

    var body: some View {
        VStack(spacing: 0) {
            if !isThreadsSelected {
                topTabHeader
            }

            ZStack(alignment: .topTrailing) {
                TabView(selection: $model.selectedTab) {
                    ForEach(swipeTabs, id: \.self) { tab in
                        NavigationStack(path: binding(for: tab)) {
                            tabContent(for: tab)
                                .toolbar(.hidden, for: .navigationBar)
                        }
                        .tag(tab)
                    }
                }
                .tabViewStyle(.page(indexDisplayMode: .never))

                if isThreadsSelected {
                    threadsBackToHomeButton
                }
            }
        }
        .appScreenBackground()
    }

    private var isThreadsSelected: Bool {
        model.selectedTab == .threads
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
        Picker("Top Tabs", selection: topTabSelectionBinding) {
            ForEach(visibleTopTabs, id: \.self) { tab in
                Text(tab.title)
                    .tag(tab)
            }
        }
        .pickerStyle(.segmented)
        .accessibilityLabel("Top tabs")
    }

    private var topTabSelectionBinding: Binding<AppTab> {
        Binding(
            get: {
                visibleTopTabs.contains(model.selectedTab) ? model.selectedTab : .home
            },
            set: { newValue in
                model.selectedTab = newValue
            }
        )
    }

    @ViewBuilder
    private var threadsBackToHomeButton: some View {
        let action = {
            withAnimation(.easeInOut(duration: 0.2)) {
                model.selectedTab = .home
            }
        }

        if #available(iOS 26, *) {
            Button(action: action) {
                Image(systemName: "chevron.right")
                    .font(.headline.weight(.semibold))
                    .frame(width: 36, height: 36)
            }
            .buttonStyle(.plain)
            .glassEffect(.regular.interactive(), in: .circle)
            .padding(.top, 8)
            .padding(.trailing, AppTheme.Layout.screenPadding)
            .accessibilityLabel("Back to home")
        } else {
            Button(action: action) {
                Image(systemName: "chevron.right")
                    .font(.headline.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .frame(width: 36, height: 36)
                    .background(AppTheme.Colors.surfaceElevated.opacity(0.95))
                    .clipShape(Circle())
                    .overlay(
                        Circle()
                            .stroke(AppTheme.Colors.outline, lineWidth: AppTheme.Layout.cartoonStrokeWidth)
                    )
            }
            .buttonStyle(.plain)
            .padding(.top, 8)
            .padding(.trailing, AppTheme.Layout.screenPadding)
            .accessibilityLabel("Back to home")
        }
    }

    @ViewBuilder
    private func tabContent(for tab: AppTab) -> some View {
        switch tab {
        case .home:
            HomeView(model: model)
        case .threads:
            AssistantThreadsView(model: model)
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
