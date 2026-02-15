import AlfredAPIClient
import SwiftUI

struct ActivityView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                activitySection
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
    }

    private var activitySection: some View {
        AppCard {
            AppSectionHeader("Activity Log", subtitle: "Recent events and outcomes") {
                AppStatusBadge(title: model.activityStatusBadge.title, style: model.activityStatusBadge.style)
            }

            Button("Refresh Activity") {
                Task {
                    await model.loadAuditEvents(reset: true)
                }
            }
            .buttonStyle(.appSecondary)
            .disabled(model.isLoading(.loadAuditEvents))

            if model.auditEvents.isEmpty {
                Text("No events yet.")
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            } else {
                ForEach(model.auditEvents, id: \.id) { event in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(event.eventType)
                            .font(.headline)
                            .foregroundStyle(AppTheme.Colors.textPrimary)
                        Text(event.timestamp.formatted(date: .abbreviated, time: .shortened))
                            .font(.footnote)
                            .foregroundStyle(AppTheme.Colors.textSecondary)
                        Text("Result: \(event.result)")
                            .font(.footnote)
                            .foregroundStyle(AppTheme.Colors.textSecondary)
                    }
                    .padding(.vertical, 6)
                }
            }

            if model.canLoadMoreAuditEvents {
                Button("Load More") {
                    Task {
                        await model.loadAuditEvents(reset: false)
                    }
                }
                .buttonStyle(.appSecondary)
                .disabled(model.isLoading(.loadAuditEvents))
            }

            if model.isLoading(.loadAuditEvents) {
                ProgressView()
                    .tint(AppTheme.Colors.accent)
            }
        }
    }
}

#Preview {
    ActivityView(model: AppModel())
}
