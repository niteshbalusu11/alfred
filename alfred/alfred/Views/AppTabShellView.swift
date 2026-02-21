import ClerkKit
import ClerkKitUI
import SwiftUI

struct AppTabShellView: View {
    @Environment(Clerk.self) private var clerk
    @ObservedObject var model: AppModel
    @State private var tabPaths: [AppTab: NavigationPath] = AppTabShellView.defaultPaths()
    private let swipeTabs: [AppTab] = [.threads, .home, .automations, .connectors]
    private let visibleTopTabs: [AppTab] = [.home, .automations, .connectors]
    private let threadsHomeButtonSize: CGFloat = 47

    var body: some View {
        VStack(spacing: 0) {
            if !isThreadsSelected {
                topTabHeader
                    .transition(.asymmetric(
                        insertion: .opacity.animation(.easeInOut(duration: 0.18)),
                        removal: .opacity.animation(.easeInOut(duration: 0.14))
                    ))
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
                        .transition(.opacity.animation(.easeInOut(duration: 0.18)))
                }
            }
        }
        .appScreenBackground()
        .animation(.easeInOut(duration: 0.2), value: isThreadsSelected)
        .sensoryFeedback(.selection, trigger: model.selectedTab)
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
                            .regular.tint(AppTheme.Colors.paper.opacity(0.02)),
                            in: .rect(cornerRadius: 18)
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 18, style: .continuous)
                                .stroke(AppTheme.Colors.paper.opacity(0.1), lineWidth: 0.8)
                        )

                    if clerk.user != nil {
                        UserButton()
                            .frame(width: 36, height: 36)
                            .padding(8)
                            .glassEffect(
                                .regular.tint(AppTheme.Colors.paper.opacity(0.04)).interactive(),
                                in: .circle
                            )
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
        .tint(AppTheme.Colors.paper.opacity(0.2))
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
                    .font(.system(size: 18, weight: .semibold))
                    .frame(width: threadsHomeButtonSize, height: threadsHomeButtonSize)
            }
            .buttonStyle(.plain)
            .glassEffect(.regular.interactive(), in: .circle)
            .padding(.top, 8)
            .padding(.trailing, AppTheme.Layout.screenPadding)
            .accessibilityLabel("Back to home")
        } else {
            Button(action: action) {
                Image(systemName: "chevron.right")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .frame(width: threadsHomeButtonSize, height: threadsHomeButtonSize)
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
            AssistantThreadsView(model: model, reservesTrailingOverlaySpace: true)
        case .automations:
            AutomationsView(model: model)
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
