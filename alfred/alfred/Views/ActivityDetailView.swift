import AlfredAPIClient
import SwiftUI

struct ActivityEventDetailView: View {
    let event: AuditEvent
    let category: ActivityFilter

    var body: some View {
        ScrollView {
            LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                AppCard {
                    AppSectionHeader("Event Detail", subtitle: event.eventType) {
                        ActivityCategoryPill(title: category.title, tint: category.color)
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        ActivityDetailRow(label: "Timestamp", value: event.timestamp.formatted(date: .abbreviated, time: .shortened))
                        ActivityDetailRow(label: "Result", value: event.result)

                        if let connector = event.connector, !connector.isEmpty {
                            ActivityDetailRow(label: "Connector", value: connector)
                        }
                    }
                }

                AppCard {
                    AppSectionHeader("Metadata", subtitle: event.metadata.isEmpty ? "No metadata attached" : nil)

                    if event.metadata.isEmpty {
                        Text("Metadata will appear here when events include additional context.")
                            .font(.footnote)
                            .foregroundStyle(AppTheme.Colors.textSecondary)
                    } else {
                        VStack(alignment: .leading, spacing: 10) {
                            ForEach(event.metadata.sorted(by: { $0.key < $1.key }), id: \.key) { key, value in
                                ActivityDetailRow(label: key, value: value.displayValue)
                            }
                        }
                    }
                }
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
        .navigationTitle("Activity")
    }
}

struct ActivityDetailRow: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label)
                .font(.caption.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textSecondary)
            Text(value)
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textPrimary)
        }
    }
}

extension StringOrNumberOrBool {
    var displayValue: String {
        switch self {
        case .string(let value):
            return value
        case .int(let value):
            return String(value)
        case .double(let value):
            return String(format: "%.2f", value)
        case .bool(let value):
            return value ? "true" : "false"
        }
    }
}
