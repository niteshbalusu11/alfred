import AlfredAPIClient
import SwiftUI
enum ActivityFilter: String, CaseIterable {
    case all
    case reminder
    case brief
    case urgent
    case system
    var title: String {
        switch self {
        case .all: return "All"
        case .reminder: return "Reminder"
        case .brief: return "Brief"
        case .urgent: return "Urgent"
        case .system: return "System"
        }
    }
    var color: Color {
        switch self {
        case .all:
            return AppTheme.Colors.textSecondary
        case .reminder:
            return AppTheme.Colors.accent
        case .brief:
            return AppTheme.Colors.success
        case .urgent:
            return AppTheme.Colors.danger
        case .system:
            return AppTheme.Colors.warning
        }
    }
    func matches(event: AuditEvent) -> Bool {
        guard self != .all else { return true }
        return ActivityFilter.category(for: event) == self
    }
    static func category(for event: AuditEvent) -> ActivityFilter {
        let type = event.eventType.lowercased()
        if type.contains("reminder") {
            return .reminder
        }
        if type.contains("brief") {
            return .brief
        }
        if type.contains("urgent") {
            return .urgent
        }
        if type.contains("system") {
            return .system
        }
        return .system
    }
}
struct ActivityFilterPicker: View {
    @Binding var selectedFilter: ActivityFilter
    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(ActivityFilter.allCases, id: \.self) { filter in
                    ActivityFilterChip(
                        title: filter.title,
                        isSelected: selectedFilter == filter,
                        tint: filter.color
                    )
                    .onTapGesture {
                        selectedFilter = filter
                    }
                }
            }
            .padding(.vertical, 4)
        }
    }
}
struct ActivityFilterChip: View {
    let title: String
    let isSelected: Bool
    let tint: Color
    var body: some View {
        Text(title)
            .font(.footnote.weight(.semibold))
            .foregroundStyle(isSelected ? AppTheme.Colors.textPrimary : AppTheme.Colors.textSecondary)
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(isSelected ? tint.opacity(0.2) : AppTheme.Colors.surfaceElevated)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(isSelected ? tint : AppTheme.Colors.outline, lineWidth: 1)
            )
            .accessibilityLabel("Filter \(title)")
    }
}
struct ActivityTimeline: View {
    let events: [AuditEvent]
    var body: some View {
        VStack(spacing: 12) {
            ForEach(events.indices, id: \.self) { index in
                let event = events[index]
                let isLast = index == events.count - 1
                let category = ActivityFilter.category(for: event)
                NavigationLink {
                    ActivityEventDetailView(event: event, category: category)
                } label: {
                    ActivityTimelineRow(event: event, category: category, isLast: isLast)
                }
                .buttonStyle(.plain)
            }
        }
    }
}
struct ActivityTimelineRow: View {
    let event: AuditEvent
    let category: ActivityFilter
    let isLast: Bool
    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            ActivityTimelineMarker(isLast: isLast, tint: category.color)
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .center, spacing: 8) {
                    Text(event.eventType)
                        .font(.headline)
                        .foregroundStyle(AppTheme.Colors.textPrimary)
                    ActivityCategoryPill(title: category.title, tint: category.color)
                }
                Text(event.timestamp.formatted(date: .abbreviated, time: .shortened))
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                if let connector = event.connector, !connector.isEmpty {
                    Text("Connector: \(connector)")
                        .font(.footnote)
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                }
                Text("Result: \(event.result)")
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }
            .padding(.vertical, 2)
            Spacer(minLength: 0)
        }
    }
}
struct ActivityTimelineMarker: View {
    let isLast: Bool
    let tint: Color
    var body: some View {
        VStack(spacing: 0) {
            Circle()
                .fill(tint)
                .frame(width: 10, height: 10)
                .padding(.top, 4)
            Rectangle()
                .fill(AppTheme.Colors.outline)
                .frame(width: 2)
                .opacity(isLast ? 0 : 1)
                .padding(.top, 4)
                .frame(maxHeight: .infinity)
        }
        .frame(width: 16)
    }
}
struct ActivityCategoryPill: View {
    let title: String
    let tint: Color
    var body: some View {
        Text(title)
            .font(.caption.weight(.semibold))
            .foregroundStyle(tint)
            .padding(.horizontal, 8)
            .padding(.vertical, 2)
            .background(
                Capsule(style: .continuous)
                    .fill(tint.opacity(0.2))
            )
    }
}
struct ActivityInlineButton: View {
    let title: String
    let action: () -> Void
    var body: some View {
        Button(title, action: action)
            .font(.footnote.weight(.semibold))
            .foregroundStyle(AppTheme.Colors.textPrimary)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(AppTheme.Colors.surfaceElevated)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .stroke(AppTheme.Colors.outline, lineWidth: 1)
            )
            .accessibilityLabel(title)
    }
}
struct ActivityCallout: View {
    let title: String
    let message: String
    let buttonTitle: String
    let action: () -> Void
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.headline)
                .foregroundStyle(AppTheme.Colors.textPrimary)
            Text(message)
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
            ActivityInlineButton(title: buttonTitle, action: action)
        }
        .padding(12)
        .background(AppTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(AppTheme.Colors.outline, lineWidth: 1)
        )
    }
}
struct ActivityEmptyState: View {
    let title: String
    let message: String
    let buttonTitle: String
    let action: () -> Void
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.headline)
                .foregroundStyle(AppTheme.Colors.textPrimary)
            Text(message)
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
            ActivityInlineButton(title: buttonTitle, action: action)
        }
        .padding(.vertical, 8)
    }
}
struct ActivityLoadingState: View {
    var body: some View {
        VStack(spacing: 12) {
            ForEach(0..<3, id: \.self) { _ in
                ActivityLoadingRow()
            }
        }
        .redacted(reason: .placeholder)
    }
}
struct ActivityLoadingRow: View {
    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            ActivityTimelineMarker(isLast: false, tint: AppTheme.Colors.outline)
            VStack(alignment: .leading, spacing: 6) {
                Rectangle()
                    .fill(AppTheme.Colors.surfaceElevated)
                    .frame(height: 16)
                    .clipShape(RoundedRectangle(cornerRadius: 4))
                Rectangle()
                    .fill(AppTheme.Colors.surfaceElevated)
                    .frame(height: 12)
                    .clipShape(RoundedRectangle(cornerRadius: 4))
                Rectangle()
                    .fill(AppTheme.Colors.surfaceElevated)
                    .frame(height: 12)
                    .clipShape(RoundedRectangle(cornerRadius: 4))
            }
        }
    }
}
