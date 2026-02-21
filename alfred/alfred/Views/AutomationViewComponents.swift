import AlfredAPIClient
import SwiftUI

struct AutomationRuleCard: View {
    let title: String
    let rule: AutomationRuleSummary
    let isMutating: Bool
    let onTogglePause: () -> Void
    let onDelete: () -> Void

    var body: some View {
        AppCard {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .top, spacing: 12) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(title)
                            .font(.headline)
                            .foregroundStyle(AppTheme.Colors.textPrimary)

                        Text(intervalSummary)
                            .font(.subheadline)
                            .foregroundStyle(AppTheme.Colors.textSecondary)
                    }

                    Spacer(minLength: 8)

                    AppStatusBadge(title: statusTitle, style: statusStyle)
                }

                VStack(alignment: .leading, spacing: 6) {
                    AutomationMetadataRow(label: "Time zone", value: rule.timeZone)
                    AutomationMetadataRow(label: "Next run", value: format(date: rule.nextRunAt))
                    AutomationMetadataRow(label: "Last run", value: format(date: rule.lastRunAt))
                }

                HStack(spacing: 8) {
                    Button(rule.status == .active ? "Pause" : "Resume", action: onTogglePause)
                        .buttonStyle(.appSecondary)
                        .disabled(isMutating)

                    Button("Delete", role: .destructive, action: onDelete)
                        .buttonStyle(.appSecondary)
                        .disabled(isMutating)
                }

                if isMutating {
                    ProgressView()
                        .tint(AppTheme.Colors.accent)
                }
            }
        }
    }

    private var intervalSummary: String {
        "Runs \(AutomationIntervalFormatter.label(for: rule.intervalSeconds))"
    }

    private var statusTitle: String {
        switch rule.status {
        case .active:
            return "Active"
        case .paused:
            return "Paused"
        }
    }

    private var statusStyle: AppStatusBadge.Style {
        switch rule.status {
        case .active:
            return .success
        case .paused:
            return .neutral
        }
    }

    private func format(date: Date?) -> String {
        guard let date else {
            return "Never"
        }
        return date.formatted(date: .abbreviated, time: .shortened)
    }
}

private struct AutomationMetadataRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(label)
                .font(.caption.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .frame(width: 72, alignment: .leading)

            Text(value)
                .font(.caption)
                .foregroundStyle(AppTheme.Colors.textPrimary)
        }
    }
}

struct AutomationCreatePayload {
    let title: String
    let intervalSeconds: Int
    let timeZone: String
    let prompt: String
}

struct AutomationCreateSheet: View {
    @Environment(\.dismiss) private var dismiss

    let defaultTimeZone: String
    let onSubmit: @Sendable (AutomationCreatePayload) async throws -> Void

    @State private var title = ""
    @State private var intervalMinutes = 60
    @State private var timeZone = ""
    @State private var prompt = ""
    @State private var isSubmitting = false
    @State private var errorMessage: String?

    private let quickIntervals: [Int] = [15, 60, 240, 1_440]

    var body: some View {
        NavigationStack {
            Form {
                Section("Automation") {
                    TextField("Name (optional)", text: $title)

                    VStack(alignment: .leading, spacing: 10) {
                        Text("Interval")
                            .font(.subheadline.weight(.semibold))

                        HStack(spacing: 8) {
                            ForEach(quickIntervals, id: \.self) { minutes in
                                Button(quickLabel(minutes: minutes)) {
                                    intervalMinutes = minutes
                                }
                                .buttonStyle(.appSecondary)
                            }
                        }

                        Stepper(value: $intervalMinutes, in: 1...10_080) {
                            Text(AutomationIntervalFormatter.label(for: intervalMinutes * 60))
                        }
                    }

                    TextField("Time zone", text: $timeZone)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }

                Section("Prompt") {
                    TextEditor(text: $prompt)
                        .frame(minHeight: 180)

                    Text("Prompt and output remain encrypted outside the enclave path.")
                        .font(.footnote)
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .font(.footnote)
                            .foregroundStyle(AppTheme.Colors.danger)
                    }
                }
            }
            .scrollDismissesKeyboard(.interactively)
            .navigationTitle("New Automation")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel") {
                        dismiss()
                    }
                    .disabled(isSubmitting)
                }

                ToolbarItem(placement: .topBarTrailing) {
                    Button(isSubmitting ? "Creating…" : "Create") {
                        Task {
                            await submit()
                        }
                    }
                    .disabled(isSubmitting)
                }
            }
            .onAppear {
                if timeZone.isEmpty {
                    timeZone = defaultTimeZone
                }
            }
        }
    }

    private func submit() async {
        let trimmedPrompt = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedPrompt.isEmpty else {
            errorMessage = "Prompt is required."
            return
        }

        let trimmedTimeZone = timeZone.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedTimeZone.isEmpty else {
            errorMessage = "Time zone is required."
            return
        }
        guard TimeZone(identifier: trimmedTimeZone) != nil else {
            errorMessage = "Time zone must be a valid IANA identifier."
            return
        }

        isSubmitting = true
        defer { isSubmitting = false }

        do {
            try await onSubmit(
                AutomationCreatePayload(
                    title: title.trimmingCharacters(in: .whitespacesAndNewlines),
                    intervalSeconds: intervalMinutes * 60,
                    timeZone: trimmedTimeZone,
                    prompt: trimmedPrompt
                )
            )
            dismiss()
        } catch {
            errorMessage = AppModel.errorMessage(from: error)
        }
    }

    private func quickLabel(minutes: Int) -> String {
        AutomationIntervalFormatter.label(for: minutes * 60)
    }
}

enum AutomationIntervalFormatter {
    static func label(for intervalSeconds: Int) -> String {
        if intervalSeconds % 86_400 == 0 {
            return "every \(intervalSeconds / 86_400)d"
        }
        if intervalSeconds % 3_600 == 0 {
            return "every \(intervalSeconds / 3_600)h"
        }
        if intervalSeconds % 60 == 0 {
            return "every \(intervalSeconds / 60)m"
        }
        return "every \(intervalSeconds)s"
    }
}

struct AutomationInlineButton: View {
    let title: String
    let action: () -> Void

    var body: some View {
        Button(title, action: action)
            .font(.caption.weight(.semibold))
            .buttonStyle(.plain)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(AppTheme.Colors.surfaceElevated)
            .clipShape(Capsule())
            .overlay(
                Capsule()
                    .stroke(AppTheme.Colors.outline, lineWidth: 1)
            )
            .foregroundStyle(AppTheme.Colors.textPrimary)
    }
}

struct AutomationCallout: View {
    let title: String
    let message: String
    let buttonTitle: String
    let action: () -> Void

    var body: some View {
        AppCard {
            VStack(alignment: .leading, spacing: 10) {
                Text(title)
                    .font(.headline)
                    .foregroundStyle(AppTheme.Colors.textPrimary)

                Text(message)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)

                Button(buttonTitle, action: action)
                    .buttonStyle(.appSecondary)
            }
        }
    }
}

struct AutomationLoadingStateCard: View {
    var body: some View {
        AppCard {
            VStack(alignment: .leading, spacing: 12) {
                ProgressView()
                    .tint(AppTheme.Colors.accent)

                Text("Loading automations…")
                    .font(.subheadline)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }
        }
    }
}

struct AutomationEmptyStateCard: View {
    let title: String
    let message: String
    let buttonTitle: String
    let action: () -> Void

    var body: some View {
        AppCard {
            VStack(alignment: .leading, spacing: 10) {
                Text(title)
                    .font(.headline)
                    .foregroundStyle(AppTheme.Colors.textPrimary)

                Text(message)
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)

                Button(buttonTitle, action: action)
                    .buttonStyle(.appPrimary)
            }
        }
    }
}
