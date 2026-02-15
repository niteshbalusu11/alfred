import SwiftUI

struct HomeView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                summarySection
                quickActionsSection
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
    }

    private var summarySection: some View {
        AppCard {
            AppSectionHeader("Today's Summary", subtitle: "Assistant status at a glance") {
                AppStatusBadge(title: "Live", style: .success)
            }

            StatusRow(title: "Connectors", status: model.googleStatusBadge)
            StatusRow(title: "Preferences", status: model.preferencesStatusBadge)
            StatusRow(title: "Privacy", status: model.privacyStatusBadge)
            StatusRow(title: "Activity", status: model.activityStatusBadge)
        }
    }

    private var quickActionsSection: some View {
        AppCard {
            AppSectionHeader("Quick Actions", subtitle: "Jump to the most-used areas")

            Button("Go to Connectors") {
                model.selectedTab = .connectors
            }
            .buttonStyle(.appPrimary)

            Button("View Activity") {
                model.selectedTab = .activity
            }
            .buttonStyle(.appSecondary)

            Button("Profile Settings") {
                model.selectedTab = .profile
            }
            .buttonStyle(.appSecondary)

            Button("Refresh Activity") {
                Task {
                    await model.loadAuditEvents(reset: true)
                }
            }
            .buttonStyle(.appSecondary)
            .disabled(model.isLoading(.loadAuditEvents))
        }
    }
}

private struct StatusRow: View {
    let title: String
    let status: (title: String, style: AppStatusBadge.Style)

    var body: some View {
        HStack {
            Text(title)
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Spacer()

            AppStatusBadge(title: status.title, style: status.style)
        }
        .padding(.vertical, 6)
    }
}

#Preview {
    HomeView(model: AppModel())
}
