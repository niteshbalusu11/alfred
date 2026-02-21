import AlfredAPIClient
import SwiftUI

struct AutomationRuleCard: View {
    let title: String
    let rule: AutomationRuleSummary
    let isMutating: Bool
    let onTogglePause: () -> Void
    let onDelete: () -> Void
    let onDebugRun: (() -> Void)?

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
                    AutomationMetadataRow(label: "Time zone", value: rule.schedule.timeZone)
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

                    if let onDebugRun {
                        Button("Run now", action: onDebugRun)
                            .buttonStyle(.appSecondary)
                            .disabled(isMutating)
                    }
                }

                if isMutating {
                    ProgressView()
                        .tint(AppTheme.Colors.accent)
                }
            }
        }
    }

    private var intervalSummary: String {
        AutomationScheduleFormatter.label(for: rule.schedule)
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
    let schedule: AutomationSchedule
    let prompt: String
}

private enum TaskFrequency: String, CaseIterable, Identifiable {
    case daily
    case weekly
    case monthly
    case annually

    var id: String { rawValue }

    var title: String {
        switch self {
        case .daily:
            return "Daily"
        case .weekly:
            return "Weekly"
        case .monthly:
            return "Monthly"
        case .annually:
            return "Annually"
        }
    }

    var scheduleType: AutomationScheduleType {
        switch self {
        case .daily:
            return .daily
        case .weekly:
            return .weekly
        case .monthly:
            return .monthly
        case .annually:
            return .annually
        }
    }
}

struct AutomationCreateSheet: View {
    @Environment(\.dismiss) private var dismiss

    let defaultTimeZone: String
    let onSubmit: @Sendable (AutomationCreatePayload) async throws -> Void

    @State private var title = ""
    @State private var frequency: TaskFrequency = .daily
    @State private var selectedTime = Date()
    @State private var timeZone = ""
    @State private var prompt = ""
    @State private var isSubmitting = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    HStack(spacing: 12) {
                        Button {
                            dismiss()
                        } label: {
                            Image(systemName: "xmark")
                                .font(.system(size: 16, weight: .semibold))
                                .foregroundStyle(AppTheme.Colors.textSecondary)
                                .frame(width: 42, height: 42)
                                .background(AppTheme.Colors.surfaceElevated, in: Circle())
                                .overlay(
                                    Circle()
                                        .stroke(AppTheme.Colors.outline, lineWidth: 1)
                                )
                        }
                        .buttonStyle(.plain)
                        .disabled(isSubmitting)

                        Spacer(minLength: 0)

                        Text("New Task")
                            .font(.headline.weight(.semibold))
                            .foregroundStyle(AppTheme.Colors.textPrimary)

                        Spacer(minLength: 0)

                        Button(isSubmitting ? "Creating..." : "Create") {
                            Task {
                                await submit()
                            }
                        }
                        .font(.headline.weight(.semibold))
                        .foregroundStyle(AppTheme.Colors.textPrimary.opacity(isSubmitting ? 0.5 : 1))
                        .padding(.horizontal, 18)
                        .padding(.vertical, 10)
                        .background(AppTheme.Colors.surfaceElevated.opacity(0.95), in: Capsule())
                        .overlay(
                            Capsule()
                                .stroke(AppTheme.Colors.outline, lineWidth: 1)
                        )
                        .disabled(isSubmitting)
                    }

                    TextField("Name of task", text: $title)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .appFieldStyle()

                    VStack(alignment: .leading, spacing: 8) {
                        Text("Schedule")
                            .font(.headline.weight(.semibold))
                            .foregroundStyle(AppTheme.Colors.textSecondary)

                        VStack(spacing: 0) {
                            TaskPickerRow(label: "Frequency") {
                                Menu {
                                    ForEach(TaskFrequency.allCases) { option in
                                        Button {
                                            frequency = option
                                        } label: {
                                            if option == frequency {
                                                Label(option.title, systemImage: "checkmark")
                                            } else {
                                                Text(option.title)
                                            }
                                        }
                                    }
                                } label: {
                                    HStack(spacing: 6) {
                                        Text(frequency.title)
                                        Image(systemName: "chevron.up.chevron.down")
                                            .font(.system(size: 11, weight: .semibold))
                                    }
                                    .font(.subheadline.weight(.medium))
                                    .foregroundStyle(AppTheme.Colors.textPrimary)
                                }
                            }

                            Divider()
                                .overlay(AppTheme.Colors.outline)

                            TaskPickerRow(label: "Time") {
                                DatePicker(
                                    "",
                                    selection: $selectedTime,
                                    displayedComponents: .hourAndMinute
                                )
                                .labelsHidden()
                                .datePickerStyle(.compact)
                            }
                        }
                        .padding(.vertical, 4)
                        .background(AppTheme.Colors.surfaceElevated)
                        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                        .overlay(
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .stroke(AppTheme.Colors.outline, lineWidth: 1)
                        )
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        Text("Instructions")
                            .font(.headline.weight(.semibold))
                            .foregroundStyle(AppTheme.Colors.textSecondary)

                        ZStack(alignment: .topLeading) {
                            if prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                Text("Enter prompt")
                                    .font(.body)
                                    .foregroundStyle(AppTheme.Colors.textSecondary.opacity(0.7))
                                    .padding(.horizontal, 14)
                                    .padding(.vertical, 18)
                            }

                            TextEditor(text: $prompt)
                                .frame(minHeight: 160)
                                .padding(8)
                        }
                        .background(AppTheme.Colors.surfaceElevated)
                        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                        .overlay(
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .stroke(AppTheme.Colors.outline, lineWidth: 1)
                        )
                    }

                    if let errorMessage {
                        Text(errorMessage)
                            .font(.footnote)
                            .foregroundStyle(AppTheme.Colors.danger)
                            .padding(12)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(AppTheme.Colors.surfaceElevated)
                            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                    }
                }
                .padding(.horizontal, AppTheme.Layout.screenPadding)
                .padding(.vertical, AppTheme.Layout.sectionSpacing)
            }
            .appScreenBackground()
            .scrollDismissesKeyboard(.interactively)
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
                    schedule: AutomationSchedule(
                        scheduleType: frequency.scheduleType,
                        timeZone: trimmedTimeZone,
                        localTime: Self.localTimeFormatter.string(from: selectedTime)
                    ),
                    prompt: trimmedPrompt
                )
            )
            dismiss()
        } catch {
            errorMessage = AppModel.errorMessage(from: error)
        }
    }

    private static let localTimeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "HH:mm"
        return formatter
    }()
}

enum AutomationScheduleFormatter {
    static func label(for schedule: AutomationSchedule) -> String {
        let frequency = switch schedule.scheduleType {
        case .daily:
            "Daily"
        case .weekly:
            "Weekly"
        case .monthly:
            "Monthly"
        case .annually:
            "Annually"
        }

        return "\(frequency) at \(schedule.localTime)"
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

                Text("Loading tasks...")
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
