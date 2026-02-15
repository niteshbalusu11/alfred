import AlfredAPIClient
import SwiftUI

struct ActivityView: View {
    @ObservedObject var model: AppModel
    @State private var selectedFilter: ActivityFilter = .all

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
                HStack(spacing: 8) {
                    AppStatusBadge(title: model.activityStatusBadge.title, style: model.activityStatusBadge.style)

                    ActivityInlineButton(title: "Refresh") {
                        Task { await model.loadAuditEvents(reset: true) }
                    }
                    .disabled(model.isLoading(.loadAuditEvents))
                }
            }

            ActivityFilterPicker(selectedFilter: $selectedFilter)

            if showErrorCallout, let errorBanner = activityErrorBanner {
                ActivityCallout(
                    title: "Activity feed error",
                    message: errorBanner.message,
                    buttonTitle: "Retry"
                ) {
                    Task { await model.loadAuditEvents(reset: true) }
                }
            }

            if isInitialLoading {
                ActivityLoadingState()
            } else if filteredEvents.isEmpty {
                ActivityEmptyState(
                    title: emptyStateTitle,
                    message: emptyStateMessage,
                    buttonTitle: emptyStateButtonTitle
                ) {
                    Task { await model.loadAuditEvents(reset: true) }
                }
            } else {
                ActivityTimeline(events: filteredEvents)
            }

            if model.canLoadMoreAuditEvents {
                ActivityInlineButton(title: model.isLoading(.loadAuditEvents) ? "Loading…" : "Load More") {
                    Task { await model.loadAuditEvents(reset: false) }
                }
                .disabled(model.isLoading(.loadAuditEvents))
            }
        }
    }

    private var isInitialLoading: Bool {
        model.isLoading(.loadAuditEvents) && model.auditEvents.isEmpty
    }

    private var activityErrorBanner: AppModel.ErrorBanner? {
        guard let banner = model.errorBanner, banner.sourceAction == .loadAuditEvents else { return nil }
        return banner
    }

    private var showErrorCallout: Bool {
        activityErrorBanner != nil && !model.auditEvents.isEmpty
    }

    private var filteredEvents: [AuditEvent] {
        model.auditEvents.filter { selectedFilter.matches(event: $0) }
    }

    private var emptyStateTitle: String {
        if activityErrorBanner != nil && model.auditEvents.isEmpty {
            return "Unable to load activity"
        }

        switch selectedFilter {
        case .all:
            return "No activity yet"
        default:
            return "No \(selectedFilter.title.lowercased()) events"
        }
    }

    private var emptyStateMessage: String {
        if let errorBanner = activityErrorBanner, model.auditEvents.isEmpty {
            return errorBanner.message
        }

        switch selectedFilter {
        case .all:
            return "Alfred will show updates once events begin flowing."
        default:
            return "Try another filter or refresh to check for new events."
        }
    }

    private var emptyStateButtonTitle: String {
        activityErrorBanner != nil && model.auditEvents.isEmpty ? "Retry" : "Refresh Activity"
    }
}

#Preview {
    ActivityView(model: AppModel())
}
