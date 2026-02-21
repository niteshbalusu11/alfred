import AlfredAPIClient
import SwiftUI

struct AutomationsView: View {
    @ObservedObject var model: AppModel
    @State private var rules: [AutomationRuleSummary] = []
    @State private var localTitles: [UUID: String] = [:]
    @State private var isLoading = false
    @State private var loadErrorMessage: String?
    @State private var mutationErrorMessage: String?
    @State private var mutatingRuleIDs: Set<UUID> = []
    @State private var activeSheet: SheetRoute?

    private enum SheetRoute: String, Identifiable {
        case create

        var id: String { rawValue }
    }

    var body: some View {
        VStack(spacing: 0) {
            header

            if isLoading && rules.isEmpty {
                ScrollView {
                    AutomationLoadingStateCard()
                        .padding(.horizontal, AppTheme.Layout.screenPadding)
                        .padding(.vertical, AppTheme.Layout.sectionSpacing)
                }
            } else if rules.isEmpty, loadErrorMessage == nil {
                TaskEmptyStateHero {
                    activeSheet = .create
                }
            } else {
                ScrollView {
                    LazyVStack(spacing: AppTheme.Layout.sectionSpacing) {
                        if let mutationErrorMessage {
                            AutomationCallout(
                                title: "Task action failed",
                                message: mutationErrorMessage,
                                buttonTitle: "Dismiss"
                            ) {
                                self.mutationErrorMessage = nil
                            }
                        }

                        if let loadErrorMessage, rules.isEmpty {
                            AutomationEmptyStateCard(
                                title: "Unable to load tasks",
                                message: loadErrorMessage,
                                buttonTitle: "Retry"
                            ) {
                                Task {
                                    await loadAutomations()
                                }
                            }
                        } else {
                            ForEach(rules, id: \.ruleId) { rule in
                                AutomationRuleCard(
                                    title: title(for: rule),
                                    rule: rule,
                                    isMutating: mutatingRuleIDs.contains(rule.ruleId),
                                    onTogglePause: { togglePause(for: rule) },
                                    onDelete: { delete(rule: rule) },
                                    onDebugRun: debugRunAction(for: rule)
                                )
                            }

                            if let loadErrorMessage {
                                AutomationCallout(
                                    title: "Could not refresh tasks",
                                    message: loadErrorMessage,
                                    buttonTitle: "Retry"
                                ) {
                                    Task {
                                        await loadAutomations()
                                    }
                                }
                            }
                        }
                    }
                    .padding(.horizontal, AppTheme.Layout.screenPadding)
                    .padding(.bottom, AppTheme.Layout.sectionSpacing)
                    .padding(.top, 8)
                }
            }
        }
        .appScreenBackground()
        .task {
            if rules.isEmpty {
                await loadAutomations()
            }
        }
        .fullScreenCover(item: $activeSheet) { _ in
            AutomationCreateSheet(
                defaultTimeZone: TimeZone.current.identifier,
                onSubmit: { payload in
                    try await createAutomation(payload)
                }
            )
        }
    }

    private var header: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                Text("Tasks")
                    .font(.title2.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)

                Text("Scheduled prompts with private push delivery")
                    .font(.footnote)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }

            Spacer(minLength: 0)

            TaskHeaderIconButton(
                systemImage: isLoading ? "arrow.triangle.2.circlepath.circle.fill" : "arrow.clockwise",
                accessibilityLabel: "Refresh tasks",
                action: {
                    Task {
                        await loadAutomations()
                    }
                }
            )
            .disabled(isLoading)
            .opacity(isLoading ? 0.55 : 1)

            TaskHeaderIconButton(
                systemImage: "plus",
                accessibilityLabel: "Create task",
                action: {
                    activeSheet = .create
                }
            )
        }
        .padding(.horizontal, AppTheme.Layout.screenPadding)
        .padding(.top, 8)
        .padding(.bottom, 10)
    }

    @MainActor
    private func loadAutomations() async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }

        do {
            let response = try await model.apiClient.listAutomations(limit: 100)
            rules = response.items.sorted(by: { $0.nextRunAt < $1.nextRunAt })
            loadErrorMessage = nil
        } catch {
            loadErrorMessage = AppModel.errorMessage(from: error)
        }
    }

    @MainActor
    private func createAutomation(_ payload: AutomationCreatePayload) async throws {
        let created = try await model.apiClient.createAutomationEncrypted(
            schedule: payload.schedule,
            prompt: payload.prompt,
            attestationConfig: AppConfiguration.assistantAttestationVerificationConfig
        )

        if !payload.title.isEmpty {
            localTitles[created.ruleId] = payload.title
        }

        upsert(created)
        loadErrorMessage = nil
    }

    private func togglePause(for rule: AutomationRuleSummary) {
        Task { @MainActor in
            guard !mutatingRuleIDs.contains(rule.ruleId) else { return }
            mutatingRuleIDs.insert(rule.ruleId)
            defer { mutatingRuleIDs.remove(rule.ruleId) }

            do {
                let nextStatus: AutomationStatus = rule.status == .active ? .paused : .active
                let updated = try await model.apiClient.updateAutomation(
                    ruleID: rule.ruleId,
                    request: UpdateAutomationRequest(status: nextStatus)
                )
                upsert(updated)
                mutationErrorMessage = nil
            } catch {
                mutationErrorMessage = AppModel.errorMessage(from: error)
            }
        }
    }

    private func delete(rule: AutomationRuleSummary) {
        Task { @MainActor in
            guard !mutatingRuleIDs.contains(rule.ruleId) else { return }
            mutatingRuleIDs.insert(rule.ruleId)
            defer { mutatingRuleIDs.remove(rule.ruleId) }

            do {
                _ = try await model.apiClient.deleteAutomation(ruleID: rule.ruleId)
                rules.removeAll(where: { $0.ruleId == rule.ruleId })
                localTitles.removeValue(forKey: rule.ruleId)
                mutationErrorMessage = nil
            } catch {
                mutationErrorMessage = AppModel.errorMessage(from: error)
            }
        }
    }

    private func debugRun(rule: AutomationRuleSummary) {
        Task { @MainActor in
            guard !mutatingRuleIDs.contains(rule.ruleId) else { return }
            mutatingRuleIDs.insert(rule.ruleId)
            defer { mutatingRuleIDs.remove(rule.ruleId) }

            do {
                _ = try await model.apiClient.triggerAutomationDebugRun(ruleID: rule.ruleId)
                mutationErrorMessage = nil
            } catch {
                mutationErrorMessage = AppModel.errorMessage(from: error)
            }
        }
    }

    private func debugRunAction(for rule: AutomationRuleSummary) -> (() -> Void)? {
        #if DEBUG
            return { debugRun(rule: rule) }
        #else
            return nil
        #endif
    }

    private func upsert(_ updated: AutomationRuleSummary) {
        if let index = rules.firstIndex(where: { $0.ruleId == updated.ruleId }) {
            rules[index] = updated
        } else {
            rules.append(updated)
        }
        rules.sort { $0.nextRunAt < $1.nextRunAt }
    }

    private func title(for rule: AutomationRuleSummary) -> String {
        if let local = localTitles[rule.ruleId], !local.isEmpty {
            return local
        }
        return "Task \(rule.ruleId.uuidString.prefix(8))"
    }
}
