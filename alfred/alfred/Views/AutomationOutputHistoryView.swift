import AlfredAPIClient
import SwiftUI

struct AutomationOutputHistorySheet: View {
    @Environment(\.dismiss) private var dismiss

    let store: AutomationOutputHistoryStore
    let initialRequestID: String?

    @State private var records: [AutomationOutputHistoryEntry] = []
    @State private var path: [String] = []
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var didApplyInitialRoute = false

    var body: some View {
        NavigationStack(path: $path) {
            Group {
                if isLoading && records.isEmpty {
                    ProgressView()
                        .tint(AppTheme.Colors.accent)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(AppTheme.Colors.background)
                } else if records.isEmpty {
                    AutomationOutputHistoryEmptyState(errorMessage: errorMessage) {
                        Task {
                            await loadHistory()
                        }
                    }
                } else {
                    List(records) { record in
                        NavigationLink(value: record.requestID) {
                            AutomationOutputHistoryRow(record: record)
                        }
                        .listRowBackground(AppTheme.Colors.surface)
                    }
                    .scrollContentBackground(.hidden)
                    .background(AppTheme.Colors.background)
                }
            }
            .navigationTitle("Task Outputs")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Close") { dismiss() }
                        .tint(AppTheme.Colors.textPrimary)
                }

                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        Task {
                            await loadHistory()
                        }
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .tint(AppTheme.Colors.textPrimary)
                    .disabled(isLoading)
                }
            }
            .navigationDestination(for: String.self) { requestID in
                if let record = records.first(where: { $0.requestID == requestID }) {
                    AutomationOutputDetailView(record: record)
                        .task {
                            _ = try? await store.markOpened(requestID: requestID)
                        }
                } else {
                    AutomationOutputDetailUnavailableView()
                }
            }
        }
        .appScreenBackground()
        .task {
            await loadHistory()
        }
    }

    @MainActor
    private func loadHistory() async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }

        do {
            records = try await store.list()
            errorMessage = nil
            applyInitialRouteIfNeeded()
        } catch {
            records = []
            errorMessage = "Unable to load task outputs."
        }
    }

    private func applyInitialRouteIfNeeded() {
        guard !didApplyInitialRoute else { return }
        didApplyInitialRoute = true
        guard let initialRequestID else { return }
        guard records.contains(where: { $0.requestID == initialRequestID }) else { return }
        path = [initialRequestID]
    }
}

private struct AutomationOutputHistoryRow: View {
    let record: AutomationOutputHistoryEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(record.title)
                .font(.headline)
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .lineLimit(2)

            Text(record.body)
                .font(.subheadline)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .lineLimit(3)

            HStack(spacing: 10) {
                Text(record.receivedAt.formatted(date: .abbreviated, time: .shortened))
                    .font(.caption)
                    .foregroundStyle(AppTheme.Colors.textSecondary)

                if record.openedAt != nil {
                    Text("Opened")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(AppTheme.Colors.success)
                }
            }
        }
        .padding(.vertical, 4)
    }
}

private struct AutomationOutputHistoryEmptyState: View {
    let errorMessage: String?
    let onRefresh: () -> Void

    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "clock.arrow.circlepath")
                .font(.system(size: 30, weight: .medium))
                .foregroundStyle(AppTheme.Colors.textSecondary)

            Text(errorMessage == nil ? "No task outputs yet" : "Unable to load task outputs")
                .font(.headline)
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Text(errorMessage ?? "Delivered automation results will appear here.")
                .font(.subheadline)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 24)

            Button("Refresh", action: onRefresh)
                .buttonStyle(.appSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, AppTheme.Layout.screenPadding)
    }
}

private struct AutomationOutputDetailView: View {
    let record: AutomationOutputHistoryEntry

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: AppTheme.Layout.sectionSpacing) {
                AppCard {
                    VStack(alignment: .leading, spacing: 12) {
                        Text(record.title)
                            .font(.title3.weight(.semibold))
                            .foregroundStyle(AppTheme.Colors.textPrimary)

                        Text(record.receivedAt.formatted(date: .complete, time: .shortened))
                            .font(.footnote)
                            .foregroundStyle(AppTheme.Colors.textSecondary)

                        Divider()
                            .overlay(AppTheme.Colors.outline)

                        Text(record.body)
                            .font(.body)
                            .foregroundStyle(AppTheme.Colors.textPrimary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            .padding(.horizontal, AppTheme.Layout.screenPadding)
            .padding(.vertical, AppTheme.Layout.sectionSpacing)
        }
        .appScreenBackground()
        .navigationTitle("Output")
        .navigationBarTitleDisplayMode(.inline)
    }
}

private struct AutomationOutputDetailUnavailableView: View {
    var body: some View {
        VStack(spacing: 10) {
            Text("Output not available")
                .font(.headline)
                .foregroundStyle(AppTheme.Colors.textPrimary)
            Text("This output may have been removed from local history.")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, AppTheme.Layout.screenPadding)
        .appScreenBackground()
    }
}
